//! Mesh logic: links table, dedup, flooding, store-and-forward, acks.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, ensure, Result};
use tokio::{sync::mpsc, task::AbortHandle};
use tracing::{debug, info, warn};

use crate::{
    link::{LinkTx, Outgoing, MAX_FRAGMENTS},
    packet::{Kind, LinkMsg, Packet, HEADER_LEN},
    persist::{self, Db, Stored},
    LinkKind, MeshConfig, MeshEvent, MeshStats, MsgId, PeerId,
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
    kind: LinkKind,
    tx: LinkTx,
    challenge: [u8; 32],
    authenticated: bool,
    pending_inventory: Vec<MsgId>,
    /// A packet from this link was dropped because the app queue was full; the next sweep asks
    /// the neighbour again for what we lack, so a burst (such as a sync batch) still completes.
    resync: bool,
}

struct State {
    links: HashMap<PeerId, LinkEntry>,
    db: Db,
    events: Option<mpsc::Sender<MeshEvent>>,
}

pub(crate) struct Node {
    pub cfg: MeshConfig,
    pub me: PeerId,
    pub stats: Counters,
    pub link_slots: Arc<tokio::sync::Semaphore>,
    outgoing_bytes: Arc<tokio::sync::Semaphore>,
    closed: AtomicBool,
    state: Mutex<State>,
    tasks: Mutex<Vec<AbortHandle>>,
    next_link_id: AtomicU64,
    /// Discovery beacons heard: peer -> (dial address, last heard).
    discovered: Mutex<HashMap<PeerId, (SocketAddr, Instant)>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Node {
    pub fn new(cfg: MeshConfig, mut db: Db, events: mpsc::Sender<MeshEvent>) -> Self {
        db.constrain(now_ms(), cfg.store_ttl.as_millis().min(u64::MAX as u128) as u64);
        Node {
            me: cfg.peer_id,
            cfg,
            stats: Counters::default(),
            link_slots: Arc::new(tokio::sync::Semaphore::new(32)),
            outgoing_bytes: Arc::new(tokio::sync::Semaphore::new(persist::MAX_STORE_BYTES)),
            closed: AtomicBool::new(false),
            state: Mutex::new(State { links: HashMap::new(), db, events: Some(events) }),
            tasks: Mutex::new(Vec::new()),
            next_link_id: AtomicU64::new(1),
            discovered: Mutex::new(HashMap::new()),
        }
    }

    fn store_ttl_ms(&self) -> u64 {
        self.cfg.store_ttl.as_millis().min(u64::MAX as u128) as u64
    }

    pub fn is_closed(&self) -> bool {
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
            self.save_store(&st);
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

    pub fn neighbor_links(&self) -> Vec<(PeerId, LinkKind)> {
        let mut v: Vec<_> = lock(&self.state).links.iter().map(|(p, l)| (*p, l.kind)).collect();
        v.sort_by_key(|(p, _)| *p);
        v
    }

    pub fn is_connected(&self, peer: PeerId) -> bool {
        lock(&self.state).links.contains_key(&peer)
    }

    /// Records a same-fleet discovery beacon.
    pub fn heard(&self, peer: PeerId, addr: SocketAddr) {
        let mut peers = lock(&self.discovered);
        peers.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(60));
        if peers.len() < 256 || peers.contains_key(&peer) { peers.insert(peer, (addr, Instant::now())); }
    }

    /// Peers heard in the last 3 discovery intervals, sorted.
    pub fn discovered(&self) -> Vec<(PeerId, SocketAddr)> {
        let Some(d) = &self.cfg.discovery else { return Vec::new() };
        let window = d.interval.saturating_mul(3).max(Duration::from_millis(1));
        let mut map = lock(&self.discovered);
        map.retain(|_, (_, at)| at.elapsed() < window);
        let mut v: Vec<_> = map.iter().map(|(p, (a, _))| (*p, *a)).collect();
        v.sort();
        v
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
            discovered: self.discovered().len(),
        }
    }

    /// Registers a link after hello. Returns its id, or `None` if it is a duplicate we drop.
    ///
    /// One link per neighbour: if one exists, the new link replaces it when both were dialed by the
    /// same side (a reconnect), otherwise the link dialed by the smaller PeerId wins. Both ends apply
    /// the same rule, so they keep the same TCP connection.
    pub fn link_up(&self, peer: PeerId, outbound: bool, kind: LinkKind, tx: LinkTx) -> Option<u64> {
        let mut st = lock(&self.state);
        if self.is_closed() {
            return None;
        }
        let dialer = |out: bool| if out { self.me } else { peer };
        let id = self.next_link_id.fetch_add(1, Ordering::Relaxed);
        let challenge = rand::random();
        let authenticated = self.cfg.ack_signer.is_none(); // Explicit transport-only test mode.
        if let Some(old) = st.links.get(&peer) {
            let new_dialer = dialer(outbound);
            if new_dialer != dialer(old.outbound) && new_dialer != self.me.min(peer) {
                debug!(%peer, "duplicate link dropped");
                return None;
            }
            info!(%peer, outbound, "link replaced");
            st.links.insert(peer, LinkEntry { id, outbound, kind, tx, challenge, authenticated, pending_inventory: vec![], resync: false });
        } else {
            info!(%peer, outbound, "link up");
            st.links.insert(peer, LinkEntry { id, outbound, kind, tx, challenge, authenticated, pending_inventory: vec![], resync: false });
            emit(&st, MeshEvent::NeighborUp(peer));
        }
        if authenticated { self.inventory(&st, peer); }
        else { self.queue(&st.links[&peer].tx, LinkMsg::Challenge(challenge), rand::random()); }
        Some(id)
    }

    fn queue(&self, tx: &LinkTx, msg: LinkMsg, frag_id: [u8; 16]) {
        if let Some(out) = self.outgoing(msg, frag_id) { let _ = tx.try_send(out); }
    }

    fn outgoing(&self, msg: LinkMsg, frag_id: [u8; 16]) -> Option<Outgoing> {
        let body = msg.encode();
        let budget = self.outgoing_bytes.clone().try_acquire_many_owned(body.len().max(256).try_into().ok()?).ok()?;
        Some(Outgoing { frag_id, body: Arc::new(body), _budget: Arc::new(budget) })
    }

    fn inventory(&self, st: &State, peer: PeerId) {
        let inv = LinkMsg::Inventory {
            held: st.db.seen.keys().copied().collect(),
            acked: st.db.acked.keys().copied().collect(),
        };
        self.queue(&st.links[&peer].tx, inv, rand::random());
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
            LinkMsg::Inventory { held, acked } => self.handle_inventory(&mut st, from, held, acked),
            LinkMsg::Challenge(nonce) => {
                use ed25519_dalek::Signer;
                if let (Some(key), Some(link)) = (&self.cfg.ack_signer, st.links.get(&from)) {
                    let signature = key.sign(&link_proof(from, self.me, nonce)).to_bytes();
                    self.queue(&link.tx, LinkMsg::Proof { key: key.verifying_key().to_bytes(), signature }, rand::random());
                }
            }
            LinkMsg::Proof { key, signature } => {
                let Some(link) = st.links.get_mut(&from) else { return };
                let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&key) else { return };
                if link.authenticated || crate::packet::peer_for(&key) != from
                    || key.verify_strict(&link_proof(self.me, from, link.challenge), &ed25519_dalek::Signature::from_bytes(&signature)).is_err() { return; }
                link.authenticated = true;
                self.inventory(&st, from);
            }
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
        // Our old acks and broadcasts give way; held messages and other senders' packets do not.
        if !st.db.make_room(&p, true) { return Err(crate::StoreFull.into()); }
        info!(msg_id = %id, to = %recipient, kind = ?kind, bytes = p.payload.len(), "sending");
        self.mark_seen(&mut st, &p);
        self.flood(&mut st, None, &p);
        self.store(&mut st, p);
        self.persist(&st);
        Ok(id)
    }

    pub fn sweep(&self) {
        let mut st = lock(&self.state);
        if !self.is_closed() && st.db.prune(now_ms()) {
            self.persist(&st);
        }
        let peers: Vec<_> = st.links.keys().copied().collect();
        for peer in peers {
            if let Some(link) = st.links.get_mut(&peer).filter(|l| l.resync && l.authenticated) {
                link.resync = false;
                self.inventory(&st, peer);
            }
            self.flush_inventory(&mut st, peer);
        }
    }

    fn expiry(&self, p: &Packet) -> u64 {
        p.created_ms.min(now_ms()).saturating_add(self.store_ttl_ms())
    }

    fn mark_seen(&self, st: &mut State, p: &Packet) {
        let exp = self.expiry(p);
        Db::remember(&mut st.db.seen, p.msg_id, exp);
    }

    fn store(&self, st: &mut State, p: Packet) {
        if !st.db.make_room(&p, p.sender == self.me) { return; }
        let expires_ms = p.created_ms.saturating_add(self.store_ttl_ms());
        st.db.store.insert(p.msg_id, Stored { packet: p, expires_ms });
    }

    /// A link whose queue is full gets the packet later from its pending list, as the queue
    /// drains, once the packet is stored. A burst such as a sync batch is then not cut short.
    fn flood(&self, st: &mut State, except: Option<PeerId>, p: &Packet) {
        let out = self.outgoing(LinkMsg::Packet(p.clone()), p.msg_id.0);
        for (peer, link) in &mut st.links {
            // Stored traffic goes only to links that proved their key (as with inventory).
            if Some(*peer) != except && out.as_ref().is_none_or(|out| link.tx.try_send(out.clone()).is_err()) && link.authenticated {
                link.pending_inventory.insert(0, p.msg_id);
            }
        }
    }

    /// Every caller holds the state lock, which `shutdown` takes after setting `closed`, so the
    /// store is never written after shutdown's final save and its folder can be moved away.
    fn persist(&self, st: &State) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        self.save_store(st);
    }

    fn save_store(&self, st: &State) {
        if let Some(path) = &self.cfg.store_path {
            if let Err(e) = persist::save(path, &st.db) {
                warn!(path = %path.display(), error = %e, "persisting mesh store failed");
            }
        }
    }

    fn handle_packet(&self, st: &mut State, from: PeerId, p: Packet) {
        if p.created_ms > now_ms().saturating_add(persist::CLOCK_SKEW_MS)
            || p.payload.len() + HEADER_LEN >= persist::MAX_MESSAGE_BYTES
        { return; }
        // Relay custody must not block local delivery of replies or gateway presence.
        let can_relay = p.kind == Kind::Ack || st.db.can_make_room(&p, false);
        if !can_relay && p.recipient != self.me && p.kind != Kind::Broadcast { return; }
        if (p.recipient == self.me || p.kind == Kind::Broadcast) && st.events.as_ref().is_none_or(|tx| tx.capacity() == 0) {
            if let Some(link) = st.links.get_mut(&from) { link.resync = true; }
            return;
        }
        if p.created_ms.saturating_add(self.store_ttl_ms()) < now_ms() {
            debug!(msg_id = %p.msg_id, "expired packet dropped");
            return;
        }
        if st.db.seen.contains_key(&p.msg_id) {
            debug!(msg_id = %p.msg_id, %from, "duplicate dropped");
            return;
        }
        // A forged ack must not poison dedup before the authentic copy arrives.
        if p.kind == Kind::Ack && self.cfg.ack_signer.is_some() && p.verified_ack().is_err() {
            return;
        }
        self.mark_seen(st, &p);
        let exp = self.expiry(&p);
        match p.kind {
            Kind::Message if p.recipient == self.me => {
                info!(msg_id = %p.msg_id, from = %p.sender, hops = p.hops, bytes = p.payload.len(), "delivered");
                self.stats.messages_delivered.fetch_add(1, Ordering::Relaxed);
                Db::remember(&mut st.db.acked, p.msg_id, exp);
                let mut ack = Packet {
                    kind: Kind::Ack,
                    ttl: self.cfg.default_ttl,
                    hops: 1,
                    msg_id: MsgId::random(),
                    sender: self.me,
                    recipient: p.sender,
                    created_ms: now_ms(),
                    payload: p.msg_id.0.to_vec(),
                };
                if let Some(key) = &self.cfg.ack_signer {
                    ack.payload.extend(blake2b_simd::Params::new().hash_length(32).hash(&p.payload).as_bytes());
                    ack.sign_ack(key);
                }
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
                if can_relay { self.relay(st, from, p); }
            }
            Kind::Ack => {
                let verified = if self.cfg.ack_signer.is_some() {
                    p.verified_ack()
                } else {
                    p.verified_ack().or_else(|_| Ok(MsgId(p.payload.as_slice().try_into()?)))
                };
                let Ok(acked) = verified else {
                    warn!(msg_id = %p.msg_id, "unauthenticated ack dropped");
                    return;
                };
                if let Some(original) = st.db.store.get(&acked) {
                    if original.packet.recipient != p.sender || original.packet.sender != p.recipient {
                        return; // Only the original destination may acknowledge this packet.
                    }
                    if self.cfg.ack_signer.is_some() && p.payload[16..48] != *blake2b_simd::Params::new().hash_length(32).hash(&original.packet.payload).as_bytes() {
                        return; // A spoofed packet with the same ID is not delivery of our payload.
                    }
                } else {
                    // Without the original, a signature cannot establish its destination.
                    return;
                }
                let first = !st.db.acked.contains_key(&acked);
                Db::remember(&mut st.db.acked, acked, exp);
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

    fn handle_inventory(&self, st: &mut State, from: PeerId, held: Vec<MsgId>, acked: Vec<MsgId>) {
        let Some(link) = st.links.get_mut(&from) else { return };
        if !link.authenticated { return; }
        let skip: HashSet<MsgId> = held.into_iter().chain(acked).collect();
        link.pending_inventory = st.db.store.keys().filter(|id| !skip.contains(id)).copied().collect();
        self.flush_inventory(st, from);
    }

    pub fn link_flushed(&self, peer: PeerId) {
        self.flush_inventory(&mut lock(&self.state), peer);
    }

    fn flush_inventory(&self, st: &mut State, from: PeerId) {
        let Some(link) = st.links.get_mut(&from) else { return };
        let mut sent = 0;
        while let Some(id) = link.pending_inventory.last().copied() {
            let Some(s) = st.db.store.get(&id) else {
                link.pending_inventory.pop();
                continue;
            };
            let Some(out) = self.outgoing(LinkMsg::Packet(s.packet.clone()), id.0) else { break };
            if link.tx.try_send(out).is_err() { break; }
            link.pending_inventory.pop();
            sent += 1;
        }
        if sent > 0 {
            self.stats.packets_synced.fetch_add(sent, Ordering::Relaxed);
            info!(peer = %from, packets = sent, "store-and-forward sync");
        }
    }
}

fn link_proof(challenger: PeerId, signer: PeerId, nonce: [u8; 32]) -> Vec<u8> {
    [b"ozm1-link-proof-v1".as_slice(), &challenger.0, &signer.0, &nonce].concat()
}

fn emit(st: &State, ev: MeshEvent) {
    if let Some(tx) = &st.events {
        let _ = tx.try_send(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::peer_for;
    use ed25519_dalek::SigningKey;

    #[test]
    fn forged_acks_cannot_confirm_or_delete_a_queued_message() {
        let a = SigningKey::from_bytes(&[1; 32]);
        let b = SigningKey::from_bytes(&[2; 32]);
        let thief = SigningKey::from_bytes(&[3; 32]);
        let (events, mut rx) = mpsc::channel(64);
        let node = Node::new(MeshConfig { peer_id: peer_for(&a.verifying_key()), ack_signer: Some(a), ..Default::default() }, Db::default(), events);
        let id = node.originate(Kind::Message, peer_for(&b.verifying_key()), b"sealed".to_vec()).unwrap();
        let mut ack = Packet { kind: Kind::Ack, ttl: 7, hops: 1, msg_id: MsgId::random(),
            sender: peer_for(&b.verifying_key()), recipient: node.me, created_ms: now_ms(), payload: id.0.to_vec() };
        node.on_link_msg(ack.sender, LinkMsg::Packet(ack.clone())); // Old, public-key-only forgery.
        assert!(rx.try_recv().is_err());
        let body = [id.0.as_slice(), blake2b_simd::Params::new().hash_length(32).hash(b"sealed").as_bytes()].concat();
        ack.payload = body.clone();
        ack.sign_ack(&thief);
        node.on_link_msg(ack.sender, LinkMsg::Packet(ack.clone())); // Forged destination signature.
        assert!(rx.try_recv().is_err());
        let mut other = ack.clone();
        other.msg_id = MsgId::random(); other.sender = peer_for(&thief.verifying_key());
        other.payload = body.clone(); other.sign_ack(&thief);
        node.on_link_msg(other.sender, LinkMsg::Packet(other)); // Valid signature, wrong recipient.
        assert!(rx.try_recv().is_err());
        let mut wrong_payload = ack.clone(); wrong_payload.msg_id = MsgId::random();
        wrong_payload.payload = body.clone(); wrong_payload.payload[16] ^= 1; wrong_payload.sign_ack(&b);
        node.on_link_msg(wrong_payload.sender, LinkMsg::Packet(wrong_payload));
        assert!(rx.try_recv().is_err());
        // A relay must not advertise an unknown ID as acknowledged just because someone signed it.
        let relay_key = SigningKey::from_bytes(&[4; 32]);
        let (events, _) = mpsc::channel(64);
        let relay = Node::new(MeshConfig { peer_id: peer_for(&relay_key.verifying_key()), ack_signer: Some(relay_key), ..Default::default() }, Db::default(), events);
        let mut premature = ack.clone();
        premature.sender = peer_for(&thief.verifying_key());
        premature.payload = body.clone(); premature.sign_ack(&thief);
        relay.on_link_msg(premature.sender, LinkMsg::Packet(premature));
        assert!(!lock(&relay.state).db.acked.contains_key(&id));
        // The real destination still confirms the same queued message, through a relay.
        ack.payload = body; ack.sign_ack(&b); ack.ttl = 3; ack.hops = 5;
        node.on_link_msg(peer_for(&thief.verifying_key()), LinkMsg::Packet(ack));
        assert_eq!(rx.try_recv().unwrap(), MeshEvent::Delivered { msg_id: id, by: peer_for(&b.verifying_key()) });
    }

    #[test]
    fn own_acks_for_a_long_sync_never_block_new_sends() {
        let (p, g) = (SigningKey::from_bytes(&[5; 32]), SigningKey::from_bytes(&[6; 32]));
        let (pid, gid) = (peer_for(&p.verifying_key()), peer_for(&g.verifying_key()));
        let (events, mut rx) = mpsc::channel(64);
        let node = Node::new(MeshConfig { peer_id: pid, ack_signer: Some(p), ..Default::default() }, Db::default(), events);
        // A catch-up over the mesh: every received chunk leaves one of our own acks in the store.
        for i in 0..persist::MAX_SENDER_PACKETS + 10 {
            let chunk = Packet { kind: Kind::Message, ttl: 7, hops: 1, msg_id: MsgId::random(), sender: gid,
                recipient: pid, created_ms: now_ms(), payload: (i as u32).to_le_bytes().to_vec() };
            node.on_link_msg(gid, LinkMsg::Packet(chunk));
            while rx.try_recv().is_ok() {}
        }
        assert_eq!(node.stats().stored, persist::MAX_SENDER_PACKETS);
        // The next sync request (or a payment) is still stored for store-and-forward.
        let id = node.originate(Kind::Message, gid, b"sync request".to_vec()).unwrap();
        assert!(lock(&node.state).db.store.contains_key(&id));
        // A gateway or relay holding those acks also accepts the sender's next request.
        let st = lock(&node.state);
        let mut next = st.db.store[&id].packet.clone();
        next.msg_id = MsgId::random();
        assert!(!st.db.can_store(&next) && st.db.can_make_room(&next, false));
    }

    #[test]
    fn future_packets_cannot_pin_the_cache_or_poison_dedup() {
        let (tx, _rx) = mpsc::channel(64);
        let node = Node::new(MeshConfig::default(), Db::default(), tx);
        let mut packet = Packet { kind: Kind::Message, ttl: 7, hops: 1, msg_id: MsgId::random(), sender: PeerId([8; 8]),
            recipient: PeerId([9; 8]), created_ms: now_ms() + 301_000, payload: vec![1, 3, 7] };
        node.on_link_msg(packet.sender, LinkMsg::Packet(packet.clone()));
        assert_eq!(node.stats().stored, 0);
        packet.created_ms = now_ms() + 299_000;
        node.on_link_msg(packet.sender, LinkMsg::Packet(packet));
        assert_eq!(node.stats().stored, 1);
    }

    #[test]
    fn full_relay_custody_still_delivers_local_messages_and_presence() {
        let (tx, mut rx) = mpsc::channel(64);
        let node = Node::new(MeshConfig::default(), Db::default(), tx);
        let sender = PeerId([2; 8]);
        let held = Packet { kind: Kind::Message, ttl: 7, hops: 1, msg_id: MsgId::random(), sender,
            recipient: PeerId([3; 8]), created_ms: now_ms(), payload: b"held for offline peer".to_vec() };
        for _ in 0..persist::MAX_SENDER_PACKETS {
            node.on_link_msg(sender, LinkMsg::Packet(Packet { msg_id: MsgId::random(), ..held.clone() }));
        }
        assert_eq!(node.stats().stored, persist::MAX_SENDER_PACKETS);
        for kind in [Kind::Message, Kind::Broadcast] {
            let packet = Packet { msg_id: MsgId::random(), kind, recipient: node.me, payload: b"local reply".to_vec(), ..held.clone() };
            node.on_link_msg(sender, LinkMsg::Packet(packet.clone()));
            assert!(matches!(rx.try_recv().unwrap(), MeshEvent::Message { msg_id, payload, .. }
                if msg_id == packet.msg_id && payload == b"local reply"));
        }
        // No held packet was evicted and the sender still cannot add relay custody.
        node.on_link_msg(sender, LinkMsg::Packet(Packet { msg_id: MsgId::random(), ..held }));
        let state = lock(&node.state);
        assert_eq!(state.db.store.values().filter(|s| s.packet.sender == sender).count(), persist::MAX_SENDER_PACKETS);
        assert!(rx.try_recv().is_err());
    }
}
