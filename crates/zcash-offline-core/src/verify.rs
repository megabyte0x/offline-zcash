//! Offline payee and gateway checks on raw transaction bytes.
//!
//! These prove the transaction is well formed, pays this payee, and spends
//! notes under an anchor the payee has seen. They cannot prove the notes are
//! unspent, so the result is "pending" until a gateway reports it mined.

use orchard::{bundle::BatchValidator, circuit::{OrchardCircuitVersion, VerifyingKey}, keys::Scope};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use zcash_primitives::transaction::{Transaction, TxVersion};
use zcash_protocol::consensus::BranchId;

use crate::{fleet::Policy, wallet::WalletState, DeviceSecret};

fn verifying_key() -> &'static VerifyingKey {
    static VK: OnceLock<VerifyingKey> = OnceLock::new();
    VK.get_or_init(|| VerifyingKey::build(OrchardCircuitVersion::PostNu6_3))
}

/// Why a payment was refused. The numeric code travels in receipts and logs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reject {
    pub code: u8,
    pub reason: String,
}

impl std::fmt::Display for Reject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "refused (code {}): {}", self.code, self.reason)
    }
}
impl std::error::Error for Reject {}

pub mod code {
    pub const PARSE: u8 = 1;
    pub const BRANCH: u8 = 2;
    pub const NOT_IRONWOOD_ONLY: u8 = 3;
    pub const NOT_TO_ME: u8 = 4;
    pub const BAD_PROOF_OR_SIG: u8 = 5;
    pub const ANCHOR: u8 = 6;
    pub const EXPIRY: u8 = 7;
    pub const CONFLICT: u8 = 8;
    pub const FEE: u8 = 9;
    pub const OVER_FLOOR: u8 = 10;
    pub const STATEMENT: u8 = 11;
}

fn rej(code: u8, reason: impl Into<String>) -> Reject {
    Reject { code, reason: reason.into() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckedTx {
    pub txid: String,
    pub auth_digest: String,
    pub expiry_height: u32,
    pub anchor: String,
    pub nullifiers: Vec<String>,
    pub fee: u64,
}

/// Structure, fee, proofs and signatures. No keys or chain view needed; the
/// gateway runs this before submitting (about 8 ms of proof checking).
pub fn check_tx(raw: &[u8], policy: &Policy) -> Result<(Transaction, CheckedTx), Reject> {
    let branch = BranchId::try_from(policy.branch_id).map_err(|_| rej(code::BRANCH, "policy branch id unknown"))?;
    let tx = Transaction::read(raw, branch).map_err(|e| rej(code::PARSE, format!("cannot parse: {e}")))?;
    if tx.version() != TxVersion::V6 || tx.consensus_branch_id() != branch {
        return Err(rej(code::BRANCH, format!("need v6 at branch {:08x}", policy.branch_id)));
    }
    if tx.transparent_bundle().is_some() || tx.sapling_bundle().is_some() || tx.orchard_bundle().is_some() || tx.sprout_bundle().is_some() {
        return Err(rej(code::NOT_IRONWOOD_ONLY, "transaction has non-Ironwood components"));
    }
    let bundle = tx.ironwood_bundle().ok_or_else(|| rej(code::NOT_IRONWOOD_ONLY, "no Ironwood bundle"))?;
    let fee = i64::from(*bundle.value_balance());
    let required = crate::builder::transaction_fee(bundle.actions().len(), policy)
        .map_err(|e| rej(code::FEE, e.to_string()))?;
    if fee != required as i64 {
        return Err(rej(code::FEE, format!("fee {fee} is not the ZIP 317 / policy fee {required}")));
    }
    // For a shielded-only v6 transaction the shielded sighash equals the txid.
    let sighash: [u8; 32] = *tx.txid().as_ref();
    let mut v = BatchValidator::new(verifying_key());
    v.add_bundle(bundle, sighash).map_err(|e| rej(code::BAD_PROOF_OR_SIG, format!("{e:?}")))?;
    if !v.validate(UnwrapErr(SysRng)) {
        return Err(rej(code::BAD_PROOF_OR_SIG, "proof or signature does not verify"));
    }
    let checked = CheckedTx {
        txid: tx.txid().to_string(),
        auth_digest: hex::encode(tx.auth_commitment().as_bytes()),
        expiry_height: u32::from(tx.expiry_height()),
        anchor: hex::encode(bundle.anchor().to_bytes()),
        nullifiers: bundle.actions().iter().map(|a| hex::encode(a.nullifier().to_bytes())).collect(),
        fee: fee as u64,
    };
    Ok((tx, checked))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Verified {
    pub tx: CheckedTx,
    /// Zatoshis paid to this payee.
    pub amount: u64,
    /// Height of the payee's own chain view the checks ran against.
    pub view_height: u32,
    pub anchor_height: u32,
}

/// Full payee check against this device's keys and last synced chain view.
pub fn verify_payment(raw: &[u8], me: &DeviceSecret, view: &WalletState, policy: &Policy) -> Result<Verified, Reject> {
    let (tx, checked) = check_tx(raw, policy)?;
    let bundle = tx.ironwood_bundle().expect("checked");
    let fvk = me.fvk().map_err(|e| rej(code::NOT_TO_ME, e.to_string()))?;
    let ivk = fvk.to_ivk(Scope::External);
    let amount: u64 = bundle
        .decrypt_outputs_with_keys(&[ivk])
        .iter()
        .map(|(_, _, note, _, _)| note.value().inner())
        .sum();
    if amount == 0 {
        return Err(rej(code::NOT_TO_ME, "no output decrypts to this payee"));
    }
    let anchor: [u8; 32] = hex::decode(&checked.anchor).unwrap().try_into().unwrap();
    let anchor_height = view.anchor_height(&anchor).ok_or_else(|| {
        rej(code::ANCHOR, format!("anchor is not a tree root this payee has seen (payee view height {})", view.tip_height))
    })?;
    if view.tip_height.saturating_sub(anchor_height) > policy.anchor_window {
        return Err(rej(code::ANCHOR, format!("anchor at height {anchor_height} is older than {} blocks", policy.anchor_window)));
    }
    if checked.expiry_height != 0 && checked.expiry_height < view.tip_height + policy.expiry_margin {
        return Err(rej(code::EXPIRY, format!("expiry {} is within {} blocks of the payee's view {}", checked.expiry_height, policy.expiry_margin, view.tip_height)));
    }
    if checked.expiry_height == 0 {
        return Err(rej(code::EXPIRY, "expiry 0 (never expires) is refused"));
    }
    // A gateway can mine and sync this exact payment before its mesh envelope arrives.
    let already_received = view.notes.iter().any(|note| note.txid == checked.txid);
    if let Some(nf) = checked.nullifiers.iter().find(|nf| !already_received && view.chain_nullifiers.contains(*nf)) {
        return Err(rej(code::CONFLICT, format!("note {nf} was already spent on chain")));
    }
    if amount > policy.floor_zat {
        return Err(rej(code::OVER_FLOOR, format!("{amount} zat is above the pending floor {}", policy.floor_zat)));
    }
    Ok(Verified { tx: checked, amount, view_height: view.tip_height, anchor_height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{builder::build_payment, wallet::{BlockData, ChunkSource, StoredNote}};

    #[test]
    fn mined_before_mesh_receipt_is_not_a_conflicting_spend() {
        let a = DeviceSecret::generate("A", "payer");
        let b = DeviceSecret::generate("B", "payee");
        let policy = Policy::default();
        let note = StoredNote {
            position: 0, height: 1, txid: "funding".into(), value: 1_000_000,
            recipient: hex::encode(a.address().unwrap().to_raw_address_bytes()),
            rho: hex::encode([0; 32]), rseed: hex::encode([7; 32]), nullifier: String::new(),
            spent_in: None, locked_by: None,
        };
        let cm = orchard::note::ExtractedNoteCommitment::from(note.note().unwrap().commitment());
        let mut payer = WalletState { tip_height: 1, tip_hash: "block1".into(),
            commitments: vec![hex::encode(cm.to_bytes())], notes: vec![note], ..Default::default() };
        let (_, root) = payer.witness(0).unwrap();
        payer.roots.insert(1, hex::encode(root));
        let mut competing = payer.clone();
        let mut payee = payer.clone();
        payee.notes.clear();
        let payment = build_payment(&mut payer, &a, b.address().unwrap(), 20_003, &policy).unwrap();
        assert_eq!(verify_payment(&payment.raw, &b, &payee, &policy).unwrap().amount, 20_003);
        payee.sync_from(&ChunkSource { prev: (1, "block1".into()), blocks: vec![BlockData {
            height: 2, hash: "block2".into(), txs: vec![payment.raw.clone()],
        }] }, &b.fvk().unwrap()).unwrap();
        assert_eq!(verify_payment(&payment.raw, &b, &payee, &policy).unwrap().amount, 20_003);
        let conflict = build_payment(&mut competing, &a, b.address().unwrap(), 20_007, &policy).unwrap();
        assert_eq!(verify_payment(&conflict.raw, &b, &payee, &policy).err().unwrap().code, code::CONFLICT);
    }
}
