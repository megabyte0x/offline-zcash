//! Throwaway feasibility check: 2-of-2 rerandomized FROST authorising an Ironwood v6
//! spend built on Zakura Common 2.2.0 / zakura-pczt 0.1.0-rc4, local NU7 network.
#![allow(deprecated)]

use std::collections::BTreeMap;

use ff::{FromUniformBytes, PrimeField};
use orchard::keys::{FullViewingKey, Scope, SpendValidatingKey, SpendingKey};
use orchard::primitives::redpallas::{self, SpendAuth};
use orchard::tree::MerkleHashOrchard;
use pasta_curves::pallas;
use pczt::roles::{
    creator::Creator, io_finalizer::IoFinalizer, low_level_signer, prover::Prover,
    signer::Signer, tx_extractor::TransactionExtractor,
};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use reddsa::frost::redpallas as frost;
use shardtree::{store::memory::MemoryShardStore, ShardTree};
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::{
    builder::{BuildConfig, Builder, BundlePadding, PcztResult},
    fees::zip317,
};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId},
    local_consensus::LocalNetwork,
    memo::MemoBytes,
    value::Zatoshis,
};

fn prf_expand(k: &[u8], t: &[&[u8]]) -> [u8; 64] {
    let mut h = blake2b_simd::Params::new()
        .hash_length(64)
        .personal(b"Zcash_ExpandSeed")
        .to_state();
    h.update(k);
    for p in t {
        h.update(p);
    }
    *h.finalize().as_array()
}

/// ZIP 2005 `use_qsk = true` FVK for a FROST group key `ak` (unverified against vectors).
fn zip2005_frost_fvk(sk: &SpendingKey, ak: [u8; 32]) -> FullViewingKey {
    let nk: [u8; 32] = FullViewingKey::from(sk).to_bytes()[32..64].try_into().unwrap();
    let qsk = &prf_expand(sk.to_bytes(), &[&[0x0c]])[..32];
    let qk = blake3::derive_key("Zcash ZIP 2005 qk-derivation v1", qsk);
    let rivk_ext = pallas::Scalar::from_uniform_bytes(&prf_expand(&qk, &[&[0x0d], &ak, &nk]));
    let mut b = [0u8; 96];
    b[..32].copy_from_slice(&ak);
    b[32..64].copy_from_slice(&nk);
    b[64..].copy_from_slice(&rivk_ext.to_repr());
    FullViewingKey::from_bytes(&b).expect("valid FVK")
}

fn net() -> LocalNetwork {
    let one = Some(BlockHeight::from_u32(1));
    LocalNetwork {
        overwinter: one, sapling: one, blossom: one, heartwood: one, canopy: one,
        nu5: one, nu6: one, nu6_1: one, nu6_2: one, nu6_3: one, nu7: one,
    }
}

fn main() {
    let mut rng = UnwrapErr(SysRng);
    let mut rng06 = rand_core_06::OsRng;

    // --- 2-of-2 key generation (trusted dealer here; DKG has the same output shape).
    let (shares, pubkeys) = frost::keys::generate_with_dealer(
        2, 2, frost::keys::IdentifierList::Default, &mut rng06,
    ).unwrap();
    let key_packages: BTreeMap<_, _> = shares
        .into_iter()
        .map(|(id, s)| (id, frost::keys::KeyPackage::try_from(s).unwrap()))
        .collect();
    let ak_bytes: [u8; 32] = pubkeys.verifying_key().serialize().unwrap().try_into().unwrap();
    // unstable-frost path (needs unstable-voting-circuits too, see Cargo.toml):
    let _ak = SpendValidatingKey::from_bytes(&ak_bytes).expect("group key is a valid ak");

    // --- Escrow FVK and address. Shared sk gives nk/rivk; ak is the group key.
    let sk = SpendingKey::from_bytes([7; 32]).unwrap();
    let fvk = zip2005_frost_fvk(&sk, ak_bytes);
    assert_eq!(&fvk.to_bytes()[..32], &ak_bytes);
    let addr = fvk.address_at(0u32, Scope::External);
    let ivk = fvk.to_ivk(Scope::External);
    let ovk = fvk.to_ovk(Scope::External);

    // --- Receive an Ironwood note at the escrow address and decrypt it with the ivk.
    let note = {
        let v = orchard::bundle::BundleVersion::ironwood_v3();
        let mut b = orchard::builder::Builder::new(
            orchard::builder::BundleType::DEFAULT, v, v.default_flags(),
            orchard::Anchor::empty_tree(),
        ).unwrap();
        b.add_output(None, addr, orchard::value::NoteValue::from_raw(1_000_000), [0u8; 512]).unwrap();
        let (bundle, meta) = b.build::<i64>(&mut rng).unwrap().unwrap();
        let action = bundle.actions().get(meta.output_action_index(0).unwrap()).unwrap();
        let domain = orchard::note_encryption::IronwoodDomain::for_action(action);
        let (note, _, _) = try_note_decryption(&domain, &ivk.prepare(), action).expect("escrow ivk decrypts");
        note
    };
    let (anchor, path): (orchard::Anchor, orchard::tree::MerklePath) = {
        let cmx: orchard::note::ExtractedNoteCommitment = note.commitment().into();
        let leaf = MerkleHashOrchard::from_cmx(&cmx);
        let mut t = ShardTree::<_, 32, 16>::new(MemoryShardStore::<MerkleHashOrchard, u32>::empty(), 100);
        t.append(leaf, incrementalmerkletree::Retention::Marked).unwrap();
        t.checkpoint(1).unwrap();
        let mp = t.witness_at_checkpoint_depth(0.into(), 0).unwrap().unwrap();
        (mp.root(leaf).into(), mp.into())
    };

    // --- Payer builds the PCZT (NU7, v6, Ironwood only).
    let mut builder = Builder::new(net(), 10.into(), BuildConfig::Standard {
        sapling_anchor: None, orchard_anchor: None, ironwood_anchor: Some(anchor),
        orchard_padding: BundlePadding::DEFAULT, ironwood_padding: BundlePadding::DEFAULT,
    });
    builder.add_ironwood_spend::<zip317::FeeRule>(fvk.clone(), note, path).unwrap();
    builder.add_ironwood_output::<zip317::FeeRule>(Some(ovk), addr, Zatoshis::const_from_u64(990_000), MemoBytes::empty()).unwrap();
    let PcztResult { pczt_parts, ironwood_meta, .. } =
        builder.build_for_pczt(&mut rng, &zip317::FeeRule::standard()).unwrap();
    let pczt = IoFinalizer::new(Creator::build_from_parts(pczt_parts).unwrap()).finalize_io().unwrap();
    let index = ironwood_meta.spend_action_index(0).unwrap();
    let pczt_bytes = pczt.serialize().unwrap();

    // --- Co-signer (payee) side: only PCZT bytes + group ak. No FVK, no note data used.
    let parsed = pczt::Pczt::parse(&pczt_bytes).unwrap();
    let nullifiers: Vec<[u8; 32]> = parsed.ironwood().actions().iter().map(|a| *a.spend().nullifier()).collect();
    let sighash = Signer::new(parsed.clone()).unwrap().shielded_sighash();
    let mut alpha: Option<pallas::Scalar> = None;
    low_level_signer::Signer::new(parsed.clone())
        .sign_ironwood_with(|_, bundle, _| -> Result<(), low_level_signer::OrchardParseError> {
            alpha = *bundle.actions()[index].spend().alpha();
            // rk == ak + [alpha]G binds this action to the escrow key.
            let rk = _ak.randomize(&alpha.unwrap());
            assert_eq!(<[u8; 32]>::from(&rk), <[u8; 32]>::from(bundle.actions()[index].spend().rk()));
            Ok(())
        }).unwrap();
    let alpha = alpha.expect("alpha present after IO finalize");
    println!("branch id in PCZT: {:#010x}", parsed.global().consensus_branch_id());
    println!("sighash: {}", hex::encode(sighash));
    println!("alpha:   {}", hex::encode(alpha.to_repr()));
    println!("nullifiers: {:?}", nullifiers.iter().map(hex::encode).collect::<Vec<_>>());

    // --- 2-of-2 rerandomized FROST signing over (sighash, alpha).
    let mut nonces = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for (id, kp) in &key_packages {
        let (n, c) = frost::round1::commit(kp.signing_share(), &mut rng06);
        nonces.insert(*id, n);
        commitments.insert(*id, c);
    }
    let signing_package = frost::SigningPackage::new(commitments, &sighash);
    let randomizer = frost::rerandomized::Randomizer::from_scalar(alpha);
    let shares: BTreeMap<_, _> = key_packages.iter().map(|(id, kp)| {
        (*id, frost_rerandomized::sign(&signing_package, &nonces[id], kp, randomizer).unwrap())
    }).collect();
    let params = frost::rerandomized::RandomizedParams::from_randomizer(pubkeys.verifying_key(), randomizer);
    let group_sig = frost::rerandomized::aggregate(&signing_package, &shares, &pubkeys, &params).unwrap();
    let sig_bytes: [u8; 64] = group_sig.serialize().unwrap().try_into().unwrap();

    // --- Apply through zakura-pczt (verifies against rk), prove, extract.
    let mut signer = Signer::new(parsed).unwrap();
    signer.apply_ironwood_signature(index, redpallas::Signature::<SpendAuth>::from(sig_bytes)).unwrap();
    let signed = signer.finish();
    let pk = orchard::circuit::ProvingKey::build(orchard::circuit::OrchardCircuitVersion::PostNu6_3);
    let proved = Prover::new(signed).create_ironwood_proof(&pk).unwrap().finish();
    let tx = TransactionExtractor::new(proved).extract().unwrap();

    // --- Independent sighash over the extracted transaction.
    // No transparent inputs: ZIP 244 shielded sighash == txid digest.
    let txid_bytes: [u8; 32] = *tx.txid().as_ref();
    assert_eq!(txid_bytes, sighash, "sighash recomputed from tx (txid) matches PCZT sighash");
    assert_eq!(tx.consensus_branch_id(), BranchId::Nu7);
    let mut raw = vec![];
    tx.write(&mut raw).unwrap();
    println!("extracted tx: version {:?}, branch {:?} ({:#010x}), {} bytes, header {}, txid {}",
        tx.version(), tx.consensus_branch_id(), u32::from(tx.consensus_branch_id()),
        raw.len(), hex::encode(&raw[..8]), tx.txid());
    println!("OK: FROST 2-of-2 aggregate accepted by apply_ironwood_signature and TransactionExtractor");
}
