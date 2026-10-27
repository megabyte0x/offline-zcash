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
    DeviceSecret, Fleet,
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
}

impl Payment {
    fn note(&mut self, event: impl Into<String>) {
        self.history.push(HistoryEntry { ms: now_ms(), event: event.into() });
    }
    fn is_final(&self) -> bool {
        matches!(self.state.as_str(), "mined" | "refused" | "gateway-rejected")
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
}

/// Everything this device persists besides the wallet.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Book {
    pub payments: BTreeMap<String, Payment>,
    pub refusals: Vec<Refusal>,
    pub gateway: BTreeMap<String, GatewayEntry>,
    pub seq_out: u64,
    pub last_seq_in: BTreeMap<String, u64>,
    #[serde(default)]
    pub gw_seq: u64,
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
    pub x25519: x25519_dalek::StaticSecret,
    pub state: Mutex<(WalletState, Book)>,
    /// Serializes proving so two pays never pick the same note.
    pay_lock: tokio::sync::Mutex<()>,
    /// Gateway: blocks already read from the node, by height (checked against the node's hash).
    block_cache: Mutex<BTreeMap<u32, BlockData>>,
    /// Device: when the last mesh sync request went out (ms), to avoid flooding the gateway.
    last_sync_req: std::sync::atomic::AtomicU64,
    sync_apply: tokio::sync::Mutex<()>,
    /// Optional observer of every app event (the Basecamp module turns them into Logos events).
    pub on_event: std::sync::OnceLock<EventHook>,
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
        let wallet = WalletState::load(&wallet_path(&cfg))?;
        let book: Book = match std::fs::read(book_path(&cfg)) {
            Ok(b) => serde_json::from_slice(&b)?,
            Err(_) => Book::default(),
        };
        let x25519 = cfg.me.x25519_secret()?;
        Ok(Arc::new(App { cfg, mesh, x25519, state: Mutex::new((wallet, book)),
            pay_lock: Default::default(),
            block_cache: Default::default(),
            last_sync_req: Default::default(),
            sync_apply: Default::default(),
            on_event: Default::default(),
        }))
    }

    pub fn name(&self) -> &str {
        &self.cfg.me.name
    }

    fn persist(&self, wallet: &WalletState, book: &Book) -> Result<()> {
        wallet.save(&wallet_path(&self.cfg))?;
        write_atomic(&book_path(&self.cfg), &serde_json::to_vec_pretty(book)?)
    }

    /// Appends a structured event to `events.jsonl` and the log. The bench reads these for timings.
    pub fn event(&self, event: &str, detail: serde_json::Value) {
        if let Some(f) = self.on_event.get() {
            f(event, &detail);
        }
        let line = json!({"ms": now_ms(), "node": self.name(), "event": event, "detail": detail});
        info!(target: "ozm", "{line}");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.cfg.data.join("events.jsonl")) {
            let _ = writeln!(f, "{line}");
        }
    }

    async fn send_to(&self, name: &str, body: Body) -> Result<()> {
        let dev = self.cfg.fleet.by_name(name)?;
        let kind = body.kind();
        let inner = Inner {
            from: self.name().to_string(),
            from_key: hex::encode(self.cfg.me.signing_key()?.verifying_key().to_bytes()),
            body,
        };
        let sealed = envelope::seal(&inner, &dev.x25519()?)?;
        let peer: PeerId = dev.peer_id.parse()?;
        let msg_id = self.mesh.send(peer, sealed.clone()).await?;
        self.event("mesh-send", json!({"kind": kind, "to": name, "bytes": sealed.len(), "msg_id": msg_id.to_string(), "envelope_id": inner.envelope_id()}));
        Ok(())
    }

    fn gateways(&self) -> Vec<String> {
        self.cfg.fleet.gateways().map(|d| d.name.clone()).filter(|n| n != self.name()).collect()
    }

    // ---------- online ----------

    /// Replaces the wallet with a freshly synced copy, keeping locks taken by
    /// payments built while the sync ran, and persists it.
    fn install_wallet(&self, mut wallet: WalletState) -> Result<(u64, u64, u32)> {
        let mut st = self.state.lock().unwrap();
        for n in wallet.notes.iter_mut() {
            if let Some(old) = st.0.notes.iter().find(|o| o.nullifier == n.nullifier) {
                if n.spent_in.is_none() && old.locked_by.is_some() {
                    n.locked_by = old.locked_by.clone();
                }
            }
        }
        st.0 = wallet;
        self.persist(&st.0, &st.1)?;
        let (spendable, locked) = st.0.balance();
        Ok((spendable, locked, st.0.tip_height))
    }

    /// Online devices sync from Zakura RPC; offline devices ask a gateway over the mesh.
    pub async fn sync(self: &Arc<Self>) -> Result<serde_json::Value> {
        let Some(url) = self.cfg.rpc.clone() else {
            let gw = self.request_mesh_sync(true).await?;
            return Ok(json!({"requested_from": gw, "tip": self.state.lock().unwrap().0.tip_height}));
        };
        let _g = self.sync_apply.lock().await;
        let mut wallet = self.state.lock().unwrap().0.clone();
        let fvk = self.cfg.me.fvk()?;
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
        self.send_to(&gw, Body::SyncReq { have_height, have_hash }).await?;
        Ok(gw)
    }

    // ---------- gateway: chain data and test funds over the mesh ----------

    async fn on_sync_req(self: &Arc<Self>, from: &str, have_height: u32, have_hash: String) -> Result<()> {
        let url = self.cfg.rpc.clone().ok_or_else(|| anyhow!("syncq received but this device is not online"))?;
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
        anyhow::ensure!(self.cfg.fleet.by_name(from)?.role.contains("gateway"), "{from} is not a gateway");
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
        let url = self.cfg.rpc.clone().ok_or_else(|| anyhow!("this gateway is not online"))?;
        anyhow::ensure!(self.cfg.mine, "this gateway does not mine (faucet only runs on Regtest gateways)");
        anyhow::ensure!(notes >= 1 && amount_each > 0, "nothing requested");
        let total = notes as u64 * amount_each;
        anyhow::ensure!(total <= self.cfg.faucet_max_zat, "request of {total} zat is above this gateway's faucet limit {}", self.cfg.faucet_max_zat);
        let to_addr = self.cfg.fleet.by_name(to)?.orchard_address()?;
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
            let (raw, txid, mine) = (built.raw.clone(), built.txid.clone(), self.cfg.mine);
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

    /// Debug only: builds a payment and breaks one thing, so the payee's refusal
    /// can be tested end to end. `tamper`: amount, payee, sig, branch, expired.
    pub async fn pay_tampered(self: &Arc<Self>, to: &str, amount: u64, tamper: &str) -> Result<Payment> {
        anyhow::ensure!(["amount", "payee", "sig", "branch", "expired"].contains(&tamper), "unknown tamper {tamper}");
        self.pay_inner(to, amount, Some(tamper.to_string())).await
    }

    async fn pay_inner(self: &Arc<Self>, to: &str, amount: u64, tamper: Option<String>) -> Result<Payment> {
        let _guard = self.pay_lock.lock().await;
        let payee = self.cfg.fleet.by_name(to)?.clone();
        anyhow::ensure!(payee.name != self.name(), "cannot pay yourself");
        let mut policy = self.cfg.fleet.policy.clone();
        let t = tamper.as_deref();
        if t == Some("expired") {
            policy.expiry_delta = 5;
        }
        let me = self.cfg.me.clone();
        let mut wallet = self.state.lock().unwrap().0.clone();
        let to_addr = if t == Some("payee") { DeviceSecret::generate("X", "payee").address()? } else { payee.orchard_address()? };
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
                    payee: payee.name.clone(),
                    seq: st.1.seq_out,
                    created_ms: now_ms(),
                },
                &self.cfg.me,
            )?;
            let mut p = Payment {
                txid: built.txid.clone(),
                auth_digest: built.auth_digest.clone(),
                direction: "sent".into(),
                counterparty: payee.name.clone(),
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
                last_send_ms: 0,
                attempts: 0,
                test_tamper: tamper.clone(),
            };
            p.note(format!("built offline: {} bytes, proved in {} ms, expiry {}", built.raw.len(), built.prove_ms, built.expiry_height));
            st.1.payments.insert(p.txid.clone(), p.clone());
            // Persist the lock and the payment before anything leaves the device.
            self.persist(&st.0, &st.1)?;
            p
        };
        self.event("built", json!({"txid": payment.txid, "bytes": built.raw.len(), "prove_ms": built.prove_ms, "amount": amount, "to": payee.name}));
        self.send_payment(&payment.txid).await?;
        Ok(self.payment(&payment.txid).unwrap())
    }

    /// Debug only: re-sends a payment as new envelopes (duplicate and idempotence tests).
    pub async fn resend(self: &Arc<Self>, txid: &str) -> Result<()> {
        let p = self.payment(txid).ok_or_else(|| anyhow!("unknown payment"))?;
        let tx = hex::decode(&p.tx_hex)?;
        if let Some(st) = p.statement.clone().filter(|_| p.direction == "sent") {
            self.send_to(&p.counterparty, Body::Pay { tx: tx.clone(), statement: st }).await?;
        }
        for g in self.gateways() {
            self.send_to(&g, Body::Sub { tx: tx.clone(), txid: p.txid.clone(), notify: vec![self.name().into(), p.counterparty.clone()] }).await?;
        }
        Ok(())
    }

    /// Sends (or re-sends) `pay` to the payee and `sub` to the gateways. Both A and B submit.
    async fn send_payment(self: &Arc<Self>, txid: &str) -> Result<()> {
        let p = self.payment(txid).ok_or_else(|| anyhow!("unknown payment"))?;
        let tx = hex::decode(&p.tx_hex)?;
        let statement = p.statement.clone().ok_or_else(|| anyhow!("no statement"))?;
        if p.receipt.is_none() {
            self.send_to(&p.counterparty, Body::Pay { tx: tx.clone(), statement }).await?;
        }
        for g in self.gateways().into_iter().filter(|_| p.test_tamper.is_none()) {
            self.send_to(&g, Body::Sub { tx: tx.clone(), txid: p.txid.clone(), notify: vec![self.name().into(), p.counterparty.clone()] }).await?;
        }
        self.update(txid, |p| {
            if p.state == "built" {
                p.state = "sent".into();
            }
            p.attempts += 1;
            p.last_send_ms = now_ms();
            p.note(format!("sent over mesh (attempt {})", p.attempts));
        })
    }

    // ---------- shared ----------

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
            MeshEvent::Message { from, payload, hops, msg_id, .. } => {
                let inner = match envelope::open(&payload, &self.x25519) {
                    Ok(i) => i,
                    Err(e) => {
                        warn!("dropping message {msg_id} from {from}: {e}");
                        return;
                    }
                };
                // The envelope's sender must be the fleet device that owns the originating mesh ID.
                let dev = self.cfg.fleet.by_peer(&from.to_string());
                if dev.map(|d| (d.name.as_str(), d.ed25519.as_str())) != Some((inner.from.as_str(), inner.from_key.as_str())) {
                    warn!("dropping message {msg_id}: sender {} does not match mesh peer {from}", inner.from);
                    return;
                }
                self.event("mesh-recv", json!({"kind": inner.body.kind(), "from": inner.from, "hops": hops, "bytes": payload.len(), "msg_id": msg_id.to_string(), "envelope_id": inner.envelope_id()}));
                let from_name = inner.from.clone();
                let r = match inner.body {
                    Body::Pay { tx, statement } => self.on_pay(&from_name, tx, statement).await,
                    Body::Rcpt(r) => self.on_receipt(&from_name, r),
                    Body::Sub { tx, txid, notify } => self.on_sub(&from_name, tx, txid, notify).await,
                    Body::Res(r) => {
                        let mined = matches!(r.body.status, TxStatus::Mined { .. });
                        let out = self.on_result(&from_name, r);
                        // A mined payment changes balances: refresh the chain view from the gateway.
                        if mined && out.is_ok() && self.cfg.rpc.is_none() {
                            let _ = self.request_mesh_sync(false).await;
                        }
                        out
                    }
                    Body::SyncReq { have_height, have_hash } => self.on_sync_req(&from_name, have_height, have_hash).await,
                    Body::SyncRes { reset, prev_hash, blocks, chain_tip } => self.on_sync_res(&from_name, reset, prev_hash, blocks, chain_tip).await,
                    Body::FundReq { notes, amount_each } => self.on_fund_req(&from_name, notes, amount_each).await,
                    Body::FundRes { ok, detail } => {
                        self.event("funds", json!({"from": from_name, "ok": ok, "detail": detail}));
                        if ok {
                            let _ = self.request_mesh_sync(true).await;
                        }
                        Ok(())
                    }
                };
                if let Err(e) = r {
                    warn!("handling message from {from_name}: {e:#}");
                    self.event("error", json!({"from": from_name, "error": format!("{e:#}")}));
                }
            }
            MeshEvent::Delivered { msg_id, by } => {
                let by_name = self.cfg.fleet.by_peer(&by.to_string()).map(|d| d.name.clone()).unwrap_or(by.to_string());
                self.event("mesh-delivered", json!({"msg_id": msg_id.to_string(), "by": by_name}));
                // Mark sent payments to that payee as delivered (the mesh-level ack).
                let txids: Vec<String> = {
                    let st = self.state.lock().unwrap();
                    st.1.payments.values().filter(|p| p.direction == "sent" && p.counterparty == by_name && p.state == "sent").map(|p| p.txid.clone()).collect()
                };
                for t in txids {
                    let _ = self.update(&t, |p| {
                        p.state = "delivered".into();
                        p.note(format!("mesh ack from {by_name}"));
                    });
                }
            }
            MeshEvent::NeighborUp(p) => self.event("neighbor-up", json!({"peer": self.peer_name(p)})),
            MeshEvent::NeighborDown(p) => self.event("neighbor-down", json!({"peer": self.peer_name(p)})),
        }
    }

    pub fn peer_name(&self, p: PeerId) -> String {
        self.cfg.fleet.by_peer(&p.to_string()).map(|d| d.name.clone()).unwrap_or(p.to_string())
    }

    // ---------- payee ----------

    async fn verify_offline(&self, tx: &[u8]) -> Result<Result<verify::Verified, Reject>> {
        let policy = self.cfg.fleet.policy.clone();
        let me = self.cfg.me.clone();
        let wallet = self.state.lock().unwrap().0.clone();
        let raw = tx.to_vec();
        Ok(tokio::task::spawn_blocking(move || verify::verify_payment(&raw, &me, &wallet, &policy)).await?)
    }

    async fn on_pay(self: &Arc<Self>, from: &str, tx: Vec<u8>, statement: Signed<Statement>) -> Result<()> {
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
        let mut checked = self.verify_offline(&tx).await?;
        // The payer may have synced past this payee's view. An offline payee refreshes its
        // chain view from a gateway over the mesh and checks once more before refusing.
        if self.cfg.rpc.is_none() && matches!(&checked, Err(r) if r.code == code::ANCHOR) {
            let before = self.state.lock().unwrap().0.tip_height;
            self.event("anchor-unknown-syncing", json!({"txid": s.txid, "view_height": before}));
            if self.request_mesh_sync(true).await.is_ok() {
                for _ in 0..60 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if self.state.lock().unwrap().0.tip_height > before {
                        break;
                    }
                }
                checked = self.verify_offline(&tx).await?;
            }
        }
        let verify_ms = t.elapsed().as_millis();
        let result = checked.and_then(|v| {
            let st_err = |r: &str| Reject { code: code::STATEMENT, reason: r.to_string() };
            statement.verify(&self.cfg.fleet, from).map_err(|e| st_err(&format!("statement: {e}")))?;
            if s.payee != self.name() {
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
                let receipt = Signed::sign(
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
                )?;
                {
                    let mut st = self.state.lock().unwrap();
                    st.1.last_seq_in.insert(from.to_string(), s.seq);
                    let mut p = Payment {
                        txid: v.tx.txid.clone(),
                        auth_digest: v.tx.auth_digest.clone(),
                        direction: "received".into(),
                        counterparty: from.to_string(),
                        amount: v.amount,
                        fee: v.tx.fee,
                        expiry_height: v.tx.expiry_height,
                        state: "accepted-pending".into(),
                        tx_hex: hex::encode(&tx),
                        nullifiers: v.tx.nullifiers.clone(),
                        statement: Some(statement.clone()),
                        receipt: Some(receipt.clone()),
                        result: None,
                        history: vec![],
                        created_ms: now_ms(),
                        last_send_ms: now_ms(),
                        attempts: 1,
                        test_tamper: None,
                    };
                    p.note(format!("checked offline in {verify_ms} ms at view height {}; anchor height {}; pending", v.view_height, v.anchor_height));
                    st.1.payments.insert(p.txid.clone(), p);
                    // fsync before the receipt leaves this device.
                    self.persist(&st.0, &st.1)?;
                }
                self.event("accepted", json!({"txid": v.tx.txid, "amount": v.amount, "from": from, "verify_ms": verify_ms}));
                self.send_to(from, Body::Rcpt(receipt)).await?;
                for g in self.gateways() {
                    self.send_to(&g, Body::Sub { tx: tx.clone(), txid: v.tx.txid.clone(), notify: vec![from.to_string(), self.name().into()] }).await?;
                }
                self.update(&v.tx.txid, |p| {
                    p.state = "submitted".into();
                    p.note("receipt sent; forwarded to gateway");
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
        r.verify(&self.cfg.fleet, from)?;
        let txid = r.body.txid.clone();
        let Some(p) = self.payment(&txid) else { bail!("receipt for unknown payment {txid}") };
        if p.counterparty != from {
            bail!("receipt from {from}, but {txid} was paid to {}", p.counterparty);
        }
        let state = match &r.body.state {
            ReceiptState::Pending => "accepted-pending".to_string(),
            ReceiptState::Refused(rej) => format!("refused: {rej}"),
        };
        self.event("receipt", json!({"txid": txid, "from": from, "state": state}));
        self.update(&txid, |p| {
            if !p.is_final() {
                p.state = match r.body.state {
                    ReceiptState::Pending => "accepted-pending".into(),
                    ReceiptState::Refused(_) => "refused".into(),
                };
            }
            p.note(format!("receipt from {from}: {state}"));
            p.receipt = Some(r);
        })
    }

    // ---------- gateway ----------

    async fn on_sub(self: &Arc<Self>, from: &str, tx: Vec<u8>, txid: String, notify: Vec<String>) -> Result<()> {
        let url = self.cfg.rpc.clone().ok_or_else(|| anyhow!("sub received but this device has no --rpc (not a gateway)"))?;
        let policy = self.cfg.fleet.policy.clone();
        let mine = self.cfg.mine;
        let raw = tx.clone();
        let t = std::time::Instant::now();
        let status = tokio::task::spawn_blocking(move || -> Result<(TxStatus, u32)> {
            let checked = verify::check_tx(&raw, &policy);
            let rpc = Rpc::new(&url);
            let checked = match checked {
                Ok((_, c)) => c,
                Err(rej) => return Ok((TxStatus::Rejected { reason: rej.to_string() }, rpc.tip().unwrap_or(0))),
            };
            submit_and_mine(&rpc, &raw, &checked.txid, mine)
        })
        .await??;
        let elapsed = t.elapsed().as_millis();
        let mut to: Vec<String> = notify.into_iter().chain(std::iter::once(from.to_string())).filter(|n| n != self.name()).collect();
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
            let entry = st.1.gateway.entry(txid.clone()).or_insert_with(|| GatewayEntry { txid: txid.clone(), notify: to.clone(), result: res.clone(), submissions: 0 });
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
        for n in to {
            self.send_to(&n, Body::Res(res.clone())).await?;
        }
        Ok(())
    }

    // ---------- payer and payee: results ----------

    fn on_result(&self, from: &str, r: Signed<GatewayResult>) -> Result<()> {
        r.verify(&self.cfg.fleet, from)?;
        anyhow::ensure!(self.cfg.fleet.by_name(from)?.role.contains("gateway"), "{from} is not a gateway");
        let txid = r.body.txid.clone();
        anyhow::ensure!(self.payment(&txid).is_some(), "result for unknown payment {txid}");
        let (state, text) = match &r.body.status {
            TxStatus::Mined { height, block_hash } => ("mined", format!("mined at height {height} in block {block_hash} (gateway-reported by {from})")),
            TxStatus::Mempool => ("in-mempool", format!("in {from}'s mempool")),
            TxStatus::Rejected { reason } => ("gateway-rejected", format!("rejected by {from}: {reason}")),
        };
        self.event("result", json!({"txid": txid, "from": from, "state": state}));
        self.update(&txid, |p| {
            if p.state != "mined" {
                p.state = state.into();
            }
            p.note(text);
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
                    r = r.and(self.send_to(&g, Body::Sub { tx: tx.clone(), txid: p.txid.clone(), notify: vec![p.counterparty.clone(), self.name().into()] }).await);
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
        let (spendable, locked) = st.0.balance();
        let neighbors: Vec<String> = self.mesh.neighbors().into_iter().map(|p| self.peer_name(p)).collect();
        let me = self.cfg.fleet.by_name(self.name()).ok();
        json!({
            "name": self.name(),
            "role": self.cfg.me.role,
            "version": env!("CARGO_PKG_VERSION"),
            "peer_id": self.mesh.peer_id().to_string(),
            "address": me.map(|d| d.address.clone()),
            "online": self.cfg.rpc.is_some(),
            "neighbors": neighbors,
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

/// Submits raw bytes (idempotent: "already in mempool" and "already mined" are
/// success), mines one block on Regtest if asked, and reports where the tx is.
pub fn submit_and_mine(rpc: &Rpc, raw: &[u8], txid: &str, mine: bool) -> Result<(TxStatus, u32)> {
    match rpc.call("sendrawtransaction", json!([hex::encode(raw)])) {
        Ok(_) => {}
        Err(e) => {
            let msg = e.downcast_ref::<RpcError>().map(|r| r.message.clone());
            match msg {
                Some(m) if m.contains("already exists in mempool") || m.contains("committed to the best chain") || m.contains("already") => {}
                Some(m) => return Ok((TxStatus::Rejected { reason: m }, rpc.tip()?)),
                None => return Err(e),
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
