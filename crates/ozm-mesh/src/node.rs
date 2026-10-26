//! Mesh logic: links table, dedup, flooding, store-and-forward, acks.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, ensure, Result};
use tokio::{sync::mpsc, task::AbortHandle};
use tracing::{debug, info, warn};

use crate::{
    link::{LinkTx, Outgoing, MAX_FRAGMENTS},
    packet::{Kind, LinkMsg, Packet, HEADER_LEN},
    persist::{self, Db, Stored},
    MeshConfig, MeshEvent, MeshStats, MsgId, PeerId,
};

#[derive(Default)]
pub(crate) struct Counters {
    frames_sent: AtomicU64,
    bytes_sent: AtomicU64,
    frames_received: AtomicU64,
    bytes_received: AtomicU64,
    packets_relayed: AtomicU64,
    packets_synced: AtomicU64,
    messages_delivered: AtomicU64,
}

impl Counters {
    pub fn frame_sent(&self, bytes: usize) {
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn frame_received(&self, bytes: usize) {
        self.frames_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

struct LinkEntry {
    id: u64,
    outbound: bool,
    tx: LinkTx,
}

struct State {
    links: HashMap<PeerId, LinkEntry>,
    db: Db,
    events: Option<mpsc::UnboundedSender<MeshEvent>>,
}

pub(crate) struct Node {
    pub cfg: MeshConfig,
    pub me: PeerId,
    pub stats: Counters,
    closed: AtomicBool,
    state: Mutex<State>,
    tasks: Mutex<Vec<AbortHandle>>,
    next_link_id: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Node {
    pub fn new(cfg: MeshConfig, mut db: Db, events: mpsc::UnboundedSender<MeshEvent>) -> Self {
        db.prune(now_ms());
        Node {
            me: cfg.peer_id,
            cfg,
            stats: Counters::default(),
            closed: AtomicBool::new(false),
            state: Mutex::new(State { links: HashMap::new(), db, events: Some(events) }),
            tasks: Mutex::new(Vec::new()),
            next_link_id: AtomicU64::new(1),
        }
    }

    fn store_ttl_ms(&self) -> u64 {
        self.cfg.store_ttl.as_millis().min(u64::MAX as u128) as u64
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Spawns a task that `shutdown` aborts.
    pub fn spawn<F: Future<Output = ()> + Send + 'static>(&self, f: F) {
        let handle = tokio::spawn(f).abort_handle();
        let mut tasks = lock(&self.tasks);
        tasks.retain(|h| !h.is_finished());
        if self.is_closed() {
            handle.abort();
        } else {
            tasks.push(handle);
        }
    }

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        {
            let mut st = lock(&self.state);
            st.links.clear();
            st.events = None;
            self.persist(&st);
        }
        for h in lock(&self.tasks).drain(..) {
            h.abort();
        }
        info!(peer = %self.me, "mesh stopped");
    }

    pub fn neighbors(&self) -> Vec<PeerId> {
        let mut v: Vec<_> = lock(&self.state).links.keys().copied().collect();
        v.sort();
        v
    }

    pub fn is_connected(&self, peer: PeerId) -> bool {
        lock(&self.state).links.contains_key(&peer)
    }

    pub fn stats(&self) -> MeshStats {
        let (stored, neighbors) = {
            let st = lock(&self.state);
            (st.db.store.len(), st.links.len())
        };
        let c = &self.stats;
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        MeshStats {
            frames_sent: get(&c.frames_sent),
            bytes_sent: get(&c.bytes_sent),
            frames_received: get(&c.frames_received),
            bytes_received: get(&c.bytes_received),
            packets_relayed: get(&c.packets_relayed),
            packets_synced: get(&c.packets_synced),
            messages_delivered: get(&c.messages_delivered),
            stored,
            neighbors,
        }
    }

    /// Registers a link after hello. Returns its id, or `None` if it is a duplicate we drop.
    ///
    /// One link per neighbour: if one exists, the new link replaces it when both were dialed by the
    /// same side (a reconnect), otherwise the link dialed by the smaller PeerId wins. Both ends apply
    /// the same rule, so they keep the same TCP connection.
    pub fn link_up(&self, peer: PeerId, outbound: bool, tx: LinkTx) -> Option<u64> {
        let mut st = lock(&self.state);
        if self.is_closed() {
            return None;
        }
        let dialer = |out: bool| if out { self.me } else { peer };
        let id = self.next_link_id.fetch_add(1, Ordering::Relaxed);
        if let Some(old) = st.links.get(&peer) {
            let new_dialer = dialer(outbound);
            if new_dialer != dialer(old.outbound) && new_dialer != self.me.min(peer) {
                debug!(%peer, "duplicate link dropped");
                return None;
            }
            info!(%peer, outbound, "link replaced");
            st.links.insert(peer, LinkEntry { id, outbound, tx });
        } else {
            info!(%peer, outbound, "link up");
            st.links.insert(peer, LinkEntry { id, outbound, tx });
            emit(&st, MeshEvent::NeighborUp(peer));
        }
        // Inventory: everything we have already seen, plus message ids known to be acked.
        let inv = LinkMsg::Inventory {
            held: st.db.seen.keys().copied().collect(),
            acked: st.db.acked.keys().copied().collect(),
        };
        let _ = st.links[&peer].tx.send(Outgoing { frag_id: rand::random(), body: Arc::new(inv.encode()) });
        Some(id)
    }

    pub fn link_down(&self, peer: PeerId, link_id: u64, err: Option<anyhow::Error>) {
        let mut st = lock(&self.state);
        if st.links.get(&peer).is_some_and(|l| l.id == link_id) {
            st.links.remove(&peer);
            match err {
                Some(e) => info!(%peer, error = %e, "link down"),
                None => info!(%peer, "link down"),
            }
            emit(&st, MeshEvent::NeighborDown(peer));
        } else {
            debug!(%peer, "superseded link closed");
        }
    }

    pub fn on_link_msg(&self, from: PeerId, msg: LinkMsg) {
        let mut st = lock(&self.state);
        if self.is_closed() {
            return;
        }
        match msg {
            LinkMsg::Packet(p) => self.handle_packet(&mut st, from, p),
            LinkMsg::Inventory { held, acked } => self.handle_inventory(&st, from, held, acked),
        }
    }

    pub fn originate(&self, kind: Kind, recipient: PeerId, payload: Vec<u8>) -> Result<MsgId> {
        let fragments = (1 + HEADER_LEN + payload.len()).div_ceil(self.cfg.mtu);
        ensure!(fragments <= MAX_FRAGMENTS, "payload too large: {} bytes", payload.len());
        let mut st = lock(&self.state);
        if self.is_closed() {
            bail!("mesh is shut down");
        }
        let p = Packet {
            kind,
            ttl: self.cfg.default_ttl,
            hops: 1,
            msg_id: MsgId::random(),
            sender: self.me,
            recipient,
            created_ms: now_ms(),
            payload,
        };
        let id = p.msg_id;
        info!(msg_id = %id, to = %recipient, kind = ?kind, bytes = p.payload.len(), "sending");
        self.mark_seen(&mut st, &p);
        self.flood(&st, None, &p);
        self.store(&mut st, p);
        self.persist(&st);
        Ok(id)
    }

    pub fn sweep(&self) {
        let mut st = lock(&self.state);
        if !self.is_closed() && st.db.prune(now_ms()) {
            self.persist(&st);
        }
    }

    fn expiry(&self, p: &Packet) -> u64 {
        p.created_ms.max(now_ms()).saturating_add(self.store_ttl_ms())
    }

    fn mark_seen(&self, st: &mut State, p: &Packet) {
        let exp = self.expiry(p);
        st.db.seen.insert(p.msg_id, exp);
    }

    fn store(&self, st: &mut State, p: Packet) {
        let expires_ms = p.created_ms.saturating_add(self.store_ttl_ms());
        st.db.store.insert(p.msg_id, Stored { packet: p, expires_ms });
    }

    fn flood(&self, st: &State, except: Option<PeerId>, p: &Packet) {
        let body = Arc::new(LinkMsg::Packet(p.clone()).encode());
        for (peer, link) in &st.links {
            if Some(*peer) != except {
                let _ = link.tx.send(Outgoing { frag_id: p.msg_id.0, body: body.clone() });
            }
        }
    }

    fn persist(&self, st: &State) {
        if let Some(path) = &self.cfg.store_path {
            if let Err(e) = persist::save(path, &st.db) {
                warn!(path = %path.display(), error = %e, "persisting mesh store failed");
            }
        }
    }

    fn handle_packet(&self, st: &mut State, from: PeerId, p: Packet) {
        if p.created_ms.saturating_add(self.store_ttl_ms()) < now_ms() {
            debug!(msg_id = %p.msg_id, "expired packet dropped");
            return;
        }
        if st.db.seen.contains_key(&p.msg_id) {
            debug!(msg_id = %p.msg_id, %from, "duplicate dropped");
            return;
        }
        self.mark_seen(st, &p);
        let exp = self.expiry(&p);
        match p.kind {
            Kind::Message if p.recipient == self.me => {
                info!(msg_id = %p.msg_id, from = %p.sender, hops = p.hops, bytes = p.payload.len(), "delivered");
                self.stats.messages_delivered.fetch_add(1, Ordering::Relaxed);
                st.db.acked.insert(p.msg_id, exp);
                let ack = Packet {
                    kind: Kind::Ack,
                    ttl: self.cfg.default_ttl,
                    hops: 1,
                    msg_id: MsgId::random(),
                    sender: self.me,
                    recipient: p.sender,
                    created_ms: now_ms(),
                    payload: p.msg_id.0.to_vec(),
                };
                emit(
                    st,
                    MeshEvent::Message {
                        from: p.sender,
                        msg_id: p.msg_id,
                        payload: p.payload,
                        hops: p.hops,
                        broadcast: false,
                    },
                );
                self.mark_seen(st, &ack);
                self.flood(st, None, &ack);
                self.store(st, ack);
            }
            Kind::Message => {
                if !st.db.acked.contains_key(&p.msg_id) {
                    self.relay(st, from, p);
                }
            }
            Kind::Broadcast => {
                info!(msg_id = %p.msg_id, from = %p.sender, hops = p.hops, "broadcast received");
                self.stats.messages_delivered.fetch_add(1, Ordering::Relaxed);
                emit(
                    st,
                    MeshEvent::Message {
                        from: p.sender,
                        msg_id: p.msg_id,
                        payload: p.payload.clone(),
                        hops: p.hops,
                        broadcast: true,
                    },
                );
                self.relay(st, from, p);
            }
            Kind::Ack => {
                let Ok(acked) = <[u8; 16]>::try_from(p.payload.as_slice()) else {
                    warn!(msg_id = %p.msg_id, "malformed ack dropped");
                    return;
                };
                let acked = MsgId(acked);
                let first = st.db.acked.insert(acked, exp).is_none();
                if st.db.store.remove(&acked).is_some() {
                    debug!(msg_id = %acked, "acked packet removed from store");
                }
                if first && p.recipient == self.me {
                    info!(msg_id = %acked, by = %p.sender, "delivery confirmed");
                    emit(st, MeshEvent::Delivered { msg_id: acked, by: p.sender });
                }
                self.relay(st, from, p);
            }
        }
        self.persist(st);
    }

    /// Stores and forwards the next-hop copy (ttl - 1, hops + 1) if it may travel further.
    fn relay(&self, st: &mut State, from: PeerId, mut p: Packet) {
        if p.ttl <= 1 {
            debug!(msg_id = %p.msg_id, "ttl exhausted, not relaying");
            return;
        }
        p.ttl -= 1;
        p.hops = p.hops.saturating_add(1);
        debug!(msg_id = %p.msg_id, kind = ?p.kind, ttl = p.ttl, %from, "relaying");
        self.stats.packets_relayed.fetch_add(1, Ordering::Relaxed);
        self.flood(st, Some(from), &p);
        self.store(st, p);
    }

    fn handle_inventory(&self, st: &State, from: PeerId, held: Vec<MsgId>, acked: Vec<MsgId>) {
        let Some(link) = st.links.get(&from) else { return };
        let skip: HashSet<MsgId> = held.into_iter().chain(acked).collect();
        let mut sent = 0;
        for (id, s) in &st.db.store {
            if skip.contains(id) {
                continue;
            }
            let body = Arc::new(LinkMsg::Packet(s.packet.clone()).encode());
            let _ = link.tx.send(Outgoing { frag_id: id.0, body });
            sent += 1;
        }
        if sent > 0 {
            self.stats.packets_synced.fetch_add(sent, Ordering::Relaxed);
            info!(peer = %from, packets = sent, "store-and-forward sync");
        }
    }
}

fn emit(st: &State, ev: MeshEvent) {
    if let Some(tx) = &st.events {
        let _ = tx.send(ev);
    }
}
