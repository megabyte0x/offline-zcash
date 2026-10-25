// Scratch proof of concept: rebuild the ZIP 221 V3 history tree from node RPC data,
// check it against every header's hashBlockCommitments, then produce and verify an
// MMR inclusion path for one block's Ironwood root. Deleted after the research task.
use std::io::{Cursor, Read};
use std::process::Command;

use primitive_types::U256;
use zcash_history::{NodeData, NodeDataV2, NodeDataV3, Tree, Version, V3};
use zcash_primitives::{block::BlockHeader, transaction::Transaction};
use zcash_protocol::consensus::BranchId;

const BRANCH: u32 = 0x77190AD9;

fn rpc(method: &str, params: &str) -> serde_json::Value {
    let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let out = Command::new("curl")
        .args(["-s", "-X", "POST", "-H", "content-type: application/json", "--data", &body, "http://127.0.0.1:18232"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["result"].clone()
}

fn h32(s: &str, reverse: bool) -> [u8; 32] {
    let mut b: [u8; 32] = hex::decode(s).unwrap().try_into().unwrap();
    if reverse {
        b.reverse();
    }
    b
}

fn read_compact(c: &mut Cursor<&[u8]>) -> u64 {
    let mut b = [0u8; 1];
    c.read_exact(&mut b).unwrap();
    match b[0] {
        x @ 0..=0xfc => x as u64,
        0xfd => { let mut x = [0u8; 2]; c.read_exact(&mut x).unwrap(); u16::from_le_bytes(x) as u64 }
        0xfe => { let mut x = [0u8; 4]; c.read_exact(&mut x).unwrap(); u32::from_le_bytes(x) as u64 }
        _ => { let mut x = [0u8; 8]; c.read_exact(&mut x).unwrap(); u64::from_le_bytes(x) }
    }
}

fn work_from_bits(bits: u32) -> U256 {
    let exp = bits >> 24;
    let mant = U256::from(bits & 0x007f_ffff);
    let target = if exp <= 3 { mant >> (8 * (3 - exp)) } else { mant << (8 * (exp - 3)) };
    (!target / (target + U256::one())) + U256::one()
}

fn blake(personal: &[u8; 16], parts: &[&[u8]]) -> [u8; 32] {
    let mut st = blake2b_simd::Params::new().hash_length(32).personal(personal).to_state();
    for p in parts {
        st.update(p);
    }
    st.finalize().as_bytes().try_into().unwrap()
}

fn sha256d(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(Sha256::digest(data)).into()
}

struct BlockInfo {
    header_bytes: Vec<u8>,
    header: BlockHeader,
    txids: Vec<[u8; 32]>,
    auth_root: [u8; 32],
    leaf: NodeDataV3,
}

fn fetch(h: u32, rev_roots: bool) -> BlockInfo {
    let raw = hex::decode(rpc("getblock", &format!(r#"["{h}", 0]"#)).as_str().unwrap()).unwrap();
    let mut c = Cursor::new(&raw[..]);
    let header = BlockHeader::read(&mut c).unwrap();
    let header_bytes = raw[..c.position() as usize].to_vec();
    let n = read_compact(&mut c);
    let (mut s_tx, mut o_tx, mut i_tx) = (0u64, 0u64, 0u64);
    let mut txids = vec![];
    let mut auths = vec![];
    for _ in 0..n {
        let tx = Transaction::read(&mut c, BranchId::Nu7).unwrap();
        s_tx += tx.sapling_bundle().is_some() as u64;
        o_tx += tx.orchard_bundle().is_some() as u64;
        i_tx += tx.ironwood_bundle().is_some() as u64;
        if let Some(b) = tx.ironwood_bundle() { println!("h={h} txid={} raw anchor bytes={}", tx.txid(), hex::encode(b.anchor().to_bytes())); }
        txids.push(*tx.txid().as_ref());
        auths.push(<[u8; 32]>::try_from(tx.auth_commitment().as_bytes()).unwrap());
    }
    assert_eq!(c.position() as usize, raw.len());
    // ZIP 244 hashAuthDataRoot: pad to a power of two with zeros, BLAKE2b "ZcashAuthDatHash".
    let mut layer = auths.clone();
    layer.resize(layer.len().next_power_of_two(), [0u8; 32]);
    while layer.len() > 1 {
        layer = layer.chunks(2).map(|p| blake(b"ZcashAuthDatHash", &[&p[0], &p[1]])).collect();
    }
    let ts = rpc("z_gettreestate", &format!(r#"["{h}"]"#));
    let rv = std::env::var("ROOTREV").unwrap_or_default(); let _ = rev_roots;
    let root = |pool: &str| h32(ts[pool]["commitments"]["finalRoot"].as_str().unwrap(), rv.contains(pool));
    let leaf = NodeDataV3 {
        v2: NodeDataV2 {
            v1: NodeData {
                consensus_branch_id: BRANCH,
                subtree_commitment: header.hash().0,
                start_time: header.time,
                end_time: header.time,
                start_target: header.bits,
                end_target: header.bits,
                start_sapling_root: root("sapling"),
                end_sapling_root: root("sapling"),
                subtree_total_work: work_from_bits(header.bits),
                start_height: h as u64,
                end_height: h as u64,
                sapling_tx: s_tx,
            },
            start_orchard_root: root("orchard"),
            end_orchard_root: root("orchard"),
            orchard_tx: o_tx,
        },
        start_ironwood_root: root("ironwood"),
        end_ironwood_root: root("ironwood"),
        ironwood_tx: i_tx,
    };
    BlockInfo { header_bytes, header, txids, auth_root: layer[0], leaf }
}

// ---- Gateway side: plain array MMR that keeps every node so it can emit paths ----
struct Mmr {
    nodes: Vec<NodeDataV3>,
    peaks: Vec<(usize, u32)>, // (node index, altitude)
    parent: Vec<Option<usize>>,
    sibling: Vec<Option<(usize, bool)>>, // (sibling index, sibling_is_left)
    leaf_idx: Vec<usize>,
}

impl Mmr {
    fn push(&mut self, d: NodeDataV3) -> usize {
        self.nodes.push(d);
        self.parent.push(None);
        self.sibling.push(None);
        self.nodes.len() - 1
    }
    fn append(&mut self, leaf: NodeDataV3) {
        let mut cur = (self.push(leaf), 0u32);
        self.leaf_idx.push(cur.0);
        while let Some(&(l, alt)) = self.peaks.last() {
            if alt != cur.1 { break; }
            self.peaks.pop();
            let p = self.push(V3::combine(&self.nodes[l], &self.nodes[cur.0]));
            self.parent[l] = Some(p);
            self.parent[cur.0] = Some(p);
            self.sibling[l] = Some((cur.0, false));
            self.sibling[cur.0] = Some((l, true));
            cur = (p, alt + 1);
        }
        self.peaks.push(cur);
    }
    fn bag(peaks: &[NodeDataV3]) -> NodeDataV3 {
        let mut acc = peaks[0].clone();
        for p in &peaks[1..] {
            acc = V3::combine(&acc, p);
        }
        acc
    }
    fn root(&self) -> [u8; 32] {
        let ps: Vec<_> = self.peaks.iter().map(|&(i, _)| self.nodes[i].clone()).collect();
        V3::hash(&Self::bag(&ps))
    }
    /// Path for leaf number k: serialized siblings bottom-up, then all peaks (the leaf's peak omitted).
    fn proof(&self, k: usize) -> Proof {
        let mut i = self.leaf_idx[k];
        let mut path = vec![];
        while let Some((s, s_left)) = self.sibling[i] {
            path.push((V3::to_bytes(&self.nodes[s]), s_left));
            i = self.parent[i].unwrap();
        }
        let pos = self.peaks.iter().position(|&(p, _)| p == i).unwrap();
        let peaks = self
            .peaks
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != pos)
            .map(|(_, &(p, _))| V3::to_bytes(&self.nodes[p]))
            .collect();
        Proof { leaf: V3::to_bytes(&self.nodes[self.leaf_idx[k]]), path, peak_pos: pos, peaks }
    }
}

struct Proof {
    leaf: Vec<u8>,
    path: Vec<(Vec<u8>, bool)>,
    peak_pos: usize,
    peaks: Vec<Vec<u8>>,
}

impl Proof {
    fn size(&self) -> usize {
        self.leaf.len() + self.path.iter().map(|p| p.0.len() + 1).sum::<usize>() + 1 + self.peaks.iter().map(|p| p.len()).sum::<usize>()
    }
}

// ---- Payee side: tree-root logic only (zcash_history V3::combine / V3::hash) ----
fn payee_verify(anchor: [u8; 32], proof: &Proof, auth_root_next: [u8; 32], header_next_commitments: [u8; 32]) -> bool {
    let leaf = V3::from_bytes(BRANCH, &proof.leaf).unwrap();
    if leaf.end_ironwood_root != anchor {
        return false;
    }
    let mut node = leaf;
    for (sib, sib_left) in &proof.path {
        let s = V3::from_bytes(BRANCH, sib).unwrap();
        node = if *sib_left { V3::combine(&s, &node) } else { V3::combine(&node, &s) };
    }
    let mut peaks: Vec<NodeDataV3> = proof.peaks.iter().map(|p| V3::from_bytes(BRANCH, p).unwrap()).collect();
    peaks.insert(proof.peak_pos, node);
    let history_root = V3::hash(&Mmr::bag(&peaks));
    blake(b"ZcashBlockCommit", &[&history_root, &auth_root_next, &[0u8; 32]]) == header_next_commitments
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rev_roots = args.get(1).map(|s| s == "rev").unwrap_or(false);
    let anchor_height: u32 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let tip = rpc("getblockcount", "[]").as_u64().unwrap() as u32;
    let blocks: Vec<BlockInfo> = (1..=tip).map(|h| fetch(h, rev_roots)).collect();

    // Cross-check: my array MMR vs zcash_history::Tree, and both vs every header.
    let mut mmr = Mmr { nodes: vec![], peaks: vec![], parent: vec![], sibling: vec![], leaf_idx: vec![] };
    let mut tree: Option<Tree<V3>> = None;
    let mut ok = 0;
    for (k, b) in blocks.iter().enumerate() {
        mmr.append(b.leaf.clone());
        match &mut tree {
            None => tree = Some(Tree::new(1, vec![(0, zcash_history::Entry::new_leaf(b.leaf.clone()))], vec![])),
            Some(t) => { t.append_leaf(b.leaf.clone()).unwrap(); }
        }
        let lib_root = V3::hash(tree.as_ref().unwrap().root_node().unwrap().data());
        assert_eq!(lib_root, mmr.root(), "array MMR disagrees with zcash_history at leaf {k}");
        if let Some(next) = blocks.get(k + 1) {
            let c = blake(b"ZcashBlockCommit", &[&mmr.root(), &next.auth_root, &[0u8; 32]]);
            if c == next.header.final_sapling_root { ok += 1 } else if ok == 0 && k < 3 {
                println!("mismatch at height {}: computed {} header {}", k + 1, hex::encode(c), hex::encode(next.header.final_sapling_root));
            }
        }
    }
    println!("tip {tip}; history roots matching header N+1 hashBlockCommitments: {ok}/{}", tip - 1);
    println!("header size {} bytes", blocks[0].header_bytes.len());
    let merkle_ok = blocks.iter().filter(|b| {
        let mut l = b.txids.clone();
        while l.len() > 1 {
            if l.len() % 2 == 1 { l.push(*l.last().unwrap()); }
            l = l.chunks(2).map(|p| sha256d(&[p[0], p[1]].concat())).collect();
        }
        l[0] == b.header.merkle_root
    }).count();
    println!("txid merkle roots matching header: {merkle_ok}/{tip}");

    if anchor_height > 0 {
        let k = (anchor_height - 1) as usize;
        let anchor = blocks[k].leaf.end_ironwood_root;
        for proof_tip in [anchor_height + 1, tip] {
            // Tree as of header `proof_tip` covers leaves 1..proof_tip-1.
            let mut m = Mmr { nodes: vec![], peaks: vec![], parent: vec![], sibling: vec![], leaf_idx: vec![] };
            for b in &blocks[..(proof_tip - 1) as usize] { m.append(b.leaf.clone()); }
            let p = m.proof(k);
            let hdr = &blocks[(proof_tip - 1) as usize];
            let good = payee_verify(anchor, &p, hdr.auth_root, hdr.header.final_sapling_root);
            let mut bad_anchor = anchor; bad_anchor[0] ^= 1;
            let bad = payee_verify(bad_anchor, &p, hdr.auth_root, hdr.header.final_sapling_root);
            let mut tampered = Proof { leaf: p.leaf.clone(), path: p.path.clone(), peak_pos: p.peak_pos, peaks: p.peaks.clone() };
            let off = tampered.leaf.len() - 40; tampered.leaf[off] ^= 1; // flip a byte of hashEarliest/LatestIronwoodRoot
            let forged = payee_verify(V3::from_bytes(BRANCH, &tampered.leaf).unwrap().end_ironwood_root, &tampered, hdr.auth_root, hdr.header.final_sapling_root);
            println!(
                "anchor h={anchor_height} root={} proven by header {proof_tip}: valid={good} wrong_anchor_rejected={} tampered_leaf_rejected={} | leaf {} B, {} siblings, {} other peaks, proof {} B (+32 B authDataRoot)",
                hex::encode(anchor), !bad, !forged, p.leaf.len(), p.path.len(), p.peaks.len(), p.size()
            );
        }
    }
}
