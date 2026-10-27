//! Serialized pruned ShardTree state, committed atomically with the wallet cursor.
use anyhow::{anyhow, Result};
use incrementalmerkletree::{Address, Position};
use orchard::tree::MerkleHashOrchard;
use serde::{Deserialize, Serialize};
use shardtree::{store::{memory::MemoryShardStore, Checkpoint, ShardStore, TreeState}, LocatedPrunableTree};
use zcash_client_backend::serialization::shardtree::{read_shard, write_shard};

use crate::wallet::Tree;

pub const CHECKPOINTS: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeData {
    cap: String,
    shards: Vec<(u64, String)>,
    checkpoints: Vec<(u32, Option<u64>, Vec<u64>)>,
}

impl TreeData {
    pub fn load(&self) -> Result<Tree> {
        let mut store = MemoryShardStore::empty();
        store.put_cap(read_shard::<MerkleHashOrchard, _>(&hex::decode(&self.cap)?[..])?)?;
        for (index, bytes) in &self.shards {
            let root = read_shard::<MerkleHashOrchard, _>(&hex::decode(bytes)?[..])?;
            store.put_shard(LocatedPrunableTree::from_parts(Address::from_parts(16.into(), *index), root)
                .map_err(|e| anyhow!("stored shard: {e:?}"))?)?;
        }
        for (height, pos, removed) in &self.checkpoints {
            store.add_checkpoint(*height, Checkpoint::from_parts(
                pos.map_or(TreeState::Empty, |p| TreeState::AtPosition(Position::from(p))),
                removed.iter().copied().map(Position::from).collect(),
            ))?;
        }
        Ok(Tree::new(store, CHECKPOINTS))
    }

    pub fn capture(tree: &Tree) -> Result<Self> {
        let encode = |root: &shardtree::PrunableTree<MerkleHashOrchard>| -> Result<String> {
            let mut bytes = Vec::new();
            write_shard(&mut bytes, root)?;
            Ok(hex::encode(bytes))
        };
        let store = tree.store();
        let cap = encode(&store.get_cap()?)?;
        let mut shards = Vec::new();
        for address in store.get_shard_roots()? {
            let shard = store.get_shard(address)?.ok_or_else(|| anyhow!("missing shard"))?;
            shards.push((address.index(), encode(shard.root())?));
        }
        let mut checkpoints = Vec::new();
        store.for_each_checkpoint(CHECKPOINTS, |h, c| {
            checkpoints.push((*h, c.position().map(u64::from), c.marks_removed().iter().copied().map(u64::from).collect()));
            Ok(())
        })?;
        Ok(Self { cap, shards, checkpoints })
    }
}
