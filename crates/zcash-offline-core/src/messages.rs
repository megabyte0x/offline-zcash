//! App messages carried inside `ozm1` envelopes, and their signatures.
//!
//! Kinds in v0.1.0: `pay` (A→B), `rcpt` (B→A), `sub` (A or B → gateway),
//! `res` (gateway → A and B). Statements, receipts and results are signed with
//! the sender's Ed25519 payment key over a domain-separated bincode encoding.

use anyhow::{anyhow, ensure, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    fleet::{verify_sig, Fleet},
    verify::Reject,
    DeviceSecret,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Body {
    Pay { tx: Vec<u8>, statement: Signed<Statement> },
    Rcpt(Signed<Receipt>),
    /// Submit request. `notify` names the devices that should get the gateway's
    /// result (a shielded transaction does not reveal payer or payee).
    Sub { tx: Vec<u8>, txid: String, notify: Vec<String> },
    Res(Signed<GatewayResult>),
    /// v0.2.0: ask a gateway for blocks after `have` (height, hash) so a device with no internet can sync.
    SyncReq { have_height: u32, have_hash: String },
    /// v0.2.0: blocks from the gateway. `reset` means the requester's view is off the gateway's chain
    /// (reorg) and the blocks start at height 1. More blocks follow while `chain_tip` is above the last one.
    SyncRes { reset: bool, prev_hash: String, blocks: Vec<crate::wallet::BlockData>, chain_tip: u32 },
    /// v0.2.0: ask a Regtest gateway for test funds, as `notes` notes of `amount_each` zatoshis.
    FundReq { notes: u32, amount_each: u64 },
    FundRes { ok: bool, detail: String },
    /// Appended variants preserve all v0.5.x discriminants. Older nodes drop unknown variants.
    Presence(Signed<Presence>),
    Auth { body: Vec<u8>, sig: String },
}

impl Body {
    pub fn kind(&self) -> &'static str {
        match self {
            Body::Pay { .. } => "pay",
            Body::Rcpt(_) => "rcpt",
            Body::Sub { .. } => "sub",
            Body::Res(_) => "res",
            Body::SyncReq { .. } => "syncq",
            Body::SyncRes { .. } => "syncr",
            Body::FundReq { .. } => "fundq",
            Body::FundRes { .. } => "fundr",
            Body::Presence(_) => "presence",
            Body::Auth { .. } => "auth",
        }
    }
}

/// Sealed, signed address binding and gateway announcement, with proof of receiving authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Presence {
    pub device: crate::DevicePublic,
    pub gateway: bool,
    pub tip: u32,
    pub created_ms: u64,
    pub address_proof: String,
}

impl Domain for Presence {
    const DOMAIN: &'static [u8] = b"ozm1-presence";
}

/// The payer's signed claim about a payment (pending tier).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Statement {
    pub txid: String,
    pub auth_digest: String,
    pub amount: u64,
    pub fee: u64,
    pub expiry_height: u32,
    pub payer: String,
    pub payee: String,
    /// Per-payer sequence number; a payee refuses a replayed or older seq.
    pub seq: u64,
    pub created_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReceiptState {
    /// Passes all offline checks at the payee's view height; not yet mined.
    Pending,
    Refused(Reject),
    /// No pending acceptance for open/unknown payers; a pinned gateway reports inclusion.
    Included,
}

/// The payee's signed answer, sent only after the payment is fsynced.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Receipt {
    pub txid: String,
    pub auth_digest: String,
    pub payee: String,
    pub amount: u64,
    pub state: ReceiptState,
    pub view_height: u32,
    pub created_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TxStatus {
    Mined { height: u32, block_hash: String },
    Mempool,
    Rejected { reason: String },
}

/// The gateway's signed report ("gateway-reported": not checked against the chain by A or B).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayResult {
    pub txid: String,
    pub status: TxStatus,
    pub tip_height: u32,
    pub gateway: String,
    pub seq: u64,
    pub created_ms: u64,
}

pub trait Domain {
    const DOMAIN: &'static [u8];
}
impl Domain for Statement {
    const DOMAIN: &'static [u8] = b"ozm1-statement";
}
impl Domain for Receipt {
    const DOMAIN: &'static [u8] = b"ozm1-receipt";
}
impl Domain for GatewayResult {
    const DOMAIN: &'static [u8] = b"ozm1-gateway-result";
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Signed<T> {
    pub body: T,
    /// Signer's Ed25519 key (hex); must belong to a fleet device.
    pub signer: String,
    pub sig: String,
}

fn signing_bytes<T: Serialize + Domain>(body: &T) -> Vec<u8> {
    let mut v = T::DOMAIN.to_vec();
    v.extend(bincode::serialize(body).expect("serialize"));
    v
}

impl<T: Serialize + DeserializeOwned + Domain> Signed<T> {
    pub fn sign(body: T, me: &DeviceSecret) -> Result<Self> {
        let sig = me.sign(&signing_bytes(&body))?;
        Ok(Signed { signer: hex::encode(me.signing_key()?.verifying_key().to_bytes()), sig: hex::encode(sig), body })
    }

    /// Checks the signature and that the signer is the named fleet device.
    pub fn verify(&self, fleet: &Fleet, expected_signer: &str) -> Result<()> {
        let dev = fleet.by_ed25519(&self.signer).ok_or_else(|| anyhow!("signer is not in the fleet"))?;
        ensure!(dev.name.eq_ignore_ascii_case(expected_signer), "signed by {} not {expected_signer}", dev.name);
        verify_sig(&dev.verifying_key()?, &signing_bytes(&self.body), &hex::decode(&self.sig)?)
    }

    pub fn verify_key(&self, dev: &crate::DevicePublic) -> Result<()> {
        ensure!(self.signer == dev.ed25519, "wrong signing key");
        verify_sig(&dev.verifying_key()?, &signing_bytes(&self.body), &hex::decode(&self.sig)?)
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}
