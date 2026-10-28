//! Node roles over the mesh: payer (A), relay (R), payee (B) and gateway (D).
//!
//! Flow: A builds and seals `pay` to B and `sub` to each gateway. B checks the
//! payment offline, fsyncs it, then sends a signed `rcpt` to A and `sub` to the
//! gateways. A gateway checks proofs, submits, mines on Regtest, and sends a
//! signed `res` to A and B. A relay only forwards mesh packets.

use anyhow::{anyhow, bail, Context, Result};
use ozm_mesh::{Mesh, MeshEvent, PeerId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{info, warn};
use zcash_offline_core::{
    builder,
    envelope::{self, Inner},
    messages::{now_ms, Body, GatewayResult, Receipt, ReceiptState, Signed, Statement, TxStatus},
    rpc::{Rpc, RpcError},
    verify::{self, code, Reject},
    wallet::{write_atomic, BlockData, ChunkSource, WalletState},
    DevicePublic, DeviceSecret, Fleet,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub ms: u64,
    pub event: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Payment {
    pub txid: String,
    pub auth_digest: String,
    /// "sent" (this device paid) or "received" (this device was paid).
    pub direction: String,
    pub counterparty: String,
    pub amount: u64,
    pub fee: u64,
    pub expiry_height: u32,
    /// sent: built → sent → delivered → accepted-pending | refused → mined
    /// received: accepted-pending → submitted → mined
    pub state: String,
    pub tx_hex: String,
    pub nullifiers: Vec<String>,
    pub statement: Option<Signed<Statement>>,
    pub receipt: Option<Signed<Receipt>>,
    pub result: Option<Signed<GatewayResult>>,
    pub history: Vec<HistoryEntry>,
    pub created_ms: u64,
    #[serde(default)]
    pub last_send_ms: u64,
    #[serde(default)]
    pub attempts: u32,
    /// Set on debug payments deliberately tampered for tests; never sent to a gateway.
    #[serde(default)]
    pub test_tamper: Option<String>,
    /// Absent when the destination address has no unique mesh binding.
    #[serde(default)]
    pub receipt_peer: Option<String>,
}

impl Payment {
    fn note(&mut self, event: impl Into<String>) {
        self.history.push(HistoryEntry { ms: now_ms(), event: event.into() });
    }
    fn is_final(&self) -> bool {
        matches!(self.state.as_str(), "mined" | "expired" | "conflict")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refusal {
    pub ms: u64,
    pub from: String,
    pub txid: String,
    pub reject: Reject,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayEntry {
    pub txid: String,
    pub notify: Vec<String>,
    pub result: Signed<GatewayResult>,
    pub submissions: u32,
    #[serde(default)]
    pub tx_hex: String,
    #[serde(default)]
    pub expiry_height: u32,
    #[serde(default)]
    pub last_submit_height: u32,
    #[serde(default)]
    pub submitter: String,
    #[serde(default)]
    pub enrolled: bool,
}

/// Everything this device persists besides the wallet.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Book {
    pub payments: BTreeMap<String, Payment>,
    pub refusals: Vec<Refusal>,
    pub gateway: BTreeMap<String, GatewayEntry>,
    #[serde(default)]
    pub request_nonces: BTreeMap<String, String>,
    pub seq_out: u64,
    pub last_seq_in: BTreeMap<String, u64>,
    #[serde(default)]
    pub gw_seq: u64,
    #[serde(default)]
    pub faucet_last: BTreeMap<String, u64>,
}

const RESULT_MAGIC: &[u8] = b"ozm1-result:";

/// Compact blocks per reply to a wallet's mesh sync request (sized for slow links).
pub const MESH_SYNC_BLOCKS: u32 = 200;
/// Compact blocks per batch when an online device scans for itself. Larger batches skip work for
/// checkpoints that pruning discards; 2,000 busy testnet blocks stay well under the 8 MiB limit.
pub const SCAN_BLOCKS: u32 = 2_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GatewayNotice {
    request_nonce: String,
    result: Signed<GatewayResult>,
}

impl zcash_offline_core::messages::Domain for GatewayNotice {
    const DOMAIN: &'static [u8] = b"ozm1-gateway-notice-v1";
}

/// The submission a gateway is processing, and targets of identical submissions that joined it.
struct InFlightSub {
    txid: String,
    raw: Vec<u8>,
    joined: Vec<String>,
}

pub struct Config {
    pub data: PathBuf,
    pub fleet: Fleet,
    pub me: DeviceSecret,
    pub rpc: Option<String>,
    pub mine: bool,
    pub retry: Duration,
    /// Gateway: largest test-fund request served (notes × amount), in zatoshis; 0 disables the faucet.
    pub faucet_max_zat: u64,
}

/// Callback for app events: name and JSON detail.
pub type EventHook = Box<dyn Fn(&str, &serde_json::Value) + Send + Sync>;

pub struct App {
    pub cfg: Config,
    pub mesh: Mesh,
    // Shared by every Basecamp app; changing RPC settings must not start another mesh.
    online: Mutex<(Option<String>, bool)>,
    pub(crate) ble: std::sync::OnceLock<ozm_ble::Ble>,
    pub x25519: x25519_dalek::StaticSecret,
    pub state: Mutex<(WalletState, Book)>,
    pub(crate) peers: Mutex<BTreeMap<String, crate::discovery::Seen>>,
    pub(crate) trust: Mutex<crate::discovery::TrustPins>,
    pub(crate) reported_tip: std::sync::atomic::AtomicU32,
    /// Serializes proving so two pays never pick the same note.
    pay_lock: tokio::sync::Mutex<()>,
    /// Gateway: blocks already read from the node, by height (checked against the node's hash).
    block_cache: Mutex<BTreeMap<u32, BlockData>>,
    /// Device: when the last mesh sync request went out (ms), to avoid flooding the gateway.
    last_sync_req: std::sync::atomic::AtomicU64,
    sync_backoff: std::sync::atomic::AtomicU64,
    sync_apply: tokio::sync::Mutex<()>,
    compact_assembly: Mutex<zcash_offline_core::sync::Assembly>,
    pub(crate) sync_sources: Mutex<Vec<String>>,
    pub(crate) chain_view: Mutex<(u32, String)>,
    last_tip_push: Mutex<(u64, u32, String)>,
    gateway_apply: tokio::sync::Mutex<()>,
    sub_rates: Mutex<BTreeMap<String, (f64, std::time::Instant)>>,
    /// The submission `gateway_apply` is processing (txid, bytes) and the notification targets of
    /// identical submissions that arrived meanwhile; they receive its result too.
    sub_in_flight: Mutex<Option<InFlightSub>>,
    /// Optional observer of every app event (the Basecamp module turns them into Logos events).
    pub on_event: std::sync::OnceLock<EventHook>,
    /// One writer at a time, so concurrent events never interleave inside a line of events.jsonl.
    events_log: Mutex<()>,
    /// Set by `stop`: no file in `cfg.data` is written afterwards, so the folder can move away.
    stopped: std::sync::atomic::AtomicBool,
}

fn wallet_path(cfg: &Config) -> PathBuf {
    cfg.data.join("wallet.json")
}
fn book_path(cfg: &Config) -> PathBuf {
    cfg.data.join("payments.json")
}

impl App {
    pub fn new(cfg: Config, mesh: Mesh) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&cfg.data)?;
        #[cfg(unix)]
        for name in ["payments.json", "events.jsonl", "trust.json"] {
            use std::os::unix::fs::PermissionsExt;
            let path = cfg.data.join(name);
            if path.exists() { std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?; }
        }
        let mut wallet = WalletState::load(&wallet_path(&cfg))?;
        let mut book: Book = match std::fs::read(book_path(&cfg)) {
            Ok(b) => serde_json::from_slice(&b)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Book::default(),
            Err(e) => return Err(e.into()),
        };
        for p in book.payments.values_mut().filter(|p| p.direction == "sent" && p.receipt_peer.is_none()) {
            if cfg.fleet.by_name(&p.counterparty).is_ok() {
                p.receipt_peer = Some(p.counterparty.clone());
            }
        }
        for note in &wallet.notes {
            if let Some(txid) = &note.locked_by {
                let expiry = book.payments.get(txid).map_or(0, |p| p.expiry_height);
                wallet.reservations.entry(note.nullifier.clone()).or_insert((txid.clone(), expiry));
            }
        }
        let x25519 = cfg.me.x25519_secret()?;
        let online = Mutex::new((cfg.rpc.clone(), cfg.mine));
        let trust = match std::fs::read(cfg.data.join("trust.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Arc::new(App { cfg, mesh, online, x25519, state: Mutex::new((wallet, book)),
            peers: Default::default(),
            trust: Mutex::new(trust),
            reported_tip: Default::default(),
            ble: Default::default(),
            pay_lock: Default::default(),
            block_cache: Default::default(),
            last_sync_req: Default::default(),
            sync_backoff: std::sync::atomic::AtomicU64::new(1),
            sync_apply: Default::default(),
            compact_assembly: Default::default(),
            sync_sources: Default::default(),
            chain_view: Default::default(),
            last_tip_push: Default::default(),
            gateway_apply: Default::default(),
            sub_rates: Default::default(),
            sub_in_flight: Default::default(),
            on_event: Default::default(),
            events_log: Default::default(),
            stopped: Default::default(),
        }))
    }

    pub fn name(&self) -> &str {
        &self.cfg.me.name
    }

    pub fn gateway_options(&self) -> (Option<String>, bool) {
        self.online.lock().unwrap().clone()
    }

    /// Enables or updates the gateway on the existing identity, wallet and mesh.
    pub fn configure_gateway(&self, rpc: String, mine: bool) -> Result<()> {
        anyhow::ensure!(self.cfg.fleet.by_name(self.name())?.is_gateway(),
            "this invite does not grant the gateway role; ask the organiser for a gateway invite");
        anyhow::ensure!(self.cfg.fleet.network != zcash_offline_core::fleet::Network::Testnet || !mine, "mining is Regtest-only");
        anyhow::ensure!(!rpc.trim().is_empty(), "chain RPC URL is required");
        if self.cfg.fleet.network == zcash_offline_core::fleet::Network::Regtest { crate::regtest_rpc_url(&rpc)?; }
        *self.online.lock().unwrap() = (Some(rpc), mine);
        Ok(())
    }

    /// Callers hold `state`, which `stop` also takes, so no write can follow a stop.
    fn persist(&self, wallet: &WalletState, book: &Book) -> Result<()> {
        anyhow::ensure!(!self.is_stopped(), "device stopped");
        wallet.save(&wallet_path(&self.cfg))?;
        write_atomic(&book_path(&self.cfg), &serde_json::to_vec_pretty(book)?)
    }

    /// Appends a structured event to `events.jsonl` and the log. The bench reads these for timings.
    pub fn event(&self, event: &str, detail: serde_json::Value) {
        if self.is_stopped() {
            return;
        }
        if let Some(f) = self.on_event.get() {
            f(event, &detail);
        }
        let line = json!({"ms": now_ms(), "node": self.name(), "event": event, "detail": detail});
        info!(target: "ozm", "{line}");
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        // `writeln!` would issue several writes; build the line and append it in one call.
        let line = format!("{line}\n");
        let _log = self.events_log.lock().unwrap();
        if let Ok(mut f) = options.open(self.cfg.data.join("events.jsonl")) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Stops the mesh, lets an in-flight payment or wallet update finish, then ends every write to
    /// `cfg.data`. Afterwards the device folder can be moved: late background tasks fail instead of
    /// recreating files there.
    pub async fn stop(&self) {
        self.mesh.shutdown();
        let _pay = self.pay_lock.lock().await;
        let _sync = self.sync_apply.lock().await;
        let _state = self.state.lock().unwrap();
        let _trust = self.trust.lock().unwrap();
        let _log = self.events_log.lock().unwrap();
        self.stopped.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    async fn send_to(&self, name: &str, body: Body) -> Result<()> {
        let dev = self.device(name)?;
        self.send_device(&dev, body).await
    }

    pub(crate) async fn send_device(&self, dev: &DevicePublic, body: Body) -> Result<()> {
        let kind = body.kind();
        let payload_bytes = bincode::serialized_size(&body)?;
        let inner = Inner {
            from: self.name().to_string(),
            from_key: hex::encode(self.cfg.me.signing_key()?.verifying_key().to_bytes()),
            body,
        };
        let recipient = dev.x25519()?;
        let inner = envelope::authenticate(inner, &self.cfg.me, &recipient)?;
        let sealed = envelope::seal(&inner, &recipient)?;
        let peer: PeerId = dev.peer_id.parse()?;
        let msg_id = self.mesh.send(peer, sealed.clone()).await?;
        self.event("mesh-send", json!({"kind": kind, "to": dev.name, "bytes": sealed.len(), "payload_bytes": payload_bytes, "msg_id": msg_id.to_string(), "envelope_id": inner.envelope_id()}));
        Ok(())
    }

    fn gateways(&self) -> Vec<String> {
        let dev = self.selected_gateway().map(|g| g.announcement.body.device)
            .or_else(|| (!self.cfg.fleet.open).then(|| self.cfg.fleet.gateways().find(|d| d.name != self.name()).cloned()).flatten());
        dev.filter(|d| self.pin_gateway(d).is_ok()).map(|d| vec![self.route_name(&d)]).unwrap_or_default()
    }

    fn require_gateway(&self, from: &str) -> Result<()> {
        if from == self.name() && self.gateway_options().0.is_some() { return Ok(()); }
        let d = self.device(from)?;
        let allowed = if self.cfg.fleet.open {
            self.trust.lock().unwrap().gateway.as_ref().is_some_and(|g| g.ed25519 == d.ed25519)
        } else {
            self.cfg.fleet.gateways().any(|g| g.peer_id == d.peer_id)
                && self.trust.lock().unwrap().gateway.as_ref().is_some_and(|g| g.ed25519 == d.ed25519)
        };
        anyhow::ensure!(allowed, "{from} is not the selected/authorised gateway");
        Ok(())
    }

    pub fn max_payment(&self) -> Result<serde_json::Value> {
        let st = self.state.lock().unwrap();
        let (amount, fee) = builder::max_payment(&st.0, &self.cfg.fleet.policy)?;
        Ok(json!({"amount": zcash_offline_core::fmt_zec(amount), "amount_zat": amount, "fee_zat": fee}))
    }

    // ---------- online ----------

    /// Replaces the wallet with a freshly synced copy, keeping locks taken by
    /// payments built while the sync ran, and persists it.
    fn install_wallet(&self, mut wallet: WalletState) -> Result<(u64, u64, u32)> {
        let mut st = self.state.lock().unwrap();
        for (nf, reservation) in &st.0.reservations {
            if reservation.1 == 0 || wallet.tip_height <= reservation.1 {
                wallet.reservations.entry(nf.clone()).or_insert_with(|| reservation.clone());
            }
        }
        for n in wallet.notes.iter_mut() {
            if let Some(old) = st.0.notes.iter().find(|o| o.nullifier == n.nullifier) {
                if n.spent_in.is_none() && old.locked_by.is_some() {
                    n.locked_by = old.locked_by.clone();
                }
            }
        }
        wallet.release_expired();
        st.0 = wallet;
        self.persist(&st.0, &st.1)?;
        let (spendable, locked) = st.0.balance();
        Ok((spendable, locked, st.0.tip_height))
    }

    /// Online devices sync from Zakura RPC; offline devices ask a gateway over the mesh.
    pub async fn sync(self: &Arc<Self>) -> Result<serde_json::Value> {
        let Some(url) = self.gateway_options().0 else {
            let gw = self.request_mesh_sync(true).await?;
            return Ok(json!({"requested_from": gw, "tip": self.state.lock().unwrap().0.tip_height}));
        };
        let _g = self.sync_apply.lock().await;
        let mut wallet = self.state.lock().unwrap().0.clone();
        let fvk = self.cfg.me.fvk()?;
        if self.cfg.fleet.network == zcash_offline_core::fleet::Network::Testnet {
            let mut source = crate::lightwallet::Lightwallet::connect(&self.testnet_servers()).await?;
            let birthday = self.cfg.fleet.birthday;
            let checkpoints = |w: &WalletState| w.hashes.iter().map(|(h, s)| (*h, s.clone())).collect::<Vec<_>>();
            let mut next = source.batch(wallet.tip_height, &wallet.tip_hash, &checkpoints(&wallet), birthday, SCAN_BLOCKS).await?;
            loop {
                // Fetch the next range while this one applies off the async runtime.
                let (height, hash) = next.blocks.last().map_or((wallet.tip_height, wallet.tip_hash.clone()), |b| (b.height, b.hash.clone()));
                let known = checkpoints(&wallet);
                let (batch, fvk) = (next, fvk.clone());
                let apply = tokio::task::spawn_blocking(move || wallet.apply_batch(&batch, &fvk).map(|_| wallet));
                let fetch = async {
                    if height >= source.tip { return Ok(None); }
                    source.batch(height, &hash, &known, birthday, SCAN_BLOCKS).await.map(Some)
                };
                let (applied, fetched) = tokio::join!(apply, fetch);
                wallet = applied??;
                self.install_wallet(wallet.clone())?;
                match fetched? { Some(batch) => next = batch, None => break }
            }
            *self.sync_sources.lock().unwrap() = source.sources;
            let out = json!({"tip": wallet.tip_height, "spendable": wallet.balance().0, "locked": wallet.balance().1});
            self.event("synced", out.clone());
            return Ok(out);
        }
        let (wallet, report) = tokio::task::spawn_blocking(move || {
            let r = wallet.sync(&Rpc::new(&url), &fvk);
            r.map(|rep| (wallet, rep))
        })
        .await??;
        let (spendable, locked, _) = self.install_wallet(wallet)?;
        let out = json!({"tip": report.tip, "new_notes": report.new_notes, "rescanned": report.rescanned, "spendable": spendable, "locked": locked});
        self.event("synced", out.clone());
        Ok(out)
    }

    /// Sends `syncq` to the first gateway (at most every 5 s unless forced). Returns its name.
    pub async fn request_mesh_sync(self: &Arc<Self>, force: bool) -> Result<String> {
        use std::sync::atomic::Ordering;
        let gw = self.gateways().into_iter().next().ok_or_else(|| anyhow!("no gateway in the fleet"))?;
        let last = self.last_sync_req.load(Ordering::Relaxed);
        if !force && now_ms().saturating_sub(last) < 5_000 {
            return Ok(gw);
        }
        self.last_sync_req.store(now_ms(), Ordering::Relaxed);
        let (have_height, have_hash) = {
            let st = self.state.lock().unwrap();
            (st.0.tip_height, st.0.tip_hash.clone())
        };
        self.send_to(&gw, Body::CompactReq { have_height, have_hash, checkpoints: vec![], birthday: self.cfg.fleet.birthday }).await?;
        Ok(gw)
    }

    pub(crate) async fn fallback_sync(self: &Arc<Self>, seconds: u64) {
        use std::sync::atomic::Ordering::Relaxed;
        let delay = seconds.max(900).saturating_mul(self.sync_backoff.load(Relaxed)).min(3600) * 1000;
        if now_ms().saturating_sub(self.last_sync_req.load(Relaxed)) >= delay {
            let _ = self.request_mesh_sync(false).await;
            self.sync_backoff.store((self.sync_backoff.load(Relaxed) * 2).min(12), Relaxed);
        }
    }

    pub(crate) fn testnet_servers(&self) -> Vec<String> {
        let configured = self.gateway_options().0.unwrap_or_default();
        if configured.is_empty() { self.cfg.fleet.servers.clone() }
        else { configured.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect() }
    }

    async fn on_compact_req(self: &Arc<Self>, from: &str, have: u32, hash: String, checkpoints: Vec<(u32, String)>, birthday: u32) -> Result<()> {
        let url = self.gateway_options().0.ok_or_else(|| anyhow!("not an online gateway"))?;
        anyhow::ensure!(checkpoints.len() <= 200, "too many sync checkpoints");
        let batch = if self.cfg.fleet.network == zcash_offline_core::fleet::Network::Testnet {
            let mut source = crate::lightwallet::Lightwallet::connect(&self.testnet_servers()).await?;
            if have > 0 && checkpoints.is_empty() && (have > source.tip || source.block_hash(have).await? != hash) {
                None
            } else {
                Some(source.batch(have, &hash, &checkpoints, birthday, MESH_SYNC_BLOCKS).await?)
            }
        } else {
            let hash = hash.clone();
            tokio::task::spawn_blocking(move || -> Result<_> {
                let rpc = Rpc::new(&url);
                let tip = rpc.tip()?;
                let mut prev = (have, hash);
                if have > 0 && (have > tip || rpc.block_hash(have)? != prev.1) {
                    if checkpoints.is_empty() { return Ok(None); }
                    prev = checkpoints.iter().rev().find(|(h, s)| *h <= tip && rpc.block_hash(*h).is_ok_and(|v| v == *s))
                        .cloned().ok_or_else(|| anyhow!("reorg exceeds retained checkpoints"))?;
                }
                let mut batch = zcash_offline_core::sync::Batch { prev_height: prev.0, prev_hash: prev.1, seed: None,
                    blocks: vec![], chain_tip: tip, sources: vec!["Regtest JSON-RPC".into()] };
                for h in prev.0 + 1..=tip.min(prev.0 + 200) {
                    let (hash, txs) = rpc.block_txs(h)?;
                    batch.push(zcash_offline_core::sync::CompactBlock::from_raw(h, hash, &txs)?);
                }
                Ok(Some(batch))
            }).await??
        };
        let Some(batch) = batch else {
            return self.send_to(from, Body::CheckpointReq { have_height: have, have_hash: hash }).await;
        };
        *self.sync_sources.lock().unwrap() = batch.sources.clone();
        if batch.seed.is_none() && batch.blocks.is_empty() && batch.prev_height == have && batch.prev_hash == hash {
            return self.send_to(from, Body::Tip { height: have, hash }).await;
        }
        let parts = batch.chunks()?;
        self.event("sync-served", json!({"to": from, "from_height": batch.prev_height + 1, "tip": batch.chain_tip,
            "payload_bytes": bincode::serialized_size(&batch)?, "chunks": parts.len()}));
        for part in parts {
            // A batch can hold more chunks than our store quota (512 packets); wait up to 30 s
            // per chunk for the requester's acks to free room instead of abandoning the batch.
            for wait in 0.. {
                match self.send_to(from, Body::CompactPart(part.clone())).await {
                    Err(e) if e.is::<ozm_mesh::StoreFull>() && wait < 300 => tokio::time::sleep(Duration::from_millis(100)).await,
                    r => { r?; break; }
                }
            }
        }
        Ok(())
    }

    async fn on_compact_part(self: &Arc<Self>, from: &str, part: zcash_offline_core::sync::Part) -> Result<()> {
        self.require_gateway(from)?;
        let _guard = self.sync_apply.lock().await;
        let batch = self.compact_assembly.lock().unwrap().insert(part)?;
        let Some(batch) = batch else { return Ok(()); };
        let mut wallet = self.state.lock().unwrap().0.clone();
        let old_tip = wallet.tip_height;
        let report = wallet.apply_batch(&batch, &self.cfg.me.fvk()?)?;
        let (_, _, tip) = self.install_wallet(wallet)?;
        if tip != old_tip { self.sync_backoff.store(1, std::sync::atomic::Ordering::Relaxed); }
        *self.sync_sources.lock().unwrap() = batch.sources.clone();
        self.event("synced", json!({"via": from, "tip": tip, "chain_tip": batch.chain_tip, "new_notes": report.new_notes,
            "rescanned": report.rescanned, "payload_bytes": bincode::serialized_size(&batch)?, "advanced": tip != old_tip}));
        if tip < batch.chain_tip { self.request_mesh_sync(true).await?; }
        Ok(())
    }

    async fn on_tip(self: &Arc<Self>, from: &str, height: u32, hash: String) -> Result<()> {
        self.require_gateway(from)?;
        let changed = {
            let mut old = self.chain_view.lock().unwrap();
            let changed = *old != (height, hash.clone());
            *old = (height, hash.clone());
            changed
        };
        let needs_sync = {
            let st = self.state.lock().unwrap();
            (height, hash) != (st.0.tip_height, st.0.tip_hash.clone())
        };
        if changed && needs_sync {
            self.sync_backoff.store(1, std::sync::atomic::Ordering::Relaxed);
            self.request_mesh_sync(false).await?;
        }
        Ok(())
    }

    async fn on_checkpoint_req(&self, from: &str, have_height: u32, have_hash: String) -> Result<()> {
        self.require_gateway(from)?;
        let body = {
            let st = self.state.lock().unwrap();
            // A delayed challenge must not rewind a newer successful sync.
            if st.0.tip_height != have_height || st.0.tip_hash != have_hash { return Ok(()); }
            Body::CompactReq { have_height, have_hash, birthday: self.cfg.fleet.birthday,
                checkpoints: st.0.hashes.iter().map(|(h, s)| (*h, s.clone())).collect() }
        };
        self.send_to(from, body).await
    }

    // ---------- gateway: chain data and test funds over the mesh ----------

    async fn on_sync_req(self: &Arc<Self>, from: &str, have_height: u32, have_hash: String) -> Result<()> {
        let url = self.gateway_options().0.ok_or_else(|| anyhow!("syncq received but this device is not online"))?;
        let cache = self.block_cache.lock().unwrap().clone();
        let (reset, prev_hash, blocks, tip, cache) = tokio::task::spawn_blocking(move || -> Result<_> {
            let rpc = Rpc::new(&url);
            let tip = rpc.tip()?;
            let reset = have_height > 0 && (have_height > tip || rpc.block_hash(have_height)? != have_hash);
            let (start, prev_hash) = if reset || have_height == 0 { (1, String::new()) } else { (have_height + 1, have_hash) };
            let mut cache = cache;
            let mut blocks = vec![];
            let mut bytes = 0usize;
            // About 192 KiB of transactions per answer; the requester asks again for the rest.
            for h in start..=tip {
                let hash = rpc.block_hash(h)?;
                let b = match cache.get(&h).filter(|b| b.hash == hash) {
                    Some(b) => b.clone(),
                    None => {
                        let (hash, txs) = rpc.block_txs(h)?;
                        let txs: Vec<Vec<u8>> = txs.into_iter().filter(|t| zcash_offline_core::wallet::has_ironwood(t)).collect();
                        let b = BlockData { height: h, hash, txs };
                        cache.insert(h, b.clone());
                        b
                    }
                };
                bytes += b.txs.iter().map(Vec::len).sum::<usize>() + 80;
                blocks.push(b);
                if bytes > 192 * 1024 {
                    break;
                }
            }
            Ok((reset, prev_hash, blocks, tip, cache))
        })
        .await??;
        *self.block_cache.lock().unwrap() = cache;
        self.event("sync-served", json!({"to": from, "from_height": blocks.first().map(|b| b.height), "blocks": blocks.len(), "tip": tip, "reset": reset}));
        self.send_to(from, Body::SyncRes { reset, prev_hash, blocks, chain_tip: tip }).await
    }

    async fn on_sync_res(self: &Arc<Self>, from: &str, reset: bool, prev_hash: String, blocks: Vec<BlockData>, chain_tip: u32) -> Result<()> {
        self.require_gateway(from)?;
        let _g = self.sync_apply.lock().await;
        let mut wallet = self.state.lock().unwrap().0.clone();
        let fvk = self.cfg.me.fvk()?;
        let last = blocks.last().map(|b| b.height);
        let (wallet, report) = tokio::task::spawn_blocking(move || -> Result<_> {
            if reset {
                wallet.reset_keep_locks();
            }
            let first = blocks.first().map_or(wallet.tip_height + 1, |b| b.height);
            if first != wallet.tip_height + 1 {
                bail!("chunk starts at {first}, wallet is at {}", wallet.tip_height);
            }
            let src = ChunkSource { prev: (wallet.tip_height, if wallet.tip_height == 0 { String::new() } else { prev_hash }), blocks };
            let rep = wallet.sync_from(&src, &fvk)?;
            Ok((wallet, rep))
        })
        .await??;
        let (spendable, locked, tip) = self.install_wallet(wallet)?;
        self.event("synced", json!({"via": from, "tip": tip, "chain_tip": chain_tip, "new_notes": report.new_notes, "rescanned": reset, "spendable": spendable, "locked": locked}));
        if last.is_some_and(|l| l < chain_tip) {
            self.request_mesh_sync(true).await?;
        }
        Ok(())
    }

    /// Device: ask a gateway for test funds (Regtest/testnet sessions).
    pub async fn request_funds(self: &Arc<Self>, notes: u32, amount_each: u64) -> Result<String> {
        anyhow::ensure!(self.cfg.fleet.network == zcash_offline_core::fleet::Network::Regtest, "no gateway faucet on testnet; use faucet.testnet.valargroup.dev, then Prepare for offline");
        let gw = self.gateways().into_iter().next().ok_or_else(|| anyhow!("no gateway in the fleet"))?;
        self.send_to(&gw, Body::FundReq { notes, amount_each }).await?;
        Ok(gw)
    }

    /// Gateway: pays test notes from its own online wallet, one transaction per note
    /// (each mined before the next, so change is spendable), then reports back.
    async fn on_fund_req(self: &Arc<Self>, from: &str, notes: u32, amount_each: u64) -> Result<()> {
        let (ok, detail) = match self.fund(from, notes, amount_each).await {
            Ok(d) => (true, d),
            Err(e) => (false, format!("{e:#}")),
        };
        self.event("funded", json!({"to": from, "ok": ok, "detail": detail}));
        self.send_to(from, Body::FundRes { ok, detail }).await
    }

    async fn fund(self: &Arc<Self>, to: &str, notes: u32, amount_each: u64) -> Result<String> {
        anyhow::ensure!(self.cfg.fleet.network == zcash_offline_core::fleet::Network::Regtest, "gateway funding is Regtest-only");
        let (url, mine) = self.gateway_options();
        let url = url.ok_or_else(|| anyhow!("this gateway is not online"))?;
        anyhow::ensure!(mine, "this gateway does not mine (faucet only runs on Regtest gateways)");
        anyhow::ensure!(notes >= 1 && amount_each > 0, "nothing requested");
        let total = (notes as u64).checked_mul(amount_each).ok_or_else(|| anyhow!("faucet amount overflow"))?;
        anyhow::ensure!(total <= self.cfg.faucet_max_zat, "request of {total} zat is above this gateway's faucet limit {}", self.cfg.faucet_max_zat);
        anyhow::ensure!(notes <= 10, "at most 10 test notes per request");
        let dev = self.device(to)?;
        let to_addr = dev.orchard_address()?;
        {
            let mut st = self.state.lock().unwrap();
            let now = now_ms();
            anyhow::ensure!(st.1.faucet_last.get(&dev.peer_id).is_none_or(|last| now.saturating_sub(*last) >= 60_000), "faucet rate limit: one request per device per 60 seconds");
            st.1.faucet_last.insert(dev.peer_id, now);
            self.persist(&st.0, &st.1)?;
        }
        let _guard = self.pay_lock.lock().await;
        let mut txids = vec![];
        for _ in 0..notes {
            self.sync().await?;
            let policy = self.cfg.fleet.policy.clone();
            let me = self.cfg.me.clone();
            let mut wallet = self.state.lock().unwrap().0.clone();
            let (wallet, built) = tokio::task::spawn_blocking(move || {
                builder::build_payment(&mut wallet, &me, to_addr, amount_each, &policy).map(|b| (wallet, b))
            })
            .await??;
            self.install_wallet(wallet)?;
            let (raw, txid) = (built.raw.clone(), built.txid.clone());
            let url2 = url.clone();
            let status = tokio::task::spawn_blocking(move || submit_and_mine(&Rpc::new(&url2), &raw, &txid, mine)).await??;
            anyhow::ensure!(matches!(status.0, TxStatus::Mined { .. }), "faucet payment not mined: {:?}", status.0);
            txids.push(built.txid);
        }
        self.sync().await?;
        Ok(format!("{notes} × {} ZEC sent and mined: {}", zcash_offline_core::fmt_zec(amount_each), txids.join(", ")))
    }

        // ---------- payer ----------

    pub async fn pay(self: &Arc<Self>, to: &str, amount: u64) -> Result<Payment> {
        self.pay_inner(to, amount, None).await
    }

    /// Persist a split and its reservation before broadcasting. Its outputs remain
    /// unavailable until compact sync discovers them in a canonical block.
    pub async fn prepare_offline(self: &Arc<Self>, count: u32, value: u64) -> Result<Payment> {
        let _guard = self.pay_lock.lock().await;
        let _sync = self.sync_apply.lock().await;
        let me = self.cfg.me.clone();
        let policy = self.cfg.fleet.policy.clone();
        let mut wallet = self.state.lock().unwrap().0.clone();
        let (wallet, built) = tokio::task::spawn_blocking(move || {
            builder::prepare_offline(&mut wallet, &me, count, value, &policy).map(|b| (wallet, b))
        }).await??;
        let (_, checked) = verify::check_tx(&built.raw, &self.cfg.fleet.policy)?;
        let mut payment = Payment {
            txid: built.txid.clone(), auth_digest: built.auth_digest, direction: "prepare".into(),
            counterparty: self.cfg.me.public_on(self.cfg.fleet.network)?.address, amount: built.amount,
            fee: built.fee, expiry_height: built.expiry_height, state: "built".into(), tx_hex: hex::encode(&built.raw),
            nullifiers: checked.nullifiers, statement: None, receipt: None, result: None, history: vec![],
            created_ms: now_ms(), last_send_ms: now_ms(), attempts: 1, test_tamper: None, receipt_peer: None,
        };
        payment.note(format!("Prepare for offline: {count} × {value} zat; fee {} zat; wait for mined before going offline", built.fee));
        {
            let mut st = self.state.lock().unwrap();
            st.0 = wallet;
            st.1.payments.insert(payment.txid.clone(), payment.clone());
            self.persist(&st.0, &st.1)?;
        }
        if self.gateway_options().0.is_some() {
            self.on_sub(self.name(), built.raw, built.txid, vec![]).await?;
        } else {
            for gateway in self.gateways() {
                self.send_to(&gateway, self.submission(&built.txid, built.raw.clone())?).await?;
            }
        }
        Ok(self.payment(&payment.txid).unwrap())
    }

    /// Debug only: builds a payment and breaks one thing, so the payee's refusal
    /// can be tested end to end. `tamper`: amount, payee, sig, branch, expired.
    pub async fn pay_tampered(self: &Arc<Self>, to: &str, amount: u64, tamper: &str) -> Result<Payment> {
        anyhow::ensure!(["amount", "payee", "sig", "branch", "expired"].contains(&tamper), "unknown tamper {tamper}");
        self.pay_inner(to, amount, Some(tamper.to_string())).await
    }

    async fn pay_inner(self: &Arc<Self>, to: &str, amount: u64, tamper: Option<String>) -> Result<Payment> {
        let _guard = self.pay_lock.lock().await;
        let _sync = self.sync_apply.lock().await;
        // Names remain accepted for existing fleet scripts; all UIs use the address contract.
        let address = self.cfg.fleet.by_name(to).map(|d| d.address.clone()).unwrap_or_else(|_| to.trim().to_string());
        let payee = self.receipt_device(&address);
        anyhow::ensure!(address != self.cfg.me.public_on(self.cfg.fleet.network)?.address, "cannot pay yourself");
        let mut policy = self.cfg.fleet.policy.clone();
        let t = tamper.as_deref();
        if t == Some("expired") {
            policy.expiry_delta = 5;
        }
        let me = self.cfg.me.clone();
        let mut wallet = self.state.lock().unwrap().0.clone();
        let to_addr = if t == Some("payee") { DeviceSecret::generate("X", "payee").address()? } else { self.cfg.fleet.network.decode_address(&address)? };
        let (wallet, mut built) = tokio::task::spawn_blocking(move || {
            builder::build_payment(&mut wallet, &me, to_addr, amount, &policy).map(|b| (wallet, b))
        })
        .await??;
        let (tx, checked) = verify::check_tx(&built.raw, &self.cfg.fleet.policy).map_err(|e| anyhow!("self-check failed: {e}"))?;
        drop(tx);
        let n = built.raw.len();
        match t {
            Some("sig") => built.raw[n - 10] ^= 1,
            Some("branch") => built.raw[8..12].copy_from_slice(&0x4dec_4df0u32.to_le_bytes()),
            _ => {}
        }
        let payment = {
            let mut st = self.state.lock().unwrap();
            st.0 = wallet;
            // Time-based so it keeps rising when the same device is reinstalled with an empty store.
            st.1.seq_out = (st.1.seq_out + 1).max(now_ms());
            let statement = Signed::sign(
                Statement {
                    txid: built.txid.clone(),
                    auth_digest: built.auth_digest.clone(),
                    amount: if t == Some("amount") { amount + 1 } else { amount },
                    fee: built.fee,
                    expiry_height: built.expiry_height,
                    payer: self.name().to_string(),
                    payee: payee.as_ref().map(|p| self.route_name(p)).unwrap_or_else(|| address.clone()),
                    seq: st.1.seq_out,
                    created_ms: now_ms(),
                },
                &self.cfg.me,
            )?;
            let mut p = Payment {
                txid: built.txid.clone(),
                auth_digest: built.auth_digest.clone(),
                direction: "sent".into(),
                counterparty: address.clone(),
                amount,
                fee: built.fee,
                expiry_height: built.expiry_height,
                state: "built".into(),
                tx_hex: hex::encode(&built.raw),
                nullifiers: checked.nullifiers,
                statement: Some(statement),
                receipt: None,
                result: None,
                history: vec![],
                created_ms: now_ms(),
                // The first send follows at once; the retry tick must not race it with a second copy.
                last_send_ms: now_ms(),
                attempts: 0,
                test_tamper: tamper.clone(),
                receipt_peer: payee.as_ref().map(|p| self.route_name(p)),
            };
            p.note(format!("built offline: {} bytes, proved in {} ms, expiry {}", built.raw.len(), built.prove_ms, built.expiry_height));
            st.1.payments.insert(p.txid.clone(), p.clone());
            // Persist the lock and the payment before anything leaves the device.
            self.persist(&st.0, &st.1)?;
            p
        };
        self.event("built", json!({"txid": payment.txid, "bytes": built.raw.len(), "prove_ms": built.prove_ms, "amount": amount, "to": address}));
        self.send_payment(&payment.txid).await?;
        Ok(self.payment(&payment.txid).unwrap())
    }

    /// Debug only: re-sends a payment as new envelopes (duplicate and idempotence tests).
    pub async fn resend(self: &Arc<Self>, txid: &str) -> Result<()> {
        let p = self.payment(txid).ok_or_else(|| anyhow!("unknown payment"))?;
        let tx = hex::decode(&p.tx_hex)?;
        if let Some(st) = p.statement.clone().filter(|_| p.direction == "sent") {
            if let Some(to) = &p.receipt_peer {
                self.send_to(to, Body::Pay { tx: tx.clone(), statement: st }).await?;
            }
        }
        for g in self.gateways() {
            self.send_to(&g, self.submission(&p.txid, tx.clone())?).await?;
        }
        Ok(())
    }

    /// Sends (or re-sends) `pay` to the payee and `sub` to the gateways. Both A and B submit.
    async fn send_payment(self: &Arc<Self>, txid: &str) -> Result<()> {
        let p = self.payment(txid).ok_or_else(|| anyhow!("unknown payment"))?;
        let tx = hex::decode(&p.tx_hex)?;
        let statement = p.statement.clone().ok_or_else(|| anyhow!("no statement"))?;
        if p.receipt.is_none() {
            if let Some(to) = &p.receipt_peer {
                // A payee disappearing must not prevent submitting the already-built transaction.
                if let Err(e) = self.send_to(to, Body::Pay { tx: tx.clone(), statement }).await {
                    self.event("payee-unreachable", json!({"txid": txid, "error": e.to_string()}));
                }
            }
        }
        for g in self.gateways().into_iter().filter(|_| p.test_tamper.is_none()) {
            self.send_to(&g, self.submission(&p.txid, tx.clone())?).await?;
        }
        self.update(txid, |p| {
            if p.state == "built" {
                p.state = if p.receipt_peer.is_some() { "sent" } else { "submitted, no payee receipt" }.into();
            }
            p.attempts += 1;
            p.last_send_ms = now_ms();
            p.note(format!("sent over mesh (attempt {})", p.attempts));
        })
    }

    // ---------- shared ----------

    fn submission(&self, txid: &str, tx: Vec<u8>) -> Result<Body> {
        let mut st = self.state.lock().unwrap();
        if !st.1.request_nonces.contains_key(txid) {
            st.1.request_nonces.insert(txid.into(), ozm_mesh::MsgId::random().to_string());
            self.persist(&st.0, &st.1)?;
        }
        Ok(Body::Sub { tx, txid: txid.into(), notify: vec![format!("nonce:{}", st.1.request_nonces[txid])] })
    }

    pub fn payment(&self, txid: &str) -> Option<Payment> {
        self.state.lock().unwrap().1.payments.get(txid).cloned()
    }

    fn update(&self, txid: &str, f: impl FnOnce(&mut Payment)) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        if let Some(p) = st.1.payments.get_mut(txid) {
            f(p);
        }
        self.persist(&st.0, &st.1)
    }

    pub async fn handle_mesh_event(self: &Arc<Self>, ev: MeshEvent) {
        match ev {
            MeshEvent::Message { from, payload, hops, msg_id, broadcast } => {
                if broadcast {
                    if let Some(bytes) = payload.strip_prefix(RESULT_MAGIC) {
                        let result = async {
                            let notice: Signed<GatewayNotice> = serde_json::from_slice(bytes)?;
                            let dev = self.device(&from.to_string())?;
                            notice.verify_key(&dev)?;
                            let txid = &notice.body.result.body.txid;
                            anyhow::ensure!(self.state.lock().unwrap().1.request_nonces.get(txid) == Some(&notice.body.request_nonce), "unknown result nonce");
                            self.handle_result(&self.route_name(&dev), notice.body.result).await
                        }.await;
                        if let Err(e) = result { warn!("dropping gateway notice: {e}"); }
                    } else {
                        let _ = self.on_key(from, &payload).await;
                    }
                    return;
                }
                let mut inner = match envelope::open(&payload, &self.x25519) {
                    Ok(i) => i,
                    Err(e) => {
                        warn!("dropping message {msg_id} from {from}: {e}");
                        return;
                    }
                };
                let sender = DevicePublic { name: inner.from.clone(), ed25519: inner.from_key.clone(),
                    peer_id: from.to_string(), role: String::new(), x25519: String::new(), address: String::new() };
                if matches!(inner.body, Body::Auth { .. }) {
                    let result = sender.verifying_key().and_then(|vk| {
                        anyhow::ensure!(zcash_offline_core::fleet::peer_id_for(&vk) == from.to_string(), "originating mesh ID does not match signing key");
                        envelope::authenticated_body(&inner, &sender, x25519_dalek::PublicKey::from(&self.x25519).as_bytes())
                    });
                    match result {
                        Ok(Body::Presence(p)) => {
                            if p.body.device.name == inner.from {
                                let gateway = p.body.gateway;
                                let new = !self.peers.lock().unwrap().contains_key(&from.to_string());
                                if self.on_presence(from, p, if self.mesh.neighbors().contains(&from) { 1 } else { hops }).is_ok()
                                    && gateway && new && self.gateway_options().0.is_none() {
                                    let _ = self.request_mesh_sync(false).await;
                                }
                            }
                            return;
                        }
                        Ok(body) => inner.body = body,
                        Err(e) => { warn!("unauthenticated message from {from}: {e}"); return; }
                    }
                } else {
                    return; // Invitations do not make unsigned requests authentic.
                }
                let dev = self.device(&from.to_string()).ok();
                if dev.as_ref().map(|d| (d.name.as_str(), d.ed25519.as_str())) != Some((inner.from.as_str(), inner.from_key.as_str())) {
                    warn!("dropping message {msg_id}: sender {} does not match mesh peer {from}", inner.from);
                    return;
                }
                self.event("mesh-recv", json!({"kind": inner.body.kind(), "from": inner.from, "hops": hops, "bytes": payload.len(), "msg_id": msg_id.to_string(), "envelope_id": inner.envelope_id()}));
                let from_name = self.route_name(&dev.unwrap());
                let r = match inner.body {
                    Body::Pay { tx, statement } => self.on_pay(&from_name, tx, statement).await,
                    Body::Rcpt(r) => self.on_receipt(&from_name, r),
                    Body::Sub { tx, txid, notify } => self.on_sub(&from_name, tx, txid, notify).await,
                    Body::Res(r) => self.handle_result(&from_name, r).await,
                    Body::SyncReq { have_height, have_hash } => self.on_sync_req(&from_name, have_height, have_hash).await,
                    Body::SyncRes { reset, prev_hash, blocks, chain_tip } => self.on_sync_res(&from_name, reset, prev_hash, blocks, chain_tip).await,
                    Body::CompactReq { have_height, have_hash, checkpoints, birthday } => self.on_compact_req(&from_name, have_height, have_hash, checkpoints, birthday).await,
                    Body::CompactPart(part) => self.on_compact_part(&from_name, part).await,
                    Body::Tip { height, hash } => self.on_tip(&from_name, height, hash).await,
                    Body::CheckpointReq { have_height, have_hash } => self.on_checkpoint_req(&from_name, have_height, have_hash).await,
                    Body::FundReq { notes, amount_each } => self.on_fund_req(&from_name, notes, amount_each).await,
                    Body::FundRes { ok, detail } => {
                        if self.require_gateway(&from_name).is_err() { return; }
                        self.event("funds", json!({"from": from_name, "ok": ok, "detail": detail}));
                        if ok {
                            let _ = self.request_mesh_sync(true).await;
                        }
                        Ok(())
                    }
                    Body::Presence(_) | Body::Auth { .. } => Ok(()),
                };
                if let Err(e) = r {
                    warn!("handling message from {from_name}: {e:#}");
                    self.event("error", json!({"from": from_name, "error": format!("{e:#}")}));
                }
            }
            MeshEvent::Delivered { msg_id, by } => {
                let by_name = self.device(&by.to_string()).map(|d| self.route_name(&d)).unwrap_or(by.to_string());
                self.event("mesh-delivered", json!({"msg_id": msg_id.to_string(), "by": by_name}));
                // Mark sent payments to that payee as delivered (the mesh-level ack).
                let txids: Vec<String> = {
                    let st = self.state.lock().unwrap();
                    st.1.payments.values().filter(|p| p.direction == "sent" && p.receipt_peer.as_deref() == Some(&by_name) && p.state == "sent").map(|p| p.txid.clone()).collect()
                };
                for t in txids {
                    let _ = self.update(&t, |p| {
                        p.state = "delivered".into();
                        p.note(format!("mesh ack from {by_name}"));
                    });
                }
            }
            MeshEvent::NeighborUp(p) => {
                self.event("neighbor-up", json!({"peer": self.peer_name(p)}));
                let _ = self.announce().await;
            }
            MeshEvent::NeighborDown(p) => self.event("neighbor-down", json!({"peer": self.peer_name(p)})),
        }
    }

    pub fn peer_name(&self, p: PeerId) -> String {
        self.device(&p.to_string()).map(|d| d.name).unwrap_or(p.to_string())
    }

    // ---------- payee ----------

    async fn verify_offline(&self, tx: &[u8], pending: bool) -> Result<Result<verify::Verified, Reject>> {
        let mut policy = self.cfg.fleet.policy.clone();
        if !pending { policy.floor_zat = u64::MAX; }
        let me = self.cfg.me.clone();
        let wallet = self.state.lock().unwrap().0.clone();
        let raw = tx.to_vec();
        Ok(tokio::task::spawn_blocking(move || verify::verify_payment(&raw, &me, &wallet, &policy)).await?)
    }

    async fn on_pay(self: &Arc<Self>, from: &str, tx: Vec<u8>, statement: Signed<Statement>) -> Result<()> {
        let _guard = self.pay_lock.lock().await;
        let s = &statement.body;
        // Duplicate (same txid): one payment; answer again with the stored receipt.
        if let Some(p) = self.payment(&s.txid) {
            self.event("duplicate-pay", json!({"txid": s.txid, "from": from}));
            if let Some(r) = p.receipt.clone() {
                self.send_to(from, Body::Rcpt(r)).await?;
            }
            return Ok(());
        }
        let t = std::time::Instant::now();
        let payer = self.device(from)?;
        let pending = !self.cfg.fleet.open && self.cfg.fleet.by_peer(&payer.peer_id).is_some();
        let exposure = if pending {
            let st = self.state.lock().unwrap();
            let outstanding: Vec<_> = st.1.payments.values().filter(|p| p.direction == "received" && p.state != "mined" && p.receipt.is_some()).collect();
            let same: Vec<_> = outstanding.iter().filter(|p| p.statement.as_ref().is_some_and(|s| s.signer == payer.ed25519)).collect();
            // Provisional test-coin limits from pending-tier.md. Expiry does not erase debt.
            (same.len() >= 3 || same.iter().map(|p| p.amount as u128).sum::<u128>() + s.amount as u128 > 100_000_000
                || outstanding.iter().map(|p| p.amount as u128).sum::<u128>() + s.amount as u128 > 300_000_000)
                .then(|| Reject { code: code::EXPOSURE, reason: "pending exposure cap (3 payments / 1 ZEC per payer; 3 ZEC total); wait for mined".into() })
        } else { None };
        let mut checked = match exposure { Some(r) => Err(r), None => self.verify_offline(&tx, pending).await? };
        // The payer may have synced past this payee's view. An offline payee refreshes its
        // chain view from a gateway over the mesh and checks once more before refusing.
        if self.gateway_options().0.is_none() && matches!(&checked, Err(r) if r.code == code::ANCHOR) {
            let before = self.state.lock().unwrap().0.tip_height;
            self.event("anchor-unknown-syncing", json!({"txid": s.txid, "view_height": before}));
            if self.request_mesh_sync(true).await.is_ok() {
                for _ in 0..60 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if self.state.lock().unwrap().0.tip_height > before {
                        break;
                    }
                }
                checked = self.verify_offline(&tx, pending).await?;
            }
        }
        let verify_ms = t.elapsed().as_millis();
        let result = checked.and_then(|v| {
            let st_err = |r: &str| Reject { code: code::STATEMENT, reason: r.to_string() };
            self.device(from).and_then(|d| statement.verify_key(&d)).map_err(|e| st_err(&format!("statement: {e}")))?;
            if s.payee != self.name() && s.payee != self.mesh.peer_id().to_string() {
                return Err(st_err("statement names another payee"));
            }
            if s.txid != v.tx.txid || s.auth_digest != v.tx.auth_digest {
                return Err(st_err("statement txid or auth_digest does not match the transaction"));
            }
            if s.amount != v.amount {
                return Err(st_err(&format!("statement amount {} but transaction pays {}", s.amount, v.amount)));
            }
            if s.expiry_height != v.tx.expiry_height {
                return Err(st_err("statement expiry does not match the transaction"));
            }
            let st = self.state.lock().unwrap();
            if let Some(last) = st.1.last_seq_in.get(from) {
                if s.seq <= *last {
                    return Err(st_err(&format!("replayed or old seq {} (last {last})", s.seq)));
                }
            }
            for p in st.1.payments.values().filter(|p| p.direction == "received" && p.state != "mined") {
                if let Some(nf) = v.tx.nullifiers.iter().find(|n| p.nullifiers.contains(n)) {
                    return Err(Reject { code: code::CONFLICT, reason: format!("note {nf} already pays pending payment {}", p.txid) });
                }
            }
            Ok(v)
        });
        match result {
            Ok(v) => {
                let receipt = pending.then(|| Signed::sign(
                    Receipt {
                        txid: v.tx.txid.clone(),
                        auth_digest: v.tx.auth_digest.clone(),
                        payee: self.name().into(),
                        amount: v.amount,
                        state: ReceiptState::Pending,
                        view_height: v.view_height,
                        created_ms: now_ms(),
                    },
                    &self.cfg.me,
                )).transpose()?;
                {
                    let mut st = self.state.lock().unwrap();
                    st.1.last_seq_in.insert(from.to_string(), s.seq);
                    let mut p = Payment {
                        txid: v.tx.txid.clone(),
                        auth_digest: v.tx.auth_digest.clone(),
                        direction: "received".into(),
                        counterparty: payer.address,
                        amount: v.amount,
                        fee: v.tx.fee,
                        expiry_height: v.tx.expiry_height,
                        state: if pending { "accepted-pending" } else { "awaiting-inclusion" }.into(),
                        tx_hex: hex::encode(&tx),
                        nullifiers: v.tx.nullifiers.clone(),
                        statement: Some(statement.clone()),
                        receipt: receipt.clone(),
                        result: None,
                        history: vec![],
                        created_ms: now_ms(),
                        last_send_ms: now_ms(),
                        attempts: 1,
                        test_tamper: None,
                        receipt_peer: Some(from.to_string()),
                    };
                    p.note(format!("checked in {verify_ms} ms at view height {}; anchor height {}; {}", v.view_height, v.anchor_height,
                        if pending { "fleet pending acceptance" } else { "not accepted: awaiting gateway-reported inclusion" }));
                    st.1.payments.insert(p.txid.clone(), p);
                    // fsync before the receipt leaves this device.
                    self.persist(&st.0, &st.1)?;
                }
                self.event(if pending { "accepted" } else { "awaiting-inclusion" }, json!({"txid": v.tx.txid, "amount": v.amount, "from": from, "verify_ms": verify_ms}));
                if let Some(receipt) = receipt { self.send_to(from, Body::Rcpt(receipt)).await?; }
                for g in self.gateways() {
                    self.send_to(&g, self.submission(&v.tx.txid, tx.clone())?).await?;
                }
                self.update(&v.tx.txid, |p| {
                    if pending && !p.is_final() { p.state = "submitted".into(); }
                    p.note("forwarded to gateway");
                })?;
            }
            Err(reject) => {
                {
                    let mut st = self.state.lock().unwrap();
                    st.1.refusals.push(Refusal { ms: now_ms(), from: from.into(), txid: s.txid.clone(), reject: reject.clone() });
                    self.persist(&st.0, &st.1)?;
                }
                self.event("refused", json!({"txid": s.txid, "from": from, "code": reject.code, "reason": reject.reason}));
                let receipt = Signed::sign(
                    Receipt {
                        txid: s.txid.clone(),
                        auth_digest: s.auth_digest.clone(),
                        payee: self.name().into(),
                        amount: 0,
                        state: ReceiptState::Refused(reject),
                        view_height: self.state.lock().unwrap().0.tip_height,
                        created_ms: now_ms(),
                    },
                    &self.cfg.me,
                )?;
                self.send_to(from, Body::Rcpt(receipt)).await?;
            }
        }
        Ok(())
    }

    // ---------- payer: receipts ----------

    fn on_receipt(&self, from: &str, r: Signed<Receipt>) -> Result<()> {
        r.verify_key(&self.device(from)?)?;
        let txid = r.body.txid.clone();
        let Some(p) = self.payment(&txid) else { bail!("receipt for unknown payment {txid}") };
        if p.receipt_peer.as_deref() != Some(from) {
            bail!("receipt from {from}, but {txid} was paid to {}", p.counterparty);
        }
        let state = match &r.body.state {
            ReceiptState::Pending => {
                anyhow::ensure!(!self.cfg.fleet.open && self.cfg.fleet.by_peer(&self.device(from)?.peer_id).is_some(), "no pending acceptance outside the invited fleet");
                "accepted-pending".to_string()
            }
            ReceiptState::Included => "included (gateway-reported)".into(),
            ReceiptState::Refused(rej) => format!("refused, may still mine until expiry: {rej}"),
        };
        self.event("receipt", json!({"txid": txid, "from": from, "state": state}));
        self.update(&txid, |p| {
            if !p.is_final() {
                p.state = match r.body.state {
                    ReceiptState::Pending => "accepted-pending".into(),
                    ReceiptState::Included => "included".into(),
                    ReceiptState::Refused(_) => "refused, may still mine until expiry".into(),
                };
            }
            p.note(format!("receipt from {from}: {state}"));
            p.receipt = Some(r);
        })
    }

    // ---------- gateway ----------

    async fn publish_gateway(&self, mut entry: GatewayEntry, status: TxStatus, tip: u32, submitted: bool) -> Result<()> {
        let txid = entry.txid.clone();
        let (result, notify) = {
            let mut st = self.state.lock().unwrap();
            st.1.gw_seq += 1;
            let result = Signed::sign(GatewayResult { txid: txid.clone(), status: status.clone(), tip_height: tip,
                gateway: self.name().into(), seq: st.1.gw_seq, created_ms: now_ms() }, &self.cfg.me)?;
            if submitted { entry.submissions += 1; entry.last_submit_height = tip; }
            entry.result = result.clone();
            let notify = entry.notify.clone();
            st.1.gateway.insert(txid.clone(), entry);
            self.persist(&st.0, &st.1)?;
            (result, notify)
        };
        self.event("gateway-result", json!({"txid": txid, "status": status, "tip": tip, "gateway_reported": true}));
        if self.payment(&txid).is_some() { self.on_result(self.name(), result.clone())?; }
        // Legacy clients keep private results; nonce requests disclose no counterpart name.
        for to in notify {
            if let Err(e) = self.send_result(&to, result.clone()).await {
                self.event("result-undelivered", json!({"txid": txid, "to": to, "error": e.to_string()}));
            }
        }
        Ok(())
    }

    async fn on_testnet_sub(self: &Arc<Self>, from: &str, raw: Vec<u8>, txid: String, notify: Vec<String>) -> Result<()> {
        anyhow::ensure!(self.gateway_options().0.is_some(), "not an online gateway");
        let old = self.state.lock().unwrap().1.gateway.get(&txid).cloned();
        anyhow::ensure!(old.as_ref().is_none_or(|e| e.tx_hex == hex::encode(&raw)), "another variant of this txid is already held");
        let expiry_height = match &old {
            Some(entry) => entry.expiry_height,
            None => {
                let (_, checked) = verify::check_tx(&raw, &self.cfg.fleet.policy)?;
                anyhow::ensure!(checked.txid == txid, "submission txid does not match transaction");
                checked.expiry_height
            }
        };
        let mut source = crate::lightwallet::Lightwallet::connect(&self.testnet_servers()).await?;
        anyhow::ensure!(expiry_height > 0 && expiry_height as u64 <= source.tip as u64 + 1 + self.cfg.fleet.policy.expiry_delta as u64, "expiry outside fleet policy");
        let mut submitted = false;
        let known = source.status(&raw, &txid).await?;
        let status = match known {
            Some(status @ (TxStatus::Included { .. } | TxStatus::Mined { .. })) => status,
            _ if source.tip > expiry_height => TxStatus::Expired,
            _ if old.as_ref().is_none_or(|e| source.tip.saturating_sub(e.last_submit_height) >= 3) => {
                submitted = true;
                source.submit(&raw).await?
            }
            Some(status) => status,
            None => TxStatus::Submitted,
        };
        *self.sync_sources.lock().unwrap() = source.sources;
        let mut entry = match old {
            Some(entry) => entry,
            None => GatewayEntry { txid: txid.clone(), tx_hex: hex::encode(&raw), expiry_height,
                submitter: from.into(), enrolled: self.enrolled_sender(from),
                submissions: 0, last_submit_height: 0, notify: vec![],
                result: Signed::sign(GatewayResult { txid, status: status.clone(), tip_height: source.tip,
                    gateway: self.name().into(), seq: 0, created_ms: now_ms() }, &self.cfg.me)? },
        };
        entry.notify.extend(notify_targets(from, notify).into_iter().chain(self.joined_submitters()).filter(|n| n != self.name()));
        entry.notify.sort();
        entry.notify.dedup();
        self.publish_gateway(entry, status, source.tip, submitted).await
    }

    /// Internet-only polling; mesh wallets sync on the pushed tip, not a 30 s timer.
    pub(crate) async fn gateway_tick(self: &Arc<Self>) -> Result<()> {
        if self.cfg.fleet.network != zcash_offline_core::fleet::Network::Testnet || self.gateway_options().0.is_none() { return Ok(()); }
        let Ok(_guard) = self.gateway_apply.try_lock() else { return Ok(()); };
        let mut source = crate::lightwallet::Lightwallet::connect(&self.testnet_servers()).await?;
        let old = self.chain_view.lock().unwrap().clone();
        let changed = old != (source.tip, source.hash.clone());
        self.reported_tip.store(source.tip, std::sync::atomic::Ordering::Relaxed);
        *self.sync_sources.lock().unwrap() = source.sources.clone();
        let entries: Vec<_> = self.state.lock().unwrap().1.gateway.values().filter(|e| !e.tx_hex.is_empty()).cloned().collect();
        for entry in entries {
            let raw = hex::decode(&entry.tx_hex)?;
            let known = source.status(&raw, &entry.txid).await?;
            let mut submitted = false;
            let status = match known {
                Some(status @ (TxStatus::Included { .. } | TxStatus::Mined { .. })) => status,
                _ if source.tip > entry.expiry_height => TxStatus::Expired,
                _ if changed && source.tip.saturating_sub(entry.last_submit_height) >= 3 => {
                    submitted = true;
                    source.submit(&raw).await?
                }
                Some(status) => status,
                None => TxStatus::Submitted,
            };
            if status != entry.result.body.status || submitted {
                self.publish_gateway(entry, status, source.tip, submitted).await?;
            }
        }
        *self.chain_view.lock().unwrap() = (source.tip, source.hash.clone());
        {
            let mut st = self.state.lock().unwrap();
            let before = st.1.gateway.len();
            st.1.gateway.retain(|_, e| e.expiry_height == 0 || source.tip as u64 <= e.expiry_height as u64 + self.cfg.fleet.policy.expiry_margin as u64);
            if before != st.1.gateway.len() { self.persist(&st.0, &st.1)?; }
        }
        let announce = {
            let mut last = self.last_tip_push.lock().unwrap();
            let due = zcash_offline_core::sync::announce_due(last.0, last.1, now_ms(), source.tip)
                || (last.1 == source.tip && last.2 != source.hash);
            if due { *last = (now_ms(), source.tip, source.hash.clone()); }
            due
        };
        if announce {
            let mut recipients: Vec<_> = self.cfg.fleet.devices.iter().filter(|d| d.name != self.name()).cloned().collect();
            recipients.extend(self.peers.lock().unwrap().values().map(|s| s.announcement.body.device.clone()));
            recipients.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
            recipients.dedup_by(|a, b| a.peer_id == b.peer_id);
            for dev in recipients {
                self.send_device(&dev, Body::Tip { height: source.tip, hash: source.hash.clone() }).await?;
            }
        }
        Ok(())
    }

    fn enrolled_sender(&self, from: &str) -> bool {
        self.cfg.fleet.devices.iter().any(|d| d.peer_id == from || d.name.eq_ignore_ascii_case(from))
    }

    fn admit_sub(&self, from: &str, txid: &str, raw: &[u8], notify: &[String]) -> Result<()> {
        anyhow::ensure!(raw.len() <= 2 * 1024 * 1024 && txid.len() == 64 && hex::decode(txid).is_ok(), "invalid submission size or txid");
        anyhow::ensure!(notify_targets(from, notify.to_vec()).len() <= 16 && notify.iter().all(|n| n.len() <= 128
            && n.strip_prefix("nonce:").is_none_or(|s| s.len() == 32 && hex::decode(s).is_ok())), "invalid notification targets");
        let now = std::time::Instant::now();
        let mut rates = self.sub_rates.lock().unwrap();
        rates.retain(|_, (_, at)| now.duration_since(*at) < Duration::from_secs(5));
        anyhow::ensure!(rates.contains_key(from) || rates.len() < 4096, "gateway sender quota exceeded");
        let (tokens, at) = rates.entry(from.into()).or_insert((4.0, now));
        *tokens = (*tokens + now.duration_since(*at).as_secs_f64()).min(4.0);
        *at = now;
        anyhow::ensure!(*tokens >= 1.0, "gateway rate limit; retry submission");
        *tokens -= 1.0;
        drop(rates);
        let st = self.state.lock().unwrap();
        if let Some(entry) = st.1.gateway.get(txid) {
            let targets = notify_targets(from, notify.to_vec());
            anyhow::ensure!(entry.notify.len() + targets.iter().filter(|n| !entry.notify.contains(n)).count() <= 16, "notification quota exceeded");
            return Ok(());
        }
        let entries = &st.1.gateway;
        anyhow::ensure!(entries.len() < 4096 && entries.values().map(|e| e.tx_hex.len() / 2).sum::<usize>() + raw.len() <= 128 * 1024 * 1024,
            "gateway global queue quota exceeded");
        anyhow::ensure!(entries.values().filter(|e| e.submitter == from || (e.submitter.is_empty() && e.notify.iter().any(|n| n == from))).count() < 256,
            "gateway per-sender queue quota exceeded");
        if !self.enrolled_sender(from) {
            let strangers: Vec<_> = entries.values().filter(|e| !e.enrolled).collect();
            anyhow::ensure!(strangers.len() < 1024 && strangers.iter().map(|e| e.tx_hex.len() / 2).sum::<usize>() + raw.len() <= 32 * 1024 * 1024,
                "gateway stranger queue quota exceeded");
        }
        Ok(())
    }

    async fn on_sub(self: &Arc<Self>, from: &str, tx: Vec<u8>, txid: String, notify: Vec<String>) -> Result<()> {
        let Ok(_guard) = self.gateway_apply.try_lock() else {
            // A payer and its payee submit the same transaction moments apart. The later one joins
            // the running submission and gets its result, instead of waiting for its own retry.
            let mut running = self.sub_in_flight.lock().unwrap();
            if let Some(sub) = running.as_mut().filter(|s| s.txid == txid && s.raw == tx) {
                self.admit_sub(from, &txid, &tx, &notify)?;
                sub.joined.extend(notify_targets(from, notify));
                return Ok(());
            }
            bail!("gateway busy; retry submission");
        };
        self.admit_sub(from, &txid, &tx, &notify)?;
        *self.sub_in_flight.lock().unwrap() = Some(InFlightSub { txid: txid.clone(), raw: tx.clone(), joined: vec![] });
        let r = self.on_sub_locked(from, tx, txid, notify).await;
        // A failed submission leaves no join window open; joined senders retry on their own.
        self.sub_in_flight.lock().unwrap().take();
        r
    }

    /// `on_sub` with `gateway_apply` held.
    async fn on_sub_locked(self: &Arc<Self>, from: &str, tx: Vec<u8>, txid: String, notify: Vec<String>) -> Result<()> {
        if self.cfg.fleet.network == zcash_offline_core::fleet::Network::Testnet {
            return self.on_testnet_sub(from, tx, txid, notify).await;
        }
        let (url, mine) = self.gateway_options();
        let url = url.ok_or_else(|| anyhow!("sub received but this device has no --rpc (not a gateway)"))?;
        let policy = self.cfg.fleet.policy.clone();
        let raw = tx.clone();
        let expected_txid = txid.clone();
        let cached = self.state.lock().unwrap().1.gateway.get(&txid).cloned();
        anyhow::ensure!(cached.as_ref().is_none_or(|e| e.tx_hex == hex::encode(&tx)), "another variant of this txid is already held");
        let t = std::time::Instant::now();
        let (status, expiry_height) = tokio::task::spawn_blocking(move || -> Result<((TxStatus, u32), u32)> {
            let expiry = if let Some(entry) = cached { entry.expiry_height } else {
                let (_, checked) = verify::check_tx(&raw, &policy)?;
                anyhow::ensure!(checked.txid == expected_txid, "submission txid does not match transaction");
                checked.expiry_height
            };
            let rpc = Rpc::new(&url);
            Ok((submit_and_mine(&rpc, &raw, &expected_txid, mine)?, expiry))
        })
        .await??;
        let elapsed = t.elapsed().as_millis();
        let mut to: Vec<String> = notify_targets(from, notify).into_iter().chain(self.joined_submitters())
            .filter(|n| n != self.name()).collect();
        to.sort();
        to.dedup();
        let res = {
            let mut st = self.state.lock().unwrap();
            st.1.gw_seq += 1;
            let seq = st.1.gw_seq;
            let res = Signed::sign(
                GatewayResult { txid: txid.clone(), status: status.0.clone(), tip_height: status.1, gateway: self.name().into(), seq, created_ms: now_ms() },
                &self.cfg.me,
            )?;
            let entry = st.1.gateway.entry(txid.clone()).or_insert_with(|| GatewayEntry { txid: txid.clone(), notify: to.clone(), result: res.clone(), submissions: 0,
                submitter: from.into(), enrolled: self.enrolled_sender(from),
                tx_hex: hex::encode(&tx), expiry_height, last_submit_height: status.1 });
            entry.submissions += 1;
            entry.result = res.clone();
            for n in &to {
                if !entry.notify.contains(n) {
                    entry.notify.push(n.clone());
                }
            }
            self.persist(&st.0, &st.1)?;
            res
        };
        self.event("gateway-result", json!({"txid": txid, "from": from, "status": status.0, "tip": status.1, "ms": elapsed}));
        if self.payment(&txid).is_some() { self.on_result(self.name(), res.clone())?; }
        for n in to {
            if let Err(e) = self.send_result(&n, res.clone()).await {
                self.event("result-undelivered", json!({"txid": txid, "to": n, "error": e.to_string()}));
            }
        }
        Ok(())
    }

    // ---------- payer and payee: results ----------

    /// Targets of identical submissions that joined the one being processed; ends the join window.
    fn joined_submitters(&self) -> Vec<String> {
        self.sub_in_flight.lock().unwrap().take().map(|s| s.joined).unwrap_or_default()
    }

    async fn send_result(&self, target: &str, result: Signed<GatewayResult>) -> Result<()> {
        if let Some(nonce) = target.strip_prefix("nonce:") {
            anyhow::ensure!(nonce.len() == 32 && hex::decode(nonce).is_ok(), "invalid request nonce");
            let notice = Signed::sign(GatewayNotice { request_nonce: nonce.into(), result }, &self.cfg.me)?;
            let mut bytes = RESULT_MAGIC.to_vec();
            bytes.extend(serde_json::to_vec(&notice)?);
            self.mesh.broadcast(bytes).await?;
            Ok(())
        } else {
            self.send_to(target, Body::Res(result)).await
        }
    }

    async fn handle_result(self: &Arc<Self>, from: &str, r: Signed<GatewayResult>) -> Result<()> {
        let included = matches!(r.body.status, TxStatus::Included { .. } | TxStatus::Mined { .. });
        let txid = r.body.txid.clone();
        self.on_result(from, r)?;
        if included {
            if let Some(p) = self.payment(&txid).filter(|p| p.direction == "received") {
                if let (Some(peer), Some(receipt)) = (p.receipt_peer, p.receipt) {
                    let _ = self.send_to(&peer, Body::Rcpt(receipt)).await;
                }
            }
            if self.gateway_options().0.is_none() { let _ = self.request_mesh_sync(true).await; }
        }
        Ok(())
    }

    fn on_result(&self, from: &str, r: Signed<GatewayResult>) -> Result<()> {
        r.verify_key(&self.device(from)?)?;
        self.require_gateway(from)?;
        let txid = r.body.txid.clone();
        let p = self.payment(&txid).ok_or_else(|| anyhow!("result for unknown payment {txid}"))?;
        anyhow::ensure!(p.result.as_ref().is_none_or(|old| old.signer != r.signer || old.body.seq < r.body.seq), "stale gateway result");
        let included = if p.direction == "received" && matches!(r.body.status, TxStatus::Mined { .. } | TxStatus::Included { .. }) {
            Some(Signed::sign(Receipt { txid: txid.clone(), auth_digest: p.auth_digest,
                payee: self.name().into(), amount: p.amount, state: ReceiptState::Included,
                view_height: r.body.tip_height, created_ms: now_ms() }, &self.cfg.me)?)
        } else { None };
        let (state, text) = match &r.body.status {
            TxStatus::Mined { height, block_hash } => ("mined", format!("mined at height {height} in block {block_hash} (gateway-reported by {from})")),
            TxStatus::Mempool => ("in-mempool", format!("in {from}'s mempool")),
            TxStatus::Rejected { reason } => ("gateway-rejected", format!("rejected by {from}: {reason}")),
            TxStatus::Included { height, block_hash } => ("included", format!("included at height {height} in block {block_hash} (gateway-reported by {from}; fewer than 10 confirmations)")),
            TxStatus::Submitted => ("submitted", format!("submitted by {from}; not yet included")),
            TxStatus::Retryable { reason } => ("retrying", format!("retryable submission error from {from}: {reason}")),
            TxStatus::Expired => ("expired", format!("expired according to {from}; locks release only after local sync passes expiry")),
            TxStatus::Conflict { reason } => ("conflict", format!("conflict reported by {from}: {reason}")),
        };
        self.event("result", json!({"txid": txid, "from": from, "state": state}));
        self.update(&txid, |p| {
            p.state = state.into();
            p.note(text);
            if included.is_some() { p.receipt = included; }
            p.result = Some(r);
        })
    }

    // ---------- retry loop ----------

    /// App-level retry keyed by txid, on top of mesh store-and-forward: A re-sends
    /// until it has a receipt, B re-submits until mined. Backoff doubles per attempt
    /// from `retry` up to 32×.
    pub async fn retry_tick(self: &Arc<Self>) {
        let now = now_ms();
        let base = self.cfg.retry.as_millis() as u64;
        let due: Vec<Payment> = {
            let st = self.state.lock().unwrap();
            st.1.payments
                .values()
                .filter(|p| !p.is_final() && p.test_tamper.is_none())
                .filter(|p| now.saturating_sub(p.last_send_ms) >= base * (1u64 << p.attempts.saturating_sub(1).min(5)))
                .cloned()
                .collect()
        };
        for p in due {
            let r = if p.direction == "sent" {
                self.send_payment(&p.txid).await
            } else {
                let tx = hex::decode(&p.tx_hex).unwrap_or_default();
                let mut r = Ok(());
                for g in self.gateways() {
                    r = match self.submission(&p.txid, tx.clone()) {
                        Ok(body) => r.and(self.send_to(&g, body).await),
                        Err(e) => Err(e),
                    };
                }
                let _ = self.update(&p.txid, |p| {
                    p.attempts += 1;
                    p.last_send_ms = now_ms();
                    p.note(format!("re-submitted to gateway (attempt {})", p.attempts));
                });
                r
            };
            if let Err(e) = r {
                warn!("retry {}: {e:#}", p.txid);
            }
        }
    }

    pub fn status(&self) -> serde_json::Value {
        let st = self.state.lock().unwrap();
        let (rpc, mine) = self.gateway_options();
        let (spendable, locked) = st.0.balance();
        let neighbors: Vec<String> = self.mesh.neighbors().into_iter().map(|p| self.peer_name(p)).collect();
        let me = self.cfg.fleet.by_name(self.name()).ok();
        let sources = self.sync_sources.lock().unwrap().clone();
        json!({
            "name": self.name(),
            "role": if me.is_some_and(|d| d.is_gateway()) { "gateway" } else { "" },
            "version": env!("CARGO_PKG_VERSION"),
            "network": self.cfg.fleet.network,
            "birthday": self.cfg.fleet.birthday,
            "chain_sources": sources,
            "single_source": sources.len() < 2,
            "peer_id": self.mesh.peer_id().to_string(),
            "address": me.map(|d| d.address.clone()),
            "online": rpc.is_some(),
            "chain_tip": self.reported_tip.load(std::sync::atomic::Ordering::Relaxed),
            "open_network": self.cfg.fleet.open,
            "selected_gateway": self.gateway_status(),
            "gateway_warning": self.gateway_warning(),
            "trust": "gateway-reported",
            "rpc": rpc,
            "mine": mine,
            "ble": self.ble.get().is_some(),
            "ble_running": self.ble.get().is_some_and(ozm_ble::Ble::is_running),
            "neighbors": neighbors,
            "neighbor_links": self.mesh.neighbor_links().into_iter().map(|(p, kind)| json!({"name": self.peer_name(p), "peer_id": p.to_string(), "kind": kind})).collect::<Vec<_>>(),
            "mesh": self.mesh.stats(),
            "wallet": {"tip": st.0.tip_height, "spendable": spendable, "locked": locked,
                        "notes": st.0.notes.iter().filter(|n| n.spent_in.is_none()).count()},
            "payments": st.1.payments.len(),
            "refusals": st.1.refusals.len(),
            "gateway_submissions": st.1.gateway.len(),
            "policy": self.cfg.fleet.policy,
        })
    }
}

fn notify_targets(from: &str, mut notify: Vec<String>) -> Vec<String> {
    if !notify.iter().any(|n| n.starts_with("nonce:")) { notify.push(from.into()); }
    notify.sort();
    notify.dedup();
    notify
}

/// Submits raw bytes (idempotent: "already in mempool" and "already mined" are
/// success), mines one block on Regtest if asked, and reports where the tx is.
pub fn submit_and_mine(rpc: &Rpc, raw: &[u8], txid: &str, mine: bool) -> Result<(TxStatus, u32)> {
    match rpc.call("sendrawtransaction", json!([hex::encode(raw)])) {
        Ok(_) => {}
        Err(e) => {
            if let Some(error) = e.downcast_ref::<RpcError>() {
                use zcash_offline_core::rpc::{classify_submit, SubmitDisposition::*};
                match classify_submit(error.code, &error.message) {
                    Accepted | Duplicate => {},
                    Retryable => return Ok((TxStatus::Retryable { reason: error.message.clone() }, rpc.tip()?)),
                    Conflict => return Ok((TxStatus::Conflict { reason: error.message.clone() }, rpc.tip()?)),
                    Rejected => return Ok((TxStatus::Rejected { reason: error.message.clone() }, rpc.tip()?)),
                }
            } else {
                return Err(e);
            }
        }
    }
    let lookup = |rpc: &Rpc| -> Result<Option<(u32, String)>> {
        let v = rpc.call("getrawtransaction", json!([txid, 1]))?;
        match v["height"].as_u64() {
            Some(h) if h > 0 => {
                let hash = v["blockhash"].as_str().map(str::to_string).map_or_else(|| rpc.block_hash(h as u32), Ok)?;
                Ok(Some((h as u32, hash)))
            }
            _ => Ok(None),
        }
    };
    if lookup(rpc)?.is_none() && mine {
        rpc.call("generate", json!([1])).context("generate")?;
    }
    let tip = rpc.tip()?;
    Ok(match lookup(rpc)? {
        Some((height, block_hash)) => (TxStatus::Mined { height, block_hash }, tip),
        None => (TxStatus::Mempool, tip),
    })
}
