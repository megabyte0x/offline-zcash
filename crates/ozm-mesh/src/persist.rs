//! The store-and-forward cache and seen/acked ids, optionally persisted as one JSON file.

use std::{collections::HashMap, fs, io::Write, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{packet::Packet, MsgId};

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
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display())),
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
    let mut f = fs::File::create(&tmp)?;
    f.write_all(&json)?;
    f.sync_all()?;
    fs::rename(&tmp, path).with_context(|| format!("renaming into {}", path.display()))
}
