//! Offline payer: builds, proves and signs one v6 Ironwood payment with no network.
//!
//! Recipe (mined on `ths` in the expiry research): Orchard-crate builder with
//! the Ironwood v3 bundle version, `TransactionData::from_parts_v6` at NU7, and
//! spend-auth signatures over the shielded sighash (equal to the txid for a
//! shielded-only v6 transaction).

use anyhow::{anyhow, bail, ensure, Result};
use orchard::{
    builder::{BundleType, Builder},
    bundle::BundleVersion,
    circuit::{OrchardCircuitVersion, ProvingKey},
    keys::{Scope, SpendAuthorizingKey},
    value::NoteValue,
};
use rand::{rand_core::UnwrapErr, rngs::{StdRng, SysRng}, SeedableRng, TryRng};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use zcash_primitives::transaction::{
    txid::{to_txid, TxIdDigester},
    Authorized, TransactionData, TxVersion, Unauthorized,
};
use zcash_protocol::{consensus::BranchId, value::ZatBalance};

use crate::{fleet::Policy, wallet::WalletState, DeviceSecret};

pub fn proving_key() -> &'static ProvingKey {
    static PK: OnceLock<ProvingKey> = OnceLock::new();
    PK.get_or_init(|| ProvingKey::build(OrchardCircuitVersion::PostNu6_3))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuiltPayment {
    pub raw: Vec<u8>,
    pub txid: String,
    pub auth_digest: String,
    pub amount: u64,
    pub fee: u64,
    pub expiry_height: u32,
    pub anchor_height: u32,
    /// Positions of the notes this payment spends (now locked in the wallet).
    pub spent_positions: Vec<u64>,
    pub prove_ms: u128,
}

/// Picks the smallest single spendable note covering `need`, else the smallest
/// pair (at most two inputs keeps the transaction at two actions).
fn select_notes(state: &WalletState, need: u64) -> Result<Vec<u64>> {
    let mut notes: Vec<_> = state.notes.iter().filter(|n| n.spendable()).collect();
    notes.sort_by_key(|n| n.value);
    if let Some(n) = notes.iter().find(|n| n.value >= need) {
        return Ok(vec![n.position]);
    }
    let mut best: Option<(u64, u64, u64)> = None;
    for (i, a) in notes.iter().enumerate() {
        for b in &notes[i + 1..] {
            let v = a.value + b.value;
            if v >= need && best.is_none_or(|(bv, _, _)| v < bv) {
                best = Some((v, a.position, b.position));
            }
        }
    }
    best.map(|(_, a, b)| vec![a, b]).ok_or_else(|| {
        anyhow!(
            "not enough spendable notes for {} zat (spendable {} zat in {} notes); unmined change cannot be spent offline",
            need,
            notes.iter().map(|n| n.value).sum::<u64>(),
            notes.len()
        )
    })
}

/// Builds a payment of `amount` zatoshis to `to`, locks the notes it spends in
/// `state` (caller persists), and returns the raw transaction.
pub fn build_payment(
    state: &mut WalletState,
    me: &DeviceSecret,
    to: orchard::Address,
    amount: u64,
    policy: &Policy,
) -> Result<BuiltPayment> {
    ensure!(amount > 0, "amount must be positive");
    ensure!(state.tip_height > 0, "wallet has never synced; sync while online first");
    let branch = BranchId::try_from(policy.branch_id).map_err(|e| anyhow!("branch id: {e}"))?;
    ensure!(branch == BranchId::Nu7, "only NU7 (v6, Ironwood) is supported");
    let fee = policy.fee_zat;
    let positions = select_notes(state, amount + fee)?;
    let sk = me.spending_key()?;
    let fvk = me.fvk()?;
    let notes: Vec<_> = positions
        .iter()
        .map(|p| state.notes.iter().find(|n| n.position == *p).unwrap().clone())
        .collect();
    let total: u64 = notes.iter().map(|n| n.value).sum();

    let v = BundleVersion::ironwood_v3();
    let mut anchor = None;
    let mut spends = vec![];
    for n in &notes {
        let (path, root) = state.witness(n.position)?;
        if let Some(a) = anchor {
            ensure!(a == root, "witnesses disagree on the anchor");
        }
        anchor = Some(root);
        spends.push((n.note()?, path));
    }
    let anchor_bytes = anchor.unwrap();
    let anchor = Option::from(orchard::Anchor::from_bytes(anchor_bytes)).ok_or_else(|| anyhow!("anchor"))?;
    let mut b = Builder::new(BundleType::DEFAULT, v, v.default_flags(), anchor).map_err(|e| anyhow!("builder: {e:?}"))?;
    for (note, path) in spends {
        b.add_spend(fvk.clone(), note, path).map_err(|e| anyhow!("add_spend: {e:?}"))?;
    }
    let mut memo = [0u8; 512];
    memo[0] = 0xF6; // "no memo"
    b.add_output(Some(fvk.to_ovk(Scope::External)), to, NoteValue::from_raw(amount), memo)
        .map_err(|e| anyhow!("add_output: {e:?}"))?;
    let change = total - amount - fee;
    if change > 0 {
        b.add_output(Some(fvk.to_ovk(Scope::Internal)), fvk.address_at(0u32, Scope::External), NoteValue::from_raw(change), memo)
            .map_err(|e| anyhow!("change output: {e:?}"))?;
    }
    let mut seed = [0u8; 32];
    SysRng.try_fill_bytes(&mut seed).map_err(|e| anyhow!("rng: {e:?}"))?;
    let (bundle, _) = b
        .build::<ZatBalance>(StdRng::from_seed(seed))
        .map_err(|e| anyhow!("build: {e:?}"))?
        .ok_or_else(|| anyhow!("empty bundle"))?;
    let expiry_height = state.tip_height + policy.expiry_delta;

    let mut rng = UnwrapErr(SysRng);
    let t = std::time::Instant::now();
    let proved = bundle.clone().create_proof(proving_key(), &mut rng).map_err(|e| anyhow!("prove: {e:?}"))?;
    let prove_ms = t.elapsed().as_millis();
    let data = TransactionData::<Unauthorized>::from_parts_v6(branch, 0, expiry_height.into(), None, None, None, Some(bundle));
    let sighash = to_txid(TxVersion::V6, branch, &data.digest(TxIdDigester));
    let signed = proved
        .apply_signatures(rng, *sighash.as_ref(), &[SpendAuthorizingKey::from(&sk)])
        .map_err(|e| anyhow!("sign: {e:?}"))?;
    let tx = TransactionData::<Authorized>::from_parts_v6(branch, 0, expiry_height.into(), None, None, None, Some(signed)).freeze()?;
    if tx.txid() != sighash {
        bail!("txid does not match the signed sighash");
    }
    let mut raw = vec![];
    tx.write(&mut raw)?;
    let txid = tx.txid().to_string();
    for n in state.notes.iter_mut().filter(|n| positions.contains(&n.position)) {
        n.locked_by = Some(txid.clone());
    }
    Ok(BuiltPayment {
        raw,
        txid,
        auth_digest: hex::encode(tx.auth_commitment().as_bytes()),
        amount,
        fee,
        expiry_height,
        anchor_height: state.tip_height,
        spent_positions: positions,
        prove_ms,
    })
}
