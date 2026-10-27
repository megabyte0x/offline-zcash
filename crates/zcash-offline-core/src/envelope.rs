//! `ozm1` envelopes: what travels over the mesh.
//!
//! Every app message is sealed to its recipient's X25519 key (ephemeral X25519
//! and ChaCha20-Poly1305) and padded to a multiple of 4 KiB before sealing, so
//! a relay or a packet capture sees only the recipient's mesh ID and a size
//! bucket: no amount, address or transaction bytes.
//!
//! Wire: `b"ozm1"` | version (1) | ephemeral X25519 public key (32) | nonce (12) | ciphertext.
//! Plaintext: u32 LE length | bincode([`Inner`]) | zero padding to 4 KiB multiple.

use anyhow::{anyhow, bail, ensure, Result};
use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit};
use rand::TryRng;
use serde::{Deserialize, Serialize};

use crate::messages::Body;

pub const MAGIC: &[u8; 4] = b"ozm1";
pub const VERSION: u8 = 1;
pub const PAD: usize = 4096;
/// Largest plaintext accepted (protects against memory abuse).
pub const MAX_PLAINTEXT: usize = 512 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Inner {
    /// Sender's device name and Ed25519 key (hex), covered by `Body::Auth`.
    pub from: String,
    pub from_key: String,
    pub body: Body,
}

impl Inner {
    /// Content ID used for de-duplication: BLAKE2b-128 of the encoded body.
    pub fn envelope_id(&self) -> String {
        let bytes = bincode::serialize(&self.body).expect("serialize");
        hex::encode(blake2b_simd::Params::new().hash_length(16).personal(b"ozm1-envelope-id").hash(&bytes).as_bytes())
    }
}

fn kdf(shared: &[u8; 32], eph: &[u8; 32], recipient: &[u8; 32]) -> [u8; 32] {
    let mut st = blake2b_simd::Params::new().hash_length(32).personal(b"ozm1-seal-key\0\0\0").to_state();
    st.update(shared).update(eph).update(recipient);
    st.finalize().as_bytes().try_into().unwrap()
}

pub fn seal(inner: &Inner, recipient_x25519: &[u8; 32]) -> Result<Vec<u8>> {
    let body = bincode::serialize(inner)?;
    ensure!(body.len() + 4 <= MAX_PLAINTEXT, "message too large");
    let message = match &inner.body { Body::Auth { body, .. } => bincode::deserialize(body)?, b => b.clone() };
    let control = !matches!(message, Body::Pay { .. } | Body::Sub { .. } | Body::Rcpt(_));
    let len = body.len() + 4;
    let padded_len = if control {
        match len { 0..=512 => 512, 513..=4096 => 4096, _ => len.div_ceil(16 * 1024) * 16 * 1024 }
    } else { len.div_ceil(PAD) * PAD };
    let mut plain = Vec::with_capacity(padded_len);
    plain.extend_from_slice(&(body.len() as u32).to_le_bytes());
    plain.extend_from_slice(&body);
    plain.resize(padded_len, 0);

    let mut rng = rand::rngs::SysRng;
    let mut eph_bytes = [0u8; 32];
    rng.try_fill_bytes(&mut eph_bytes).map_err(|e| anyhow!("rng: {e:?}"))?;
    let eph = x25519_dalek::StaticSecret::from(eph_bytes);
    let eph_pub = x25519_dalek::PublicKey::from(&eph);
    let shared = eph.diffie_hellman(&x25519_dalek::PublicKey::from(*recipient_x25519));
    if !shared.was_contributory() {
        bail!("recipient key is low order");
    }
    let key = kdf(shared.as_bytes(), eph_pub.as_bytes(), recipient_x25519);
    let mut nonce = [0u8; 12];
    rng.try_fill_bytes(&mut nonce).map_err(|e| anyhow!("rng: {e:?}"))?;
    let ct = ChaCha20Poly1305::new(&key.into())
        .encrypt(&nonce.into(), plain.as_slice())
        .map_err(|_| anyhow!("seal failed"))?;
    let mut out = Vec::with_capacity(4 + 1 + 32 + 12 + ct.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(eph_pub.as_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open(bytes: &[u8], my_secret: &x25519_dalek::StaticSecret) -> Result<Inner> {
    ensure!(bytes.len() > 49 && &bytes[..4] == MAGIC, "not an ozm1 envelope");
    ensure!(bytes[4] == VERSION, "unsupported envelope version {}", bytes[4]);
    let eph: [u8; 32] = bytes[5..37].try_into().unwrap();
    let nonce: [u8; 12] = bytes[37..49].try_into().unwrap();
    let my_pub = x25519_dalek::PublicKey::from(my_secret);
    let shared = my_secret.diffie_hellman(&x25519_dalek::PublicKey::from(eph));
    let key = kdf(shared.as_bytes(), &eph, my_pub.as_bytes());
    let plain = ChaCha20Poly1305::new(&key.into())
        .decrypt(&nonce.into(), &bytes[49..])
        .map_err(|_| anyhow!("cannot open envelope (not for this device or tampered)"))?;
    ensure!(plain.len() >= 4 && plain.len() <= MAX_PLAINTEXT, "bad plaintext size");
    let len = u32::from_le_bytes(plain[..4].try_into().unwrap()) as usize;
    ensure!(4 + len <= plain.len(), "bad length prefix");
    Ok(bincode::deserialize(&plain[4..4 + len])?)
}

/// v0.6 peers authenticate even sync/faucet requests; sealing alone does not authenticate a sender.
pub fn authenticate(mut inner: Inner, me: &crate::DeviceSecret, recipient: &[u8; 32]) -> Result<Inner> {
    let body = bincode::serialize(&inner.body)?;
    let mut bytes = b"ozm1-message".to_vec();
    bytes.extend(bincode::serialize(&(&inner.from, &inner.from_key, recipient, &body))?);
    inner.body = Body::Auth { body, sig: hex::encode(me.sign(&bytes)?) };
    Ok(inner)
}

pub fn authenticated_body(inner: &Inner, sender: &crate::DevicePublic, recipient: &[u8; 32]) -> Result<Body> {
    let Body::Auth { body, sig } = &inner.body else { bail!("authenticated message required") };
    let mut bytes = b"ozm1-message".to_vec();
    bytes.extend(bincode::serialize(&(&inner.from, &inner.from_key, recipient, body))?);
    crate::fleet::verify_sig(&sender.verifying_key()?, &bytes, &hex::decode(sig)?)?;
    let body = bincode::deserialize(body)?;
    ensure!(!matches!(body, Body::Auth { .. }), "nested authentication");
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{messages::Body, DeviceSecret};

    #[test]
    fn seal_open_round_trip_and_padding() {
        let b = DeviceSecret::generate("B", "payee");
        let c = DeviceSecret::generate("C", "payee");
        let inner = Inner {
            from: "A".into(),
            from_key: "00".into(),
            body: Body::Sub { tx: vec![7u8; 9166], txid: "ab".into(), notify: vec![] },
        };
        let sealed = seal(&inner, &b.public().unwrap().x25519().unwrap()).unwrap();
        // 9,166 tx bytes + framing pad to 12 KiB of plaintext (+16 tag + 49 header).
        assert_eq!(sealed.len(), 3 * PAD + 16 + 49);
        // No transaction bytes in the clear.
        assert!(!sealed.windows(64).any(|w| w.iter().all(|x| *x == 7)));
        assert_eq!(open(&sealed, &b.x25519_secret().unwrap()).unwrap(), inner);
        assert!(open(&sealed, &c.x25519_secret().unwrap()).is_err());
        let mut tampered = sealed.clone();
        tampered[100] ^= 1;
        assert!(open(&tampered, &b.x25519_secret().unwrap()).is_err());
    }

    #[test]
    fn authenticated_requests_bind_the_sender_body_and_recipient() {
        let a = DeviceSecret::generate("A", "payer");
        let b = DeviceSecret::generate("B", "gateway");
        let c = DeviceSecret::generate("C", "gateway");
        let recipient = b.public().unwrap().x25519().unwrap();
        let body = Body::FundReq { notes: 3, amount_each: 17_003 };
        let inner = Inner { from: a.name.clone(), from_key: a.public().unwrap().ed25519, body: body.clone() };
        let mut signed = authenticate(inner, &a, &recipient).unwrap();
        assert_eq!(authenticated_body(&signed, &a.public().unwrap(), &recipient).unwrap(), body);
        assert!(authenticated_body(&signed, &c.public().unwrap(), &recipient).is_err());
        assert!(authenticated_body(&signed, &a.public().unwrap(), &c.public().unwrap().x25519().unwrap()).is_err());
        if let Body::Auth { body, .. } = &mut signed.body { body[4] ^= 1; }
        assert!(authenticated_body(&signed, &a.public().unwrap(), &recipient).is_err());
    }

    #[test]
    fn compact_sync_fits_small_buckets_without_changing_payment_padding() {
        let a = DeviceSecret::generate("A", "payer");
        let g = DeviceSecret::generate("G", "gateway");
        let recipient = a.public().unwrap().x25519().unwrap();
        let hash = "ab".repeat(32);
        for (body, padded) in [
            (Body::Tip { height: 4_550_000, hash: hash.clone() }, 512),
            (Body::CompactReq { have_height: 4_550_000, have_hash: hash.clone(), checkpoints: vec![], birthday: 4_465_026 }, 512),
            (Body::CompactPart(crate::sync::Part { id: "ab".repeat(64), index: 0, count: 1, data: vec![7; crate::sync::CHUNK_BYTES] }), 4096),
            (Body::CompactReq { have_height: 4_550_000, have_hash: hash.clone(),
                checkpoints: (4_549_801..=4_550_000).map(|h| (h, hash.clone())).collect(), birthday: 4_465_026 }, 16384),
            (Body::Sub { tx: vec![7; 9166], txid: hash, notify: vec![] }, 12288),
        ] {
            let inner = authenticate(Inner { from: g.name.clone(), from_key: g.public().unwrap().ed25519, body: body.clone() }, &g, &recipient).unwrap();
            let bytes = seal(&inner, &recipient).unwrap();
            assert_eq!(bytes.len(), padded + 65);
            assert_eq!(authenticated_body(&open(&bytes, &a.x25519_secret().unwrap()).unwrap(), &g.public().unwrap(), &recipient).unwrap(), body);
        }
    }

    #[test]
    fn sync_submission_and_faucet_replies_require_a_sender_signature() {
        let sender = DeviceSecret::generate("gateway", "gateway");
        let receiver = DeviceSecret::generate("wallet", "payer");
        let recipient = receiver.public().unwrap().x25519().unwrap();
        for body in [
            Body::SyncReq { have_height: 17, have_hash: "a".into() },
            Body::SyncRes { reset: true, prev_hash: "b".into(), blocks: vec![], chain_tip: 29 },
            Body::FundRes { ok: true, detail: "funded".into() },
            Body::Sub { tx: vec![1, 9, 3], txid: "c".into(), notify: vec!["wallet".into()] },
        ] {
            let inner = Inner { from: sender.name.clone(), from_key: sender.public().unwrap().ed25519, body: body.clone() };
            let sealed_forgery = seal(&inner, &recipient).unwrap();
            let forgery = open(&sealed_forgery, &receiver.x25519_secret().unwrap()).unwrap();
            assert!(authenticated_body(&forgery, &sender.public().unwrap(), &recipient).is_err());
            let mut signed = authenticate(inner, &sender, &recipient).unwrap();
            assert_eq!(authenticated_body(&signed, &sender.public().unwrap(), &recipient).unwrap(), body);
            signed.from = "impersonated".into();
            assert!(authenticated_body(&signed, &sender.public().unwrap(), &recipient).is_err());
        }
    }
}
