//! Online wallet sync and the offline note inventory.
//!
//! The device scans the chain over Zakura RPC while it is online, keeping every
//! Ironwood note commitment (to rebuild witnesses offline), the tree root after
//! each block (the payee's known anchors), and its own notes. After that it can
//! build (payer) or check (payee) payments with no connection.

use anyhow::{anyhow, ensure, Context, Result};
use incrementalmerkletree::{Position, Retention};
use orchard::{
    keys::{FullViewingKey, PreparedIncomingViewingKey, Scope},
    note::{RandomSeed, Rho},
    tree::{MerkleHashOrchard, MerklePath},
    value::NoteValue,
    Note, NoteVersion,
};
use serde::{Deserialize, Serialize};
use shardtree::{store::memory::MemoryShardStore, ShardTree};
use std::{collections::BTreeMap, path::Path};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BranchId;

use crate::{rpc::Rpc, sync::{Batch, CompactBlock, Seed}, wallet_tree::{TreeData, CHECKPOINTS}};

pub type Tree = ShardTree<MemoryShardStore<MerkleHashOrchard, u32>, 32, 16>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StoredNote {
    pub position: u64,
    pub height: u32,
    pub txid: String,
    pub value: u64,
    pub recipient: String,
    pub rho: String,
    pub rseed: String,
    pub nullifier: String,
    /// Txid of the mined transaction that spent this note.
    pub spent_in: Option<String>,
    /// Txid of an offline payment built from this note that is not mined yet.
    /// A locked note is never used again (no double payment from one device).
    pub locked_by: Option<String>,
}

impl StoredNote {
    pub fn from_note(note: &Note, fvk: &FullViewingKey, txid: String, height: u32, position: u64) -> Self {
        Self { position, height, txid, value: note.value().inner(),
            recipient: hex::encode(note.recipient().to_raw_address_bytes()), rho: hex::encode(note.rho().to_bytes()),
            rseed: hex::encode(note.rseed().as_bytes()), nullifier: hex::encode(note.nullifier(fvk).to_bytes()),
            spent_in: None, locked_by: None }
    }

    pub fn commitment(&self) -> Result<[u8; 32]> {
        Ok(orchard::note::ExtractedNoteCommitment::from(self.note()?.commitment()).to_bytes())
    }

    pub fn note(&self) -> Result<Note> {
        let recipient: [u8; 43] = hex::decode(&self.recipient)?.try_into().map_err(|_| anyhow!("recipient"))?;
        let recipient = Option::from(orchard::Address::from_raw_address_bytes(&recipient)).ok_or_else(|| anyhow!("recipient"))?;
        let rho: [u8; 32] = hex::decode(&self.rho)?.try_into().map_err(|_| anyhow!("rho"))?;
        let rho = Option::from(Rho::from_bytes(&rho)).ok_or_else(|| anyhow!("rho"))?;
        let rseed: [u8; 32] = hex::decode(&self.rseed)?.try_into().map_err(|_| anyhow!("rseed"))?;
        let rseed = Option::from(RandomSeed::from_bytes(rseed, &rho)).ok_or_else(|| anyhow!("rseed"))?;
        Option::from(Note::from_parts(recipient, NoteValue::from_raw(self.value), rho, rseed, NoteVersion::V3))
            .ok_or_else(|| anyhow!("note does not reconstruct"))
    }
    pub fn spendable(&self) -> bool {
        self.spent_in.is_none() && self.locked_by.is_none()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WalletState {
    pub tip_height: u32,
    pub tip_hash: String,
    /// Legacy v0.6 inventory, converted once to a pruned ShardTree on next sync.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commitments: Vec<String>,
    /// Ironwood tree root after each scanned block (hex), by height.
    pub roots: BTreeMap<u32, String>,
    pub notes: Vec<StoredNote>,
    /// Mesh-decrypted outputs, without a chain position or witness. Never selected for spending.
    #[serde(default)]
    pub pending_incoming: Vec<StoredNote>,
    /// Every Ironwood nullifier revealed on chain (hex), for conflict checks.
    #[serde(default)]
    pub chain_nullifiers: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub tree: Option<TreeData>,
    #[serde(default)]
    pub hashes: BTreeMap<u32, String>,
    #[serde(default)]
    pub nullifiers_by_height: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    pub spent_heights: BTreeMap<String, u32>,
    /// Reservations survive rollback, including notes temporarily orphaned off chain.
    #[serde(default)]
    pub reservations: BTreeMap<String, (String, u32)>,
    #[serde(default)]
    pub birthday: u32,
    #[serde(default)]
    pub created_at: u64,
}

impl WalletState {
    /// Testnet block 4,550,000, observed from GetBlock and GetTreeState on 28 October 2026.
    pub const TESTNET_CHECKPOINT: (u32, u64) = (4_550_000, 1_793_168_502);

    /// Resolve an offline-created wallet only once. A day of overlap protects addresses shared
    /// before the first gateway contact; the contact tip alone would silently miss those receipts.
    pub fn resolve_birthday(&mut self, network: crate::fleet::Network, floor: u32, tip: u32, time: u64) {
        if self.birthday != 0 { return; }
        let height = match network {
            crate::fleet::Network::Regtest => tip.saturating_sub(100).max(1),
            crate::fleet::Network::Testnet => {
                let (checkpoint, checkpoint_time) = Self::TESTNET_CHECKPOINT;
                let elapsed = self.created_at.saturating_sub(86_400).saturating_sub(checkpoint_time);
                let span = time.saturating_sub(checkpoint_time).max(1);
                let estimate = checkpoint as u128 + elapsed as u128 * tip.saturating_sub(checkpoint) as u128 / span as u128;
                estimate.min(tip as u128).max(checkpoint.min(tip) as u128) as u32
            }
        };
        self.birthday = height.max(floor.min(tip)).min(tip).max(1);
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Atomic, fsynced write (temp file + rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_vec(self)?)
    }

    pub fn balance(&self) -> (u64, u64) {
        let spendable = self.notes.iter().filter(|n| n.spendable()).map(|n| n.value).sum();
        let locked = self.notes.iter().filter(|n| n.spent_in.is_none() && n.locked_by.is_some()).map(|n| n.value).sum();
        (spendable, locked)
    }

    pub fn record_incoming(&mut self, notes: Vec<StoredNote>) -> Result<()> {
        for note in notes {
            let commitment = note.commitment()?;
            if !self.notes.iter().chain(&self.pending_incoming).any(|n| n.commitment().is_ok_and(|c| c == commitment)) {
                self.pending_incoming.push(note);
            }
        }
        Ok(())
    }

    /// Height at which this anchor (tree root) was the tip root, if known.
    pub fn anchor_height(&self, anchor: &[u8; 32]) -> Option<u32> {
        let a = hex::encode(anchor);
        self.roots.iter().rev().find(|(_, r)| **r == a).map(|(h, _)| *h)
    }

    fn rebuild_tree(&self, marked: &[u64]) -> Result<Tree> {
        if let Some(data) = &self.tree { return data.load(); }
        let mut tree = Tree::new(MemoryShardStore::empty(), CHECKPOINTS);
        for (i, c) in self.commitments.iter().enumerate() {
            let b: [u8; 32] = hex::decode(c)?.try_into().map_err(|_| anyhow!("cmx"))?;
            let leaf = Option::from(MerkleHashOrchard::from_bytes(&b)).ok_or_else(|| anyhow!("cmx"))?;
            let r = if marked.contains(&(i as u64)) || self.notes.iter().any(|n| n.position == i as u64) { Retention::Marked } else { Retention::Ephemeral };
            tree.append(leaf, r).map_err(|e| anyhow!("tree append: {e:?}"))?;
        }
        tree.checkpoint(self.tip_height).map_err(|e| anyhow!("checkpoint: {e:?}"))?;
        Ok(tree)
    }

    /// Witness for a note against the root at the last synced tip.
    pub fn witness(&self, position: u64) -> Result<(MerklePath, [u8; 32])> {
        let tree = self.rebuild_tree(&[position])?;
        let path = tree
            .witness_at_checkpoint_depth(Position::from(position), 0)
            .map_err(|e| anyhow!("witness: {e:?}"))?
            .ok_or_else(|| anyhow!("no witness for position {position}"))?;
        let root = tree
            .root_at_checkpoint_depth(Some(0))
            .map_err(|e| anyhow!("root: {e:?}"))?
            .ok_or_else(|| anyhow!("no root"))?;
        Ok((path.into(), root.to_bytes()))
    }

    /// Forgets the chain view for a rescan, keeping notes locked by offline
    /// payments so a reorg never frees a note we already paid with.
    pub fn reset_keep_locks(&mut self) {
        let reservations = std::mem::take(&mut self.reservations);
        let pending_incoming = std::mem::take(&mut self.pending_incoming);
        let (birthday, created_at) = (self.birthday, self.created_at);
        *self = WalletState::default();
        self.reservations = reservations;
        self.pending_incoming = pending_incoming;
        self.birthday = birthday;
        self.created_at = created_at;
    }

    /// Scans new blocks from the node. Rescans from scratch after a reorg.
    pub fn sync(&mut self, rpc: &Rpc, fvk: &FullViewingKey) -> Result<SyncReport> {
        if self.tip_height == 0 && self.birthday > 1 { self.seed(&rpc.seed(self.birthday - 1)?)?; }
        self.sync_from(rpc, fvk)
    }

    /// Scans new blocks from any source: the node's RPC, or blocks a gateway sent over the mesh.
    pub fn sync_from<S: BlockSource>(&mut self, rpc: &S, fvk: &FullViewingKey) -> Result<SyncReport> {
        let chain_tip = rpc.tip()?;
        let mut view = self.clone();
        if self.tip_height > 0 && (self.tip_height > chain_tip || rpc.block_hash(self.tip_height)? != self.tip_hash) {
            let ancestor = self.hashes.iter().rev().find(|(h, hash)| **h <= chain_tip && rpc.block_hash(**h).is_ok_and(|v| v == **hash));
            if let Some((height, _)) = ancestor {
                view.rollback(*height)?;
            } else {
                view.reset_keep_locks();
            }
        }
        let mut batch = Batch { prev_height: view.tip_height, prev_hash: view.tip_hash.clone(), seed: None,
            blocks: vec![], chain_tip, sources: vec!["Regtest JSON-RPC".into()] };
        for h in view.tip_height + 1..=chain_tip {
            let (hash, txs) = rpc.block_txs(h).with_context(|| format!("block {h}"))?;
            batch.push(CompactBlock::from_raw(h, hash, &txs)?);
        }
        let report = view.apply_batch(&batch, fvk)?;
        *self = view;
        Ok(report)
    }

    pub fn seed(&mut self, seed: &Seed) -> Result<()> {
        ensure!(self.tip_height == 0, "cannot reseed an existing wallet");
        let bytes = hex::decode(&seed.frontier)?;
        let frontier = zcash_primitives::merkle_tree::read_frontier_v0::<MerkleHashOrchard, _>(&bytes[..])?;
        let mut tree = Tree::new(MemoryShardStore::empty(), CHECKPOINTS);
        tree.insert_frontier(frontier, Retention::Ephemeral).map_err(|e| anyhow!("seed frontier: {e:?}"))?;
        tree.checkpoint(seed.height).map_err(|e| anyhow!("seed checkpoint: {e:?}"))?;
        let root = tree.root_at_checkpoint_id(&seed.height).map_err(|e| anyhow!("seed root: {e:?}"))?
            .ok_or_else(|| anyhow!("missing seed root"))?;
        self.roots.insert(seed.height, hex::encode(root.to_bytes()));
        self.tree = Some(TreeData::capture(&tree)?);
        if self.birthday == 0 { self.birthday = seed.height + 1; }
        self.tip_height = seed.height;
        self.tip_hash = seed.hash.clone();
        self.hashes.insert(seed.height, seed.hash.clone());
        Ok(())
    }

    pub fn rollback(&mut self, height: u32) -> Result<()> {
        let mut tree = self.rebuild_tree(&[])?;
        ensure!(tree.truncate_to_checkpoint(&height).map_err(|e| anyhow!("rollback: {e:?}"))?,
            "reorg exceeds retained checkpoints; resync from the original birthday");
        self.tip_hash = self.hashes.get(&height).cloned().ok_or_else(|| anyhow!("missing checkpoint hash at {height}"))?;
        self.tip_height = height;
        self.notes.retain(|n| n.height <= height);
        for note in &mut self.notes {
            if self.spent_heights.get(&note.nullifier).is_some_and(|h| *h > height) {
                note.spent_in = None;
                note.locked_by = self.reservations.get(&note.nullifier).map(|r| r.0.clone());
            }
        }
        self.spent_heights.retain(|_, h| *h <= height);
        self.nullifiers_by_height.retain(|h, _| *h <= height);
        self.chain_nullifiers = self.nullifiers_by_height.values().flatten().cloned().collect();
        self.roots.retain(|h, _| *h <= height);
        self.hashes.retain(|h, _| *h <= height);
        self.tree = Some(TreeData::capture(&tree)?);
        self.commitments.clear();
        Ok(())
    }

    pub fn lock_notes(&mut self, positions: &[u64], txid: &str, expiry: u32) {
        for note in self.notes.iter_mut().filter(|n| positions.contains(&n.position)) {
            note.locked_by = Some(txid.into());
            self.reservations.insert(note.nullifier.clone(), (txid.into(), expiry));
        }
    }

    /// Expiry is inclusive. Only a scanned tip beyond it releases an unspent note.
    pub fn release_expired(&mut self) {
        // Keep expired reservations through the rewind window so a reorg can relock them.
        let oldest = self.tip_height.saturating_sub(CHECKPOINTS as u32 - 1);
        self.reservations.retain(|_, (_, expiry)| *expiry == 0 || oldest <= *expiry);
        for n in &mut self.notes {
            n.locked_by = self.reservations.get(&n.nullifier)
                .filter(|(_, expiry)| n.spent_in.is_none() && (*expiry == 0 || self.tip_height <= *expiry))
                .map(|(txid, _)| txid.clone());
        }
    }

    /// An invalid or interrupted batch cannot publish a cursor ahead of its tree or notes.
    pub fn apply_batch(&mut self, batch: &Batch, fvk: &FullViewingKey) -> Result<SyncReport> {
        let mut next = self.clone();
        let report = next.apply_batch_inner(batch, fvk)?;
        *self = next;
        Ok(report)
    }

    fn apply_batch_inner(&mut self, batch: &Batch, fvk: &FullViewingKey) -> Result<SyncReport> {
        let rescanned = batch.prev_height < self.tip_height;
        if let Some(seed) = &batch.seed { self.seed(seed)?; }
        if batch.prev_height < self.tip_height { self.rollback(batch.prev_height)?; }
        ensure!(self.tip_height == batch.prev_height && self.tip_hash == batch.prev_hash, "sync predecessor mismatch");
        let ivk = PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::External));
        let mut tree = self.rebuild_tree(&[])?;
        let mut new_notes = 0;
        let mut position = tree.max_leaf_position(None).map_err(|e| anyhow!("position: {e:?}"))?.map_or(0, |p| u64::from(p) + 1);
        // Only the last CHECKPOINTS heights survive this batch's pruning below. Checkpointing and
        // computing a root for every earlier block cost milliseconds each and were discarded.
        let kept = batch.blocks.last().map_or(0, |b| b.height).saturating_sub(CHECKPOINTS as u32 - 1);
        // Trial decryption is ~0.1 ms per action and independent; testnet ranges hold ~10 actions per
        // block. Decrypt on all cores first; the tree and notes below still apply in chain order.
        let decrypted = {
            use rayon::prelude::*;
            let actions: Vec<_> = batch.blocks.iter().flat_map(|b| &b.txs).flat_map(|t| &t.actions).collect();
            actions.par_iter().map(|a| {
                let action = a.compact()?;
                let domain = orchard::note_encryption::IronwoodDomain::for_compact_action(&action);
                Ok(zcash_note_encryption::try_compact_note_decryption(&domain, &ivk, &action).map(|(note, _)| note))
            }).collect::<Result<Vec<_>>>()?
        };
        let mut decrypted = decrypted.into_iter();
        // Leaves wait here until a checkpoint needs them: one bulk insert instead of a per-leaf append,
        // which costs ~0.1 ms each on busy testnet ranges.
        let mut pending: Vec<(MerkleHashOrchard, Retention<u32>)> = Vec::new();
        let mut pending_from = position;
        for block in &batch.blocks {
            ensure!(block.height > self.tip_height && block.height <= batch.chain_tip, "non-monotonic sync height");
            ensure!(block.txs.is_empty() || block.height == self.tip_height + 1, "nonempty block skips heights");
            for tx in &block.txs {
                let mut id = tx.txid;
                id.reverse();
                let txid = hex::encode(id);
                for a in &tx.actions {
                    let nf = hex::encode(a.nf);
                    self.nullifiers_by_height.entry(block.height).or_default().push(nf.clone());
                    for n in self.notes.iter_mut().filter(|n| n.nullifier == nf) {
                        n.spent_in = Some(txid.clone());
                        self.spent_heights.insert(nf.clone(), block.height);
                    }
                    let note = decrypted.next().expect("one decryption per action");
                    let marked = note.is_some();
                    if let Some(note) = note {
                        self.pending_incoming.retain(|n| !n.commitment().is_ok_and(|c| c == a.cmx));
                        let nullifier = hex::encode(note.nullifier(fvk).to_bytes());
                        if !self.notes.iter().any(|n| n.nullifier == nullifier) {
                            let mut stored = StoredNote::from_note(&note, fvk, txid.clone(), block.height, position);
                            stored.locked_by = self.reservations.get(&nullifier).map(|r| r.0.clone());
                            self.notes.push(stored);
                            new_notes += 1;
                        } else if let Some(n) = self.notes.iter_mut().find(|n| n.nullifier == nullifier) {
                            n.position = position;
                            n.height = block.height;
                        }
                    }
                    let leaf = Option::from(MerkleHashOrchard::from_bytes(&a.cmx)).ok_or_else(|| anyhow!("cmx"))?;
                    pending.push((leaf, if marked { Retention::Marked } else { Retention::Ephemeral }));
                    position += 1;
                }
            }
            if block.height >= kept && !pending.is_empty() {
                tree.batch_insert(Position::from(pending_from), pending.drain(..)).map_err(|e| anyhow!("tree insert: {e:?}"))?;
                pending_from = position;
            }
            // Empty runs advance checkpoints without transporting per-block ciphertexts or hashes.
            for h in (self.tip_height + 1).max(kept)..=block.height {
                tree.checkpoint(h).map_err(|e| anyhow!("checkpoint: {e:?}"))?;
            }
            if block.height >= kept {
                let root = tree
                    .root_at_checkpoint_id(&block.height)
                    .map_err(|e| anyhow!("root: {e:?}"))?
                    .ok_or_else(|| anyhow!("no root"))?;
                self.roots.insert(block.height, hex::encode(root.to_bytes()));
                self.hashes.insert(block.height, block.hash.clone());
            }
            self.tip_height = block.height;
            self.tip_hash = block.hash.clone();
        }
        let floor = self.tip_height.saturating_sub(CHECKPOINTS as u32 - 1);
        self.roots.retain(|h, _| *h >= floor);
        self.hashes.retain(|h, _| *h >= floor);
        self.nullifiers_by_height.retain(|h, _| *h >= floor);
        self.chain_nullifiers = self.nullifiers_by_height.values().flatten().cloned().collect();
        self.tree = Some(TreeData::capture(&tree)?);
        self.commitments.clear();
        self.release_expired();
        Ok(SyncReport { tip: self.tip_height, new_notes, rescanned })
    }
}

/// True if the raw NU7 transaction carries an Ironwood bundle (the only transactions wallet sync needs).
pub fn has_ironwood(raw: &[u8]) -> bool {
    Transaction::read(raw, BranchId::Nu7).map(|tx| tx.ironwood_bundle().is_some()).unwrap_or(false)
}

pub fn check_frontier_size(encoded: &str, expected: u64) -> Result<()> {
    let bytes = hex::decode(encoded)?;
    let frontier = zcash_primitives::merkle_tree::read_frontier_v0::<MerkleHashOrchard, _>(&bytes[..])?;
    ensure!(frontier.tree_size() == expected, "Ironwood frontier size {} differs from block metadata {expected}", frontier.tree_size());
    Ok(())
}

/// Where wallet sync reads blocks from.
pub trait BlockSource {
    fn tip(&self) -> Result<u32>;
    fn block_hash(&self, h: u32) -> Result<String>;
    /// Block hash and raw transactions. Sources may omit transactions without an Ironwood bundle.
    fn block_txs(&self, h: u32) -> Result<(String, Vec<Vec<u8>>)>;
}

impl BlockSource for Rpc {
    fn tip(&self) -> Result<u32> {
        Rpc::tip(self)
    }
    fn block_hash(&self, h: u32) -> Result<String> {
        Rpc::block_hash(self, h)
    }
    fn block_txs(&self, h: u32) -> Result<(String, Vec<Vec<u8>>)> {
        Rpc::block_txs(self, h)
    }
}

/// A run of consecutive blocks a gateway sent over the mesh (gateway-reported, like its results).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlockData {
    pub height: u32,
    pub hash: String,
    /// Only transactions with an Ironwood bundle.
    pub txs: Vec<Vec<u8>>,
}

pub struct ChunkSource {
    /// Gateway's hash at `first - 1`, so the wallet can detect a reorg below the chunk.
    pub prev: (u32, String),
    pub blocks: Vec<BlockData>,
}

impl BlockSource for ChunkSource {
    fn tip(&self) -> Result<u32> {
        Ok(self.blocks.last().map_or(self.prev.0, |b| b.height))
    }
    fn block_hash(&self, h: u32) -> Result<String> {
        if h == self.prev.0 {
            return Ok(self.prev.1.clone());
        }
        self.blocks.iter().find(|b| b.height == h).map(|b| b.hash.clone()).ok_or_else(|| anyhow!("block {h} not in chunk"))
    }
    fn block_txs(&self, h: u32) -> Result<(String, Vec<Vec<u8>>)> {
        self.blocks.iter().find(|b| b.height == h).map(|b| (b.hash.clone(), b.txs.clone())).ok_or_else(|| anyhow!("block {h} not in chunk"))
    }
}

#[derive(Debug, Serialize)]
pub struct SyncReport {
    pub tip: u32,
    pub new_notes: usize,
    pub rescanned: bool,
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    {
        let mut options = std::fs::OpenOptions::new();
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
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent() {
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::{Action, CompactTx};

    #[test]
    fn offline_creation_uses_the_creation_day_not_the_first_contact_tip() {
        use crate::fleet::Network;
        let (height, time) = WalletState::TESTNET_CHECKPOINT;
        let mut wallet = WalletState { created_at: time + 3 * 86_400, ..Default::default() };
        // Contact a week after the checkpoint. Creation minus a day is only two days in.
        wallet.resolve_birthday(Network::Testnet, 4_465_026, height + 24_192, time + 7 * 86_400);
        assert_eq!(wallet.birthday, height + 6_912);
        wallet.reset_keep_locks();
        wallet.resolve_birthday(Network::Testnet, height + 9_000, height + 30_000, time + 9 * 86_400);
        assert_eq!((wallet.birthday, wallet.created_at), (height + 6_912, time + 3 * 86_400));
        for (created_at, floor, tip, expected) in [
            (time - 86_400, 0, height + 900, height),
            (u64::MAX, 0, height + 900, height + 900),
            (time, height + 400, height + 900, height + 400),
        ] {
            let mut w = WalletState { created_at, ..Default::default() };
            w.resolve_birthday(Network::Testnet, floor, tip, time + 86_400);
            assert_eq!(w.birthday, expected);
        }
        let mut w = WalletState::default();
        w.resolve_birthday(Network::Regtest, 0, 53, 0);
        assert_eq!(w.birthday, 1);
        w.birthday = 0;
        w.resolve_birthday(Network::Regtest, 0, 804, 0);
        assert_eq!(w.birthday, 704);
        assert!(check_frontier_size("000000", 0).is_ok());
        assert!(check_frontier_size("000000", 1).is_err());
    }

    #[test]
    fn persisted_reorg_restores_spends_and_expiry_unlocks_only_beyond_the_boundary() {
        let fvk = crate::DeviceSecret::generate("A", "payer").fvk().unwrap();
        let block = |height, hash: &str, n: u8| CompactBlock { height, hash: hash.into(), txs: vec![CompactTx {
            txid: [n; 32], actions: vec![Action { nf: [n; 32], cmx: [n; 32], epk: [0; 32], ct: vec![0; 52] }],
        }] };
        let batch = |prev_height, prev_hash: &str, blocks: Vec<CompactBlock>, tip| Batch {
            prev_height, prev_hash: prev_hash.into(), seed: None, blocks, chain_tip: tip, sources: vec![],
        };
        let mut wallet = WalletState::default();
        wallet.apply_batch(&batch(0, "", vec![block(1, "a1", 1)], 1), &fvk).unwrap();
        wallet.notes.push(StoredNote { position: 0, height: 1, txid: "owned".into(), value: 70_001,
            recipient: String::new(), rho: String::new(), rseed: String::new(), nullifier: hex::encode([2; 32]), spent_in: None, locked_by: None });
        wallet.lock_notes(&[0], "pending", 3);
        let original = wallet.clone();
        wallet.apply_batch(&batch(1, "a1", vec![block(2, "a2", 2)], 2), &fvk).unwrap();
        assert_eq!(wallet.balance(), (0, 0));
        // Restart from the durable representation, then replace the spending block.
        let mut wallet: WalletState = serde_json::from_slice(&serde_json::to_vec(&wallet).unwrap()).unwrap();
        wallet.apply_batch(&batch(1, "a1", vec![block(2, "b2", 3)], 2), &fvk).unwrap();
        assert_eq!(wallet.balance(), (0, 70_001));
        assert_eq!(wallet.notes[0].spent_in, None);
        let mut one_pass = original;
        one_pass.apply_batch(&batch(1, "a1", vec![block(2, "b2", 3)], 2), &fvk).unwrap();
        assert_eq!(wallet.roots, one_pass.roots);
        wallet.apply_batch(&batch(2, "b2", vec![block(3, "b3", 4)], 3), &fvk).unwrap();
        assert_eq!(wallet.balance(), (0, 70_001));
        let before = serde_json::to_value(&wallet).unwrap();
        assert!(wallet.apply_batch(&batch(3, "wrong fork", vec![block(4, "b4", 5)], 4), &fvk).is_err());
        assert_eq!(serde_json::to_value(&wallet).unwrap(), before);
        wallet.apply_batch(&batch(3, "b3", vec![block(4, "b4", 5)], 4), &fvk).unwrap();
        assert_eq!(wallet.balance(), (70_001, 0));
        // Expiry is not finality: rewinding below the boundary restores the reservation.
        let mut wallet: WalletState = serde_json::from_slice(&serde_json::to_vec(&wallet).unwrap()).unwrap();
        wallet.apply_batch(&batch(2, "b2", vec![block(3, "c3", 6)], 3), &fvk).unwrap();
        assert_eq!(wallet.balance(), (0, 70_001));
        wallet.apply_batch(&batch(3, "c3", vec![block(4, "c4", 7)], 4), &fvk).unwrap();
        assert_eq!(wallet.balance(), (70_001, 0));
    }

    /// One large batch skips checkpoints that pruning would discard; it must keep exactly the
    /// window that block-by-block batches keep, so anchors and reorg rollback behave the same.
    #[test]
    fn large_batches_keep_the_same_checkpoint_window_as_small_ones() {
        let fvk = crate::DeviceSecret::generate("A", "").fvk().unwrap();
        // Every third height is empty and not transported, like a compact range with gaps.
        let blocks: Vec<CompactBlock> = (1..=450u32).filter(|h| h % 3 != 0).map(|h| {
            let mut cmx = [0; 32];
            cmx[..4].copy_from_slice(&h.to_le_bytes());
            CompactBlock { height: h, hash: format!("h{h}"), txs: if h % 3 == 1 { vec![] } else { vec![CompactTx {
                txid: cmx, actions: vec![Action { nf: cmx, cmx, epk: [0; 32], ct: vec![0; 52] }] }] } }
        }).collect();
        let mut one = WalletState::default();
        one.apply_batch(&Batch { prev_height: 0, prev_hash: String::new(), seed: None, blocks: blocks.clone(), chain_tip: 450, sources: vec![] }, &fvk).unwrap();
        let mut small = WalletState::default();
        for chunk in blocks.chunks(7) {
            let batch = Batch { prev_height: small.tip_height, prev_hash: small.tip_hash.clone(), seed: None, blocks: chunk.to_vec(), chain_tip: 450, sources: vec![] };
            small.apply_batch(&batch, &fvk).unwrap();
        }
        assert_eq!((one.tip_height, &one.roots, &one.hashes), (small.tip_height, &small.roots, &small.hashes));
        assert_eq!(*one.roots.keys().next().unwrap(), one.tip_height - CHECKPOINTS as u32 + 1);
        // The oldest retained height is still a rollback target; one below it is not.
        for wallet in [&one, &small] {
            let oldest = *wallet.roots.keys().next().unwrap();
            let mut back = wallet.clone();
            back.rollback(oldest).unwrap();
            assert_eq!(back.rebuild_tree(&[]).unwrap().root_at_checkpoint_depth(Some(0)).unwrap().unwrap().to_bytes(),
                <[u8; 32]>::try_from(hex::decode(&wallet.roots[&oldest]).unwrap()).unwrap());
            assert!(wallet.clone().rollback(oldest - 1).is_err());
        }
    }
}
