//! The store-and-forward cache and seen/acked ids, optionally persisted as one JSON file.

use std::{collections::HashMap, fs, io::{Read, Write}, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{packet::{Kind, Packet}, MsgId};

pub(crate) const MAX_PACKETS: usize = 4096;
pub(crate) const MAX_STORE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_SENDER_PACKETS: usize = 512;
pub(crate) const MAX_SENDER_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_IDS: usize = 32768;
pub(crate) const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const CLOCK_SKEW_MS: u64 = 5 * 60 * 1000;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Db {
    /// Packets to hand to neighbours that lack them, keyed by their own msg_id.
    pub store: HashMap<MsgId, Stored>,
    /// Every msg_id processed, with its expiry (unix ms).
    pub seen: HashMap<MsgId, u64>,
    /// Message ids an Ack has been seen for, with expiry (unix ms).
    pub acked: HashMap<MsgId, u64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Stored {
    /// The copy to send onward: ttl and hops already adjusted for the next link.
    pub packet: Packet,
    pub expires_ms: u64,
}

impl Db {
    pub fn can_store(&self, p: &Packet) -> bool {
        self.fits(p, false, false)
    }

    /// Whether `p` fits if `make_room` evicts what it may: the sender's old acks and, for this
    /// device's own packets, also its old broadcasts.
    pub fn can_make_room(&self, p: &Packet, own: bool) -> bool {
        self.fits(p, true, own)
    }

    fn fits(&self, p: &Packet, evict: bool, own: bool) -> bool {
        if self.store.contains_key(&p.msg_id) { return false; }
        let bytes = p.payload.len() + crate::packet::HEADER_LEN;
        let (mut total, mut count) = (bytes, 1);
        let mut sender_bytes = bytes;
        let mut sender_count = 1;
        for s in self.store.values().filter(|s| !(evict && Self::evictable(s, p, own))) {
            let len = s.packet.payload.len() + crate::packet::HEADER_LEN;
            total += len;
            count += 1;
            if s.packet.sender == p.sender { sender_count += 1; sender_bytes += len; }
        }
        bytes < MAX_MESSAGE_BYTES && count <= MAX_PACKETS && total <= MAX_STORE_BYTES
            && sender_count <= MAX_SENDER_PACKETS && sender_bytes <= MAX_SENDER_BYTES
    }

    /// A sender's new packet may displace only that sender's entries, so other senders keep
    /// their quota, and never a held message. Every received sync chunk leaves an ack, and
    /// nothing acks an ack, so a long catch-up would otherwise fill the sender's quota.
    fn evictable(s: &Stored, p: &Packet, own: bool) -> bool {
        s.packet.sender == p.sender && (s.packet.kind == Kind::Ack || (own && s.packet.kind == Kind::Broadcast))
    }

    /// Evicts the sender's oldest evictable entries, acks first, until `p` fits. Returns false,
    /// evicting nothing, if it cannot fit.
    pub fn make_room(&mut self, p: &Packet, own: bool) -> bool {
        if !self.can_make_room(p, own) { return false; }
        while !self.can_store(p) {
            let Some(id) = self.store.iter().filter(|(_, s)| Self::evictable(s, p, own))
                .min_by_key(|(_, s)| (s.packet.kind != Kind::Ack, s.packet.created_ms))
                .map(|(id, _)| *id) else { return false };
            self.store.remove(&id);
        }
        true
    }

    pub fn remember(map: &mut HashMap<MsgId, u64>, id: MsgId, expiry: u64) {
        if map.len() >= MAX_IDS && !map.contains_key(&id) {
            if let Some(old) = map.iter().min_by_key(|(_, e)| **e).map(|(k, _)| *k) { map.remove(&old); }
        }
        map.insert(id, expiry);
    }

    /// Old caches are transport data, not wallet history. Keep only bounded, live entries.
    pub fn constrain(&mut self, now: u64, ttl: u64) {
        self.prune(now);
        let old = std::mem::take(&mut self.store);
        for (id, mut stored) in old {
            if stored.packet.created_ms > now.saturating_add(CLOCK_SKEW_MS)
                || stored.packet.created_ms.saturating_add(ttl) < now { continue; }
            stored.expires_ms = stored.expires_ms.min(now.saturating_add(ttl));
            if self.can_store(&stored.packet) { self.store.insert(id, stored); }
        }
        for map in [&mut self.seen, &mut self.acked] {
            let old = std::mem::take(map);
            for (id, expiry) in old { Self::remember(map, id, expiry.min(now.saturating_add(ttl))); }
        }
    }

    /// Drops expired entries; returns whether anything changed.
    pub fn prune(&mut self, now_ms: u64) -> bool {
        let before = self.store.len() + self.seen.len() + self.acked.len();
        self.store.retain(|_, s| s.expires_ms > now_ms);
        self.seen.retain(|_, e| *e > now_ms);
        self.acked.retain(|_, e| *e > now_ms);
        before != self.store.len() + self.seen.len() + self.acked.len()
    }
}

pub(crate) fn load(path: &Path) -> Result<Db> {
    match fs::File::open(path) {
        Ok(file) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            let mut bytes = Vec::new();
            let limit = (MAX_STORE_BYTES * 2 + 16 * 1024 * 1024) as u64;
            file.take(limit + 1).read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() as u64 <= limit, "mesh cache exceeds disk limit; move it aside to rebuild the transport cache");
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Db::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes a temp file next to `path`, syncs it, then renames it over `path`.
pub(crate) fn save(path: &Path, db: &Db) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let json = serde_json::to_vec(db)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(&json)?;
    f.sync_all()?;
    fs::rename(&tmp, path).with_context(|| format!("renaming into {}", path.display()))
}
