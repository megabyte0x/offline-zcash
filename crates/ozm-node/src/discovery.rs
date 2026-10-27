//! Open mesh bootstrap. Only public routing keys are broadcast; presence (including addresses,
//! gateway role and tip) is signed and individually sealed. Fleet keys retain their authority.
use anyhow::{anyhow, ensure, Result};
use ozm_mesh::PeerId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use zcash_offline_core::{
    fleet::peer_id_for,
    messages::{now_ms, Body, Presence, Signed},
    DevicePublic,
};

use crate::App;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct TrustPins {
    pub gateway: Option<DevicePublic>,
    pub addresses: std::collections::BTreeMap<String, String>,
}

const KEY_MAGIC: &[u8] = b"ozmk";
const LIVE_MS: u64 = 3 * 60 * 60 * 1000;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Seen {
    pub announcement: Signed<Presence>,
    pub hops: u8,
}

impl App {
    pub(crate) fn device(&self, name: &str) -> Result<DevicePublic> {
        if let Some(d) = self.cfg.fleet.by_peer(name) {
            return Ok(d.clone());
        }
        if let Some(s) = self.peers.lock().unwrap().get(name) {
            return Ok(s.announcement.body.device.clone());
        }
        self.cfg.fleet.devices.iter().find(|d| d.name.eq_ignore_ascii_case(name) || d.address == name).cloned()
            .ok_or_else(|| anyhow!("unknown mesh device {name}"))
    }

    pub(crate) fn route_name(&self, d: &DevicePublic) -> String {
        // Fleet names are the legacy wire contract. Unenrolled names are display-only.
        self.cfg.fleet.by_peer(&d.peer_id).map(|k| k.name.clone()).unwrap_or_else(|| d.peer_id.clone())
    }

    pub(crate) fn receipt_device(&self, address: &str) -> Option<DevicePublic> {
        let mut devices: Vec<_> = self.peers.lock().unwrap().values()
            .filter(|s| now_ms().saturating_sub(s.announcement.body.created_ms) < LIVE_MS)
            .map(|s| s.announcement.body.device.clone()).filter(|d| d.address == address).collect();
        devices.extend(self.cfg.fleet.devices.iter().filter(|d| d.address == address && self.cfg.fleet.address_proofs.get(&d.peer_id)
            .is_some_and(|proof| zcash_offline_core::binding::verify(d, proof).is_ok())).cloned());
        devices.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
        devices.dedup_by(|a, b| a.peer_id == b.peer_id);
        if devices.len() != 1 { return None; }
        let d = devices.remove(0);
        let mut pins = self.trust.lock().unwrap();
        if pins.addresses.get(address).is_some_and(|key| key != &d.ed25519) { return None; }
        pins.addresses.insert(address.to_string(), d.ed25519.clone());
        self.save_trust(&pins).ok()?;
        Some(d)
    }

    fn save_trust(&self, pins: &TrustPins) -> Result<()> {
        zcash_offline_core::wallet::write_atomic(&self.cfg.data.join("trust.json"), &serde_json::to_vec_pretty(pins)?)
    }

    pub(crate) fn pin_gateway(&self, d: &DevicePublic) -> Result<()> {
        let mut pins = self.trust.lock().unwrap();
        ensure!(pins.gateway.as_ref().is_none_or(|g| g.ed25519 == d.ed25519), "gateway changed; automatic switching blocked");
        pins.gateway = Some(d.clone());
        self.save_trust(&pins)
    }

    pub(crate) fn selected_gateway(&self) -> Option<Seen> {
        let now = now_ms();
        let pin = self.trust.lock().unwrap().gateway.clone();
        self.peers.lock().unwrap().values().filter(|s| {
            let p = &s.announcement.body;
            p.gateway && now.saturating_sub(p.created_ms) < LIVE_MS
                && pin.as_ref().is_none_or(|g| g.ed25519 == p.device.ed25519)
                && (self.cfg.fleet.open || self.cfg.fleet.gateways().any(|d| d.ed25519 == p.device.ed25519))
        }).min_by_key(|s| (s.hops, std::cmp::Reverse(s.announcement.body.tip), s.announcement.body.device.peer_id.clone())).cloned()
    }

    pub(crate) async fn announce(&self) -> Result<()> {
        if let Some(url) = self.gateway_options().0.filter(|_| self.cfg.fleet.network == zcash_offline_core::fleet::Network::Regtest) {
            if let Ok(Ok(tip)) = tokio::task::spawn_blocking(move || zcash_offline_core::rpc::Rpc::new(&url).tip()).await {
                self.reported_tip.store(tip, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let mut key = KEY_MAGIC.to_vec();
        key.extend_from_slice(self.cfg.me.signing_key()?.verifying_key().as_bytes());
        // Each listener replies with fresh sealed presence; do not also send it a second time.
        // Stored key broadcasts bootstrap new links through the mesh inventory exchange.
        self.mesh.broadcast(key).await?;
        Ok(())
    }

    async fn send_presence(&self, to: &DevicePublic) -> Result<()> {
        let body = Presence {
            device: self.cfg.me.public_on(self.cfg.fleet.network)?,
            gateway: self.gateway_options().0.is_some(),
            tip: if self.gateway_options().0.is_some() { self.reported_tip.load(std::sync::atomic::Ordering::Relaxed) }
                else { self.state.lock().unwrap().0.tip_height },
            created_ms: now_ms(),
            address_proof: zcash_offline_core::binding::prove_on(&self.cfg.me, self.cfg.fleet.network)?,
        };
        self.send_device(to, Body::Presence(Signed::sign(body, &self.cfg.me)?)).await
    }

    pub(crate) async fn on_key(&self, from: PeerId, payload: &[u8]) -> Result<()> {
        ensure!(payload.len() == 36 && payload.starts_with(KEY_MAGIC), "not a routing key");
        let mut dev = DevicePublic {
            name: from.to_string(), role: String::new(), peer_id: from.to_string(),
            ed25519: hex::encode(&payload[4..]), x25519: String::new(), address: String::new(),
        };
        let vk = dev.verifying_key()?;
        ensure!(peer_id_for(&vk) == from.to_string(), "routing key does not match mesh ID");
        dev.x25519 = hex::encode(vk.to_montgomery().to_bytes());
        self.send_presence(&dev).await
    }

    pub(crate) fn on_presence(&self, from: PeerId, p: Signed<Presence>, hops: u8) -> Result<()> {
        let d = &p.body.device;
        p.verify_key(d)?;
        let vk = d.verifying_key()?;
        ensure!(peer_id_for(&vk) == from.to_string() && d.peer_id == from.to_string(), "presence mesh ID mismatch");
        ensure!(d.x25519 == hex::encode(vk.to_montgomery().to_bytes()), "presence sealing key mismatch");
        ensure!(!d.name.is_empty() && d.name.len() <= 64 && d.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'), "invalid device name");
        zcash_offline_core::binding::verify(d, &p.body.address_proof)?;
        self.cfg.fleet.network.decode_address(&d.address)?;
        ensure!(self.trust.lock().unwrap().addresses.get(&d.address).is_none_or(|key| key == &d.ed25519), "address binding changed; pinned identity retained");
        let now = now_ms();
        ensure!(p.body.created_ms <= now.saturating_add(5_000) && now.saturating_sub(p.body.created_ms) < LIVE_MS, "stale presence");
        if let Some(known) = self.cfg.fleet.by_peer(&d.peer_id) {
            ensure!(known == d, "presence changed an invited identity");
        }
        let mut peers = self.peers.lock().unwrap();
        if let Some(old) = peers.get(&d.peer_id) {
            ensure!(p.body.created_ms > old.announcement.body.created_ms, "replayed presence");
        }
        // Bound unsolicited identities; old advertisements expire rather than persisting untrusted keys.
        peers.retain(|_, s| now.saturating_sub(s.announcement.body.created_ms) < LIVE_MS);
        ensure!(peers.len() < 1024 || peers.contains_key(&d.peer_id), "discovery directory full");
        peers.insert(d.peer_id.clone(), Seen { announcement: p, hops });
        Ok(())
    }

    pub(crate) fn gateway_status(&self) -> serde_json::Value {
        self.selected_gateway().map(|s| json!({
            "name": s.announcement.body.device.name, "peer_id": s.announcement.body.device.peer_id,
            "hops": s.hops, "tip": s.announcement.body.tip, "trust": "gateway-reported",
        })).unwrap_or(serde_json::Value::Null)
    }

    pub(crate) fn gateway_warning(&self) -> String {
        let pin = self.trust.lock().unwrap().gateway.clone();
        if let Some(pin) = pin {
            if self.selected_gateway().is_none() {
                return format!("Pinned gateway {} ({}) unavailable; automatic gateway changes are blocked. Verify the operator before resetting data/trust.json.", pin.name, pin.peer_id);
            }
        }
        if self.cfg.fleet.open { "First gateway use is trust-on-first-use; a nearer self-signed Sybil can win. Verify its mesh ID with the operator.".into() }
        else { String::new() }
    }
}
