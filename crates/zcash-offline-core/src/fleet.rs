//! Device keys, the fleet file and policy values.
//!
//! Each device has an Ironwood spending key (funds) and an Ed25519 payment key
//! (statements, receipts, gateway results). The X25519 sealing key is derived
//! from the Ed25519 key, and the mesh peer ID is a hash of the Ed25519 key.

use anyhow::{anyhow, bail, Context, Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use rand::TryRng;
use serde::{Deserialize, Serialize};
use std::path::Path;
use zcash_keys::address::UnifiedAddress;
use zcash_protocol::consensus::{BlockHeight, NetworkType, NetworkUpgrade, Parameters, TEST_NETWORK};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    /// Fleet files and invites without a `network` field are Regtest sessions; keep that default
    /// for loading. New sessions and invite-free joins default to testnet (`Network::NEW_SESSION`).
    #[default]
    Regtest,
    Testnet,
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Self::Regtest => "regtest", Self::Testnet => "testnet" })
    }
}

impl std::str::FromStr for Network {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s { "regtest" => Ok(Self::Regtest), "testnet" => Ok(Self::Testnet), _ => bail!("network must be regtest or testnet") }
    }
}

impl Parameters for Network {
    fn network_type(&self) -> NetworkType {
        match self { Self::Regtest => NetworkType::Regtest, Self::Testnet => NetworkType::Test }
    }
    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        match self { Self::Regtest => crate::regtest_network().activation_height(nu), Self::Testnet => TEST_NETWORK.activation_height(nu) }
    }
}

impl Network {
    /// The network a new session or invite-free identity uses unless the user picks Regtest.
    pub const NEW_SESSION: Network = Network::Testnet;

    pub fn decode_address(self, address: &str) -> Result<orchard::Address> {
        let Some(zcash_keys::address::Address::Unified(ua)) = zcash_keys::address::Address::decode(&self, address) else {
            bail!("expected a {self} unified Zcash address with an Ironwood receiver")
        };
        ua.orchard().copied().ok_or_else(|| anyhow!("address has no Ironwood receiver"))
    }
}

pub fn default_servers() -> Vec<String> {
    vec!["https://testnet.zec.rocks:443".into(), "https://zaino.testnet.unsafe.zec.rocks:443".into()]
}

/// Policy values every device in the fleet applies. Kept in the fleet file so
/// branch ID, expiry and fee are configuration, never hard-coded.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Policy {
    /// Consensus branch ID the payee requires (NU7 = 0x77190AD9).
    pub branch_id: u32,
    /// Blocks between the payer's view of the tip and the transaction expiry.
    pub expiry_delta: u32,
    /// Fee every fleet transaction pays, in zatoshis (2 actions × 5,000).
    pub fee_zat: u64,
    /// Largest single pending-tier payment a payee accepts, in zatoshis.
    pub floor_zat: u64,
    /// Blocks of expiry the payee requires beyond its own view of the tip.
    pub expiry_margin: u32,
    /// How many blocks back from the payee's tip an anchor may be.
    pub anchor_window: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            branch_id: 0x7719_0AD9,
            expiry_delta: 72_576,
            fee_zat: 10_000,
            floor_zat: 50_000_000,
            expiry_margin: 3_456,
            anchor_window: 100,
        }
    }
}

/// What every device knows about every other device.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DevicePublic {
    pub name: String,
    /// "gateway" or empty. Older fleets may hold combinations like "payer+payee"; only gateway counts.
    pub role: String,
    pub peer_id: String,
    pub ed25519: String,
    pub x25519: String,
    /// Unified address with an Ironwood (Orchard-encoded) receiver.
    pub address: String,
}

/// Every device can pay and receive, and every device relays; gateway is the only role with an effect.
pub fn is_gateway_role(role: &str) -> bool {
    role.split('+').any(|r| r == "gateway")
}

impl DevicePublic {
    pub fn is_gateway(&self) -> bool {
        is_gateway_role(&self.role)
    }
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        let b: [u8; 32] = hex::decode(&self.ed25519)?
            .try_into()
            .map_err(|_| anyhow!("ed25519 key length"))?;
        Ok(VerifyingKey::from_bytes(&b)?)
    }
    pub fn x25519(&self) -> Result<[u8; 32]> {
        hex::decode(&self.x25519)?
            .try_into()
            .map_err(|_| anyhow!("x25519 key length"))
    }
    pub fn orchard_address(&self) -> Result<orchard::Address> {
        decode_address(&self.address)
    }
}

/// Decode a supported public identity; payment entry points additionally enforce the fleet network.
pub fn decode_address(address: &str) -> Result<orchard::Address> {
    Network::Regtest.decode_address(address).or_else(|_| Network::Testnet.decode_address(address))
}

/// A device's private keys. Stored in its own file, never in the fleet file.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceSecret {
    pub name: String,
    pub role: String,
    pub ironwood_sk: String,
    pub ed25519_sk: String,
    /// Key creation time in Unix seconds; zero identifies pre-v0.7.4 devices.
    #[serde(default)]
    pub created_at: u64,
}

impl DeviceSecret {
    pub fn generate(name: &str, role: &str) -> Self {
        let mut rng = rand::rngs::SysRng;
        let sk = loop {
            let mut b = [0u8; 32];
            rng.try_fill_bytes(&mut b).expect("os rng");
            if SpendingKey::from_bytes(b).is_some().into() {
                break b;
            }
        };
        let mut ed = [0u8; 32];
        rng.try_fill_bytes(&mut ed).expect("os rng");
        DeviceSecret {
            name: name.into(),
            role: role.into(),
            ironwood_sk: hex::encode(sk),
            ed25519_sk: hex::encode(ed),
            created_at: crate::messages::now_ms() / 1000,
        }
    }

    pub fn spending_key(&self) -> Result<SpendingKey> {
        let b: [u8; 32] = hex::decode(&self.ironwood_sk)?
            .try_into()
            .map_err(|_| anyhow!("spending key length"))?;
        Option::from(SpendingKey::from_bytes(b)).ok_or_else(|| anyhow!("invalid spending key"))
    }
    pub fn fvk(&self) -> Result<FullViewingKey> {
        Ok(FullViewingKey::from(&self.spending_key()?))
    }
    pub fn signing_key(&self) -> Result<SigningKey> {
        let b: [u8; 32] = hex::decode(&self.ed25519_sk)?
            .try_into()
            .map_err(|_| anyhow!("ed25519 key length"))?;
        Ok(SigningKey::from_bytes(&b))
    }
    /// X25519 secret derived from the Ed25519 key (owner decision, 26 October).
    pub fn x25519_secret(&self) -> Result<x25519_dalek::StaticSecret> {
        Ok(x25519_dalek::StaticSecret::from(self.signing_key()?.to_scalar_bytes()))
    }
    pub fn address(&self) -> Result<orchard::Address> {
        Ok(self.fvk()?.address_at(0u32, Scope::External))
    }

    pub fn public(&self) -> Result<DevicePublic> {
        self.public_on(Network::Regtest)
    }

    pub fn public_on(&self, network: Network) -> Result<DevicePublic> {
        let sig = self.signing_key()?;
        let vk = sig.verifying_key();
        let ua = UnifiedAddress::from_receivers(Some(self.address()?), None, None)
            .ok_or_else(|| anyhow!("cannot build unified address"))?;
        Ok(DevicePublic {
            name: self.name.clone(),
            role: self.role.clone(),
            peer_id: peer_id_for(&vk),
            ed25519: hex::encode(vk.to_bytes()),
            x25519: hex::encode(vk.to_montgomery().to_bytes()),
            address: ua.encode(&network),
        })
    }

    pub fn sign(&self, msg: &[u8]) -> Result<[u8; 64]> {
        Ok(self.signing_key()?.sign(msg).to_bytes())
    }
}

/// Mesh peer ID: the first 8 bytes of BLAKE2b("ozm1-peer-id", ed25519 key), hex.
pub fn peer_id_for(vk: &VerifyingKey) -> String {
    let h = blake2b_simd::Params::new()
        .hash_length(8)
        .personal(b"ozm1-peer-id\0\0\0\0")
        .hash(vk.as_bytes());
    hex::encode(h.as_bytes())
}

pub fn verify_sig(vk: &VerifyingKey, msg: &[u8], sig: &[u8]) -> Result<()> {
    let sig: [u8; 64] = sig.try_into().map_err(|_| anyhow!("signature length"))?;
    vk.verify_strict(msg, &ed25519_dalek::Signature::from_bytes(&sig))
        .map_err(|_| anyhow!("bad signature"))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fleet {
    pub name: String,
    #[serde(default)]
    pub network: Network,
    /// Optional scan floor for newly generated devices. Existing wallets keep their birthday.
    #[serde(default)]
    pub birthday: u32,
    #[serde(default = "default_servers")]
    pub servers: Vec<String>,
    pub policy: Policy,
    pub devices: Vec<DevicePublic>,
    /// Locally generated identity, with self-signed gateway discovery instead of an organiser allowlist.
    #[serde(default)]
    pub open: bool,
    /// Receiving-authority proofs allow invited receipt routing before live discovery.
    #[serde(default)]
    pub address_proofs: std::collections::BTreeMap<String, String>,
}

impl Fleet {
    pub fn load(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Ok(serde_json::from_str(&s)?)
    }
    pub fn by_name(&self, name: &str) -> Result<&DevicePublic> {
        self.devices
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| anyhow!("no device named {name} in the fleet"))
    }
    pub fn by_peer(&self, peer_id: &str) -> Option<&DevicePublic> {
        self.devices.iter().find(|d| d.peer_id == peer_id)
    }
    pub fn by_ed25519(&self, key_hex: &str) -> Option<&DevicePublic> {
        self.devices.iter().find(|d| d.ed25519 == key_hex)
    }
    pub fn gateways(&self) -> impl Iterator<Item = &DevicePublic> {
        self.devices.iter().filter(|d| d.is_gateway())
    }
}

/// Generates a fleet: one secret per (name, role), and the shared fleet file.
pub fn generate_fleet(name: &str, devices: &[(String, String)], policy: Policy) -> Result<(Fleet, Vec<DeviceSecret>)> {
    generate_fleet_on(name, devices, policy, Network::Regtest, 1)
}

pub fn generate_fleet_on(name: &str, devices: &[(String, String)], policy: Policy, network: Network, birthday: u32) -> Result<(Fleet, Vec<DeviceSecret>)> {
    if devices.is_empty() {
        bail!("no devices");
    }
    anyhow::ensure!(network != Network::Testnet || birthday == 0 || birthday >= 4_465_026, "testnet birthday must be at or after NU7 (4465026)");
    let secrets: Vec<_> = devices.iter().map(|(n, r)| DeviceSecret::generate(n, r)).collect();
    let publics = secrets.iter().map(|s| s.public_on(network)).collect::<Result<Vec<_>>>()?;
    let address_proofs = secrets.iter().map(|s| Ok((s.public_on(network)?.peer_id, crate::binding::prove_on(s, network)?))).collect::<Result<_>>()?;
    Ok((Fleet { name: name.into(), network, birthday, servers: default_servers(), policy, devices: publics, open: false, address_proofs }, secrets))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derived_keys_agree() {
        let s = DeviceSecret::generate("A", "payer");
        let p = s.public().unwrap();
        // The X25519 public key derived from the Ed25519 public key matches the secret's.
        let derived = x25519_dalek::PublicKey::from(&s.x25519_secret().unwrap());
        assert_eq!(hex::encode(derived.as_bytes()), p.x25519);
        assert_eq!(p.orchard_address().unwrap(), s.address().unwrap());
        assert_eq!(p.peer_id.len(), 16);
        let sig = s.sign(b"hello").unwrap();
        verify_sig(&p.verifying_key().unwrap(), b"hello", &sig).unwrap();
        assert!(verify_sig(&p.verifying_key().unwrap(), b"hellp", &sig).is_err());
    }
}
