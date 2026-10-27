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
    fees::{zip317, FeeRule},
    txid::{to_txid, TxIdDigester},
    Authorized, TransactionData, TxVersion, Unauthorized,
};
use zcash_protocol::{consensus::BranchId, value::ZatBalance};

use crate::{fleet::Policy, wallet::WalletState, DeviceSecret};

pub fn proving_key() -> &'static ProvingKey {
    static PK: OnceLock<ProvingKey> = OnceLock::new();
    PK.get_or_init(|| ProvingKey::build(OrchardCircuitVersion::PostNu6_3).expect("supported Ironwood circuit"))
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

/// Keeps the old smallest single/pair choice, then grows an input set at its actual fee.
fn select_notes(state: &WalletState, amount: u64, policy: &Policy) -> Result<(Vec<u64>, u64)> {
    let mut notes: Vec<_> = state.notes.iter().filter(|n| n.spendable()).collect();
    notes.sort_by_key(|n| n.value);
    let (positions, max, max_fee) = max_notes(state, policy)?;
    if amount == max && max > 0 {
        return Ok((positions, max_fee));
    }
    let fee = transaction_fee(2, policy)?;
    let need = amount.checked_add(fee).ok_or_else(|| anyhow!("amount overflow"))?;
    if let Some(n) = notes.iter().find(|n| n.value >= need) {
        return Ok((vec![n.position], fee));
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
    if let Some((_, a, b)) = best {
        return Ok((vec![a, b], fee));
    }
    let mut positions = Vec::new();
    let mut total = 0;
    for n in notes.iter().rev() {
        positions.push(n.position);
        total += n.value;
        let fee = transaction_fee(positions.len(), policy)?;
        if amount.checked_add(fee).is_some_and(|need| total >= need) {
            return Ok((positions, fee));
        }
    }
    bail!("not enough spendable notes for {amount} zat plus the ZIP 317 fee; unmined change cannot be spent offline")
}

/// The same padded Ironwood action count used by Builder::build. One recipient and optional change
/// both fit the minimum two actions; more inputs increase the fee. Policy fee is a configurable floor.
pub fn transaction_fee(inputs: usize, policy: &Policy) -> Result<u64> {
    let v = BundleVersion::ironwood_v3();
    let actions = BundleType::DEFAULT.num_actions(v.default_flags(), inputs, 2).map_err(|e| anyhow!(e))?;
    let fee = zip317::FeeRule::standard().fee_required(
        &crate::regtest_network(), 1.into(), [], [], 0, 0, 0, actions,
    ).map_err(|e| anyhow!("fee: {e:?}"))?;
    Ok(u64::from(fee).max(policy.fee_zat))
}

/// A non-mutating quote; excludes unmined change, locked notes and uneconomic inputs.
pub fn max_payment(state: &WalletState, policy: &Policy) -> Result<(u64, u64)> {
    let (_, amount, fee) = max_notes(state, policy)?;
    Ok((amount, fee))
}

fn max_notes(state: &WalletState, policy: &Policy) -> Result<(Vec<u64>, u64, u64)> {
    let mut notes: Vec<_> = state.notes.iter().filter(|n| n.spendable()).collect();
    notes.sort_by_key(|n| std::cmp::Reverse(n.value));
    let mut best = (0, 0, transaction_fee(0, policy)?);
    let mut total = 0;
    for (i, n) in notes.iter().enumerate() {
        total += n.value;
        let fee = transaction_fee(i + 1, policy)?;
        let amount = total.saturating_sub(fee);
        // At each input count the largest notes maximize value; ties prefer fewer inputs.
        if amount > best.1 {
            best = (i + 1, amount, fee);
        }
    }
    Ok((notes[..best.0].iter().map(|n| n.position).collect(), best.1, best.2))
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
    let (positions, fee) = select_notes(state, amount, policy)?;
    build_selected(state, me, &[(to, amount)], positions, fee, policy)
}

/// Turn one mined note into equal self outputs, with the actual action-count fee.
pub fn prepare_offline(state: &mut WalletState, me: &DeviceSecret, count: u32, value: u64, policy: &Policy) -> Result<BuiltPayment> {
    ensure!((2..=100).contains(&count) && value > 0, "prepare needs 2–100 positive notes");
    let amount = value.checked_mul(count as u64).ok_or_else(|| anyhow!("split amount overflow"))?;
    let fee_without_change = transaction_fee(count as usize, policy)?;
    let fee_with_change = transaction_fee(count as usize + 1, policy)?;
    let note = state.notes.iter().filter(|n| n.spendable()).filter_map(|n| {
        let fee = if n.value.checked_sub(amount) == Some(fee_without_change) { fee_without_change } else { fee_with_change };
        amount.checked_add(fee).filter(|need| n.value >= *need).map(|_| (n, fee))
    }).min_by_key(|(n, _)| n.value).ok_or_else(|| anyhow!("no single mined note covers the split plus fee"))?;
    build_selected(state, me, &vec![(me.address()?, value); count as usize], vec![note.0.position], note.1, policy)
}

fn build_selected(state: &mut WalletState, me: &DeviceSecret, outputs: &[(orchard::Address, u64)], positions: Vec<u64>, fee: u64, policy: &Policy) -> Result<BuiltPayment> {
    let amount = outputs.iter().map(|(_, value)| value).sum::<u64>();
    ensure!(state.tip_height > 0, "wallet has never synced; sync while online first");
    let branch = BranchId::try_from(policy.branch_id).map_err(|e| anyhow!("branch id: {e}"))?;
    ensure!(branch == BranchId::Nu7, "only NU7 (v6, Ironwood) is supported");
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
    for (to, value) in outputs {
        b.add_output(Some(fvk.to_ovk(Scope::External)), *to, NoteValue::from_raw(*value), memo)
            .map_err(|e| anyhow!("add_output: {e:?}"))?;
    }
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
    ensure!(fee == transaction_fee(bundle.actions().len(), policy)?, "planned fee differs from built action count");
    let expiry_height = state.tip_height.checked_add(policy.expiry_delta).ok_or_else(|| anyhow!("expiry overflow"))?;

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
    state.lock_notes(&positions, &txid, expiry_height);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::StoredNote;

    #[test]
    fn max_uses_the_builder_fee_and_ignores_locked_and_spent_notes() {
        let policy = Policy::default();
        let mut state = WalletState::default();
        for (i, value) in [13_001, 25_002, 39_003, 51_004, 90_000].into_iter().enumerate() {
            state.notes.push(StoredNote { position: i as u64, value, height: 1, txid: String::new(), recipient: String::new(),
                rho: String::new(), rseed: String::new(), nullifier: String::new(), spent_in: None, locked_by: None });
        }
        state.notes[3].locked_by = Some("pending".into());
        state.notes[4].spent_in = Some("mined".into());
        assert_eq!(transaction_fee(1, &policy).unwrap(), 10_000);
        assert_eq!(transaction_fee(2, &policy).unwrap(), 10_000);
        assert_eq!(max_payment(&state, &policy).unwrap(), (62_006, 15_000));
        // A quote is non-mutating. A dust input must not reduce the maximum sendable amount.
        assert_eq!(state.balance(), (77_006, 51_004));
        state.notes[0].value = 1;
        assert_eq!(max_payment(&state, &policy).unwrap(), (54_005, 10_000));
    }
}
