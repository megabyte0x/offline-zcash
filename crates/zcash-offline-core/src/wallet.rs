//! Online wallet sync and the offline note inventory.
//!
//! The device scans the chain over Zakura RPC while it is online, keeping every
//! Ironwood note commitment (to rebuild witnesses offline), the tree root after
//! each block (the payee's known anchors), and its own notes. After that it can
//! build (payer) or check (payee) payments with no connection.

use anyhow::{anyhow, Context, Result};
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

use crate::rpc::Rpc;

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
    /// Every Ironwood note commitment on the chain, in tree order (hex).
    pub commitments: Vec<String>,
    /// Ironwood tree root after each scanned block (hex), by height.
    pub roots: BTreeMap<u32, String>,
    pub notes: Vec<StoredNote>,
    /// Every Ironwood nullifier revealed on chain (hex), for conflict checks.
    #[serde(default)]
    pub chain_nullifiers: std::collections::BTreeSet<String>,
}

impl WalletState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
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

    /// Height at which this anchor (tree root) was the tip root, if known.
    pub fn anchor_height(&self, anchor: &[u8; 32]) -> Option<u32> {
        let a = hex::encode(anchor);
        self.roots.iter().rev().find(|(_, r)| **r == a).map(|(h, _)| *h)
    }

    fn rebuild_tree(&self, marked: &[u64]) -> Result<Tree> {
        let mut tree = Tree::new(MemoryShardStore::empty(), 1000);
        for (i, c) in self.commitments.iter().enumerate() {
            let b: [u8; 32] = hex::decode(c)?.try_into().map_err(|_| anyhow!("cmx"))?;
            let leaf = Option::from(MerkleHashOrchard::from_bytes(&b)).ok_or_else(|| anyhow!("cmx"))?;
            let r = if marked.contains(&(i as u64)) { Retention::Marked } else { Retention::Ephemeral };
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

    /// Scans new blocks from the node. Rescans from scratch after a reorg.
    pub fn sync(&mut self, rpc: &Rpc, fvk: &FullViewingKey) -> Result<SyncReport> {
        let chain_tip = rpc.tip()?;
        let mut rescanned = false;
        if self.tip_height > 0 && (self.tip_height > chain_tip || rpc.block_hash(self.tip_height)? != self.tip_hash) {
            let notes = std::mem::take(&mut self.notes);
            *self = WalletState::default();
            // Keep offline locks across the rescan, so a reorg never frees a note we already paid with.
            self.notes = notes.into_iter().filter(|n| n.locked_by.is_some()).map(|mut n| { n.spent_in = None; n }).collect();
            rescanned = true;
        }
        let ivk = PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::External));
        let mut tree = self.rebuild_tree(&[])?;
        let mut new_notes = 0;
        for h in self.tip_height + 1..=chain_tip {
            let (hash, txs) = rpc.block_txs(h).with_context(|| format!("block {h}"))?;
            for bytes in txs {
                let tx = Transaction::read(&bytes[..], BranchId::Nu7).with_context(|| format!("parse tx in block {h}"))?;
                let txid = tx.txid().to_string();
                let Some(bundle) = tx.ironwood_bundle() else { continue };
                for action in bundle.actions() {
                    let nf = hex::encode(action.nullifier().to_bytes());
                    self.chain_nullifiers.insert(nf.clone());
                    for n in self.notes.iter_mut().filter(|n| n.nullifier == nf) {
                        n.spent_in = Some(txid.clone());
                    }
                    let domain = orchard::note_encryption::IronwoodDomain::for_action(action);
                    if let Some((note, _, _)) = zcash_note_encryption::try_note_decryption(&domain, &ivk, action) {
                        let position = self.commitments.len() as u64;
                        let nullifier = hex::encode(note.nullifier(fvk).to_bytes());
                        // A rescan after a reorg may meet a note we kept because it was locked.
                        if !self.notes.iter().any(|n| n.nullifier == nullifier) {
                            self.notes.push(StoredNote {
                                position,
                                height: h,
                                txid: txid.clone(),
                                value: note.value().inner(),
                                recipient: hex::encode(note.recipient().to_raw_address_bytes()),
                                rho: hex::encode(note.rho().to_bytes()),
                                rseed: hex::encode(note.rseed().as_bytes()),
                                nullifier,
                                spent_in: None,
                                locked_by: None,
                            });
                            new_notes += 1;
                        } else if let Some(n) = self.notes.iter_mut().find(|n| n.nullifier == nullifier) {
                            n.position = position;
                            n.height = h;
                        }
                    }
                    let cmx = action.cmx().to_bytes();
                    self.commitments.push(hex::encode(cmx));
                    let leaf = Option::from(MerkleHashOrchard::from_bytes(&cmx)).ok_or_else(|| anyhow!("cmx"))?;
                    tree.append(leaf, Retention::Ephemeral).map_err(|e| anyhow!("tree append: {e:?}"))?;
                }
            }
            tree.checkpoint(h).map_err(|e| anyhow!("checkpoint: {e:?}"))?;
            let root = tree
                .root_at_checkpoint_id(&h)
                .map_err(|e| anyhow!("root: {e:?}"))?
                .ok_or_else(|| anyhow!("no root at {h}"))?;
            self.roots.insert(h, hex::encode(root.to_bytes()));
            self.tip_height = h;
            self.tip_hash = hash;
        }
        // Unlock notes whose offline payment was mined (spent_in is set) — they are spent, not locked.
        for n in self.notes.iter_mut() {
            if n.spent_in.is_some() {
                n.locked_by = None;
            }
        }
        Ok(SyncReport { tip: self.tip_height, new_notes, rescanned })
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
        let mut f = std::fs::File::create(&tmp)?;
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
