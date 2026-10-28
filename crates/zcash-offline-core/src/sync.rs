//! Ironwood-only mesh sync. Empty block runs carry their end height/hash. Batches
//! are fragmented at byte boundaries, including inside a transaction or block.
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::BranchId;

// Leave room for the authenticated envelope inside a 4 KiB padding bucket.
pub const CHUNK_BYTES: usize = 3 * 1024;
pub const MAX_BATCH_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub nf: [u8; 32],
    pub cmx: [u8; 32],
    pub epk: [u8; 32],
    pub ct: Vec<u8>,
}

impl Action {
    pub fn compact(&self) -> Result<orchard::note_encryption::CompactAction> {
        Ok(orchard::note_encryption::CompactAction::from_parts(
            Option::from(orchard::note::Nullifier::from_bytes(&self.nf)).ok_or_else(|| anyhow!("invalid nullifier"))?,
            Option::from(orchard::note::ExtractedNoteCommitment::from_bytes(&self.cmx)).ok_or_else(|| anyhow!("invalid cmx"))?,
            zcash_note_encryption::EphemeralKeyBytes(self.epk),
            self.ct.as_slice().try_into().map_err(|_| anyhow!("compact ciphertext must be 52 bytes"))?,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompactTx {
    pub txid: [u8; 32],
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompactBlock {
    /// Empty transactions mean an empty run ending at this height/hash.
    pub height: u32,
    pub hash: String,
    pub txs: Vec<CompactTx>,
}

impl CompactBlock {
    pub fn from_raw(height: u32, hash: String, txs: &[Vec<u8>]) -> Result<Self> {
        let mut compact = Vec::new();
        for raw in txs {
            let tx = Transaction::read(&raw[..], BranchId::Nu7)?;
            if let Some(bundle) = tx.ironwood_bundle() {
                compact.push(CompactTx { txid: *tx.txid().as_ref(), actions: bundle.actions().iter().map(|a| Action {
                    nf: a.nullifier().to_bytes(), cmx: a.cmx().to_bytes(), epk: a.encrypted_note().epk_bytes,
                    ct: a.encrypted_note().enc_ciphertext[..52].to_vec(),
                }).collect() });
            }
        }
        Ok(Self { height, hash, txs: compact })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Seed {
    pub height: u32,
    pub hash: String,
    /// GetTreeState's serialized commitment tree, not a byte-reversed root.
    pub frontier: String,
}

impl Seed {
    pub fn root(&self) -> Result<String> {
        let bytes = hex::decode(&self.frontier)?;
        let tree = zcash_primitives::merkle_tree::read_frontier_v0::<orchard::tree::MerkleHashOrchard, _>(&bytes[..])?;
        Ok(hex::encode(tree.root().to_bytes()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Batch {
    pub prev_height: u32,
    pub prev_hash: String,
    pub seed: Option<Seed>,
    pub blocks: Vec<CompactBlock>,
    pub chain_tip: u32,
    pub sources: Vec<String>,
}

impl Batch {
    pub fn push(&mut self, block: CompactBlock) {
        if block.txs.is_empty() && self.blocks.last().is_some_and(|b| b.txs.is_empty() && b.height % 1_000 != 0) {
            *self.blocks.last_mut().unwrap() = block;
        } else {
            self.blocks.push(block);
        }
    }

    pub fn chunks(&self) -> Result<Vec<Part>> {
        let bytes = bincode::serialize(self)?;
        ensure!(bytes.len() <= MAX_BATCH_BYTES, "sync batch exceeds memory limit");
        let id = hex::encode(blake2b_simd::blake2b(&bytes).as_bytes());
        Ok(bytes.chunks(CHUNK_BYTES).enumerate().map(|(index, data)| Part {
            id: id.clone(), index: index as u32, count: bytes.len().div_ceil(CHUNK_BYTES) as u32, data: data.to_vec(),
        }).collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub id: String,
    pub index: u32,
    pub count: u32,
    pub data: Vec<u8>,
}

/// One bounded, out-of-order response. Only a complete, hash-checked batch is applied.
#[derive(Default)]
pub struct Assembly {
    id: String,
    parts: Vec<Option<Vec<u8>>>,
}

impl Assembly {
    pub fn insert(&mut self, part: Part) -> Result<Option<Batch>> {
        ensure!(part.count > 0 && part.count as usize <= MAX_BATCH_BYTES.div_ceil(CHUNK_BYTES)
            && part.index < part.count && part.data.len() <= CHUNK_BYTES, "invalid sync fragment");
        if self.id != part.id {
            self.id = part.id.clone();
            self.parts = vec![None; part.count as usize];
        }
        ensure!(self.parts.len() == part.count as usize, "sync fragment count changed");
        self.parts[part.index as usize] = Some(part.data);
        if self.parts.iter().any(Option::is_none) { return Ok(None); }
        let bytes: Vec<u8> = self.parts.iter().flat_map(|p| p.as_ref().unwrap().iter().copied()).collect();
        ensure!(bytes.len() <= MAX_BATCH_BYTES && hex::encode(blake2b_simd::blake2b(&bytes).as_bytes()) == self.id, "sync batch digest mismatch");
        let batch = bincode::deserialize(&bytes)?;
        *self = Self::default();
        Ok(Some(batch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_block_crosses_chunks_without_loss_or_partial_commit() {
        let mut batch = Batch { prev_height: 91, prev_hash: "parent".into(), seed: None, blocks: vec![], chain_tip: 94, sources: vec![] };
        batch.push(CompactBlock { height: 92, hash: "empty92".into(), txs: vec![] });
        batch.push(CompactBlock { height: 93, hash: "empty93".into(), txs: vec![] });
        batch.push(CompactBlock { height: 94, hash: "large".into(), txs: vec![CompactTx { txid: [9; 32],
            actions: (0..4097).map(|i| Action { nf: [i as u8; 32], cmx: [(i / 256) as u8; 32], epk: [3; 32], ct: vec![7; 52] }).collect() }] });
        assert_eq!(batch.blocks.len(), 2);
        assert!(bincode::serialized_size(&batch).unwrap() > crate::envelope::MAX_PLAINTEXT as u64);
        let mut chunks = batch.chunks().unwrap();
        chunks.reverse();
        let mut assembly = Assembly::default();
        for chunk in chunks.iter().take(chunks.len() - 1) {
            assert!(assembly.insert(chunk.clone()).unwrap().is_none());
        }
        assert_eq!(assembly.insert(chunks.last().unwrap().clone()).unwrap().unwrap(), batch);
    }
}
