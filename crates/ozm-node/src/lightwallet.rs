//! TLS lightwalletd gateway. No viewing keys, address queries or wallet scanning.
use anyhow::{anyhow, bail, ensure, Context, Result};
use std::time::Duration;
use tonic::{transport::{Channel, ClientTlsConfig, Endpoint}, Code};
use zcash_client_backend::proto::{compact_formats, service::{self, compact_tx_streamer_client::CompactTxStreamerClient}};
use zcash_offline_core::{messages::TxStatus, rpc::{classify_submit, SubmitDisposition}, sync::{Action, Batch, CompactBlock, CompactTx, Seed}};

type Client = CompactTxStreamerClient<Channel>;

pub struct Lightwallet {
    clients: Vec<(String, Client, u32)>,
    pub tip: u32,
    pub hash: String,
    pub sources: Vec<String>,
}

fn block_id(height: u32) -> service::BlockId {
    service::BlockId { height: height as u64, hash: vec![] }
}

fn display_hash(bytes: &[u8]) -> Result<String> {
    ensure!(bytes.len() == 32, "server hash length");
    Ok(hex::encode(bytes.iter().rev().copied().collect::<Vec<_>>()))
}

fn compact(block: compact_formats::CompactBlock) -> Result<CompactBlock> {
    let mut txs = Vec::new();
    for tx in block.vtx.into_iter().filter(|t| !t.ironwood_actions.is_empty()) {
        txs.push(CompactTx { txid: tx.txid.try_into().map_err(|_| anyhow!("txid length"))?,
            actions: tx.ironwood_actions.into_iter().map(|a| -> Result<_> {
                let a = Action { nf: a.nullifier.try_into().map_err(|_| anyhow!("nf length"))?,
                    cmx: a.cmx.try_into().map_err(|_| anyhow!("cmx length"))?,
                    epk: a.ephemeral_key.try_into().map_err(|_| anyhow!("epk length"))?, ct: a.ciphertext };
                a.compact()?;
                Ok(a)
            }).collect::<Result<_>>()? });
    }
    Ok(CompactBlock { height: block.height.try_into()?, hash: display_hash(&block.hash)?, txs })
}

impl Lightwallet {
    pub async fn connect(servers: &[String]) -> Result<Self> {
        let mut clients = Vec::new();
        let mut errors = Vec::new();
        for url in servers {
            let result = async {
                ensure!(url.starts_with("https://"), "public testnet requires TLS (https://)");
                let channel = Endpoint::from_shared(url.clone())?.connect_timeout(Duration::from_secs(3))
                    .timeout(Duration::from_secs(20)).tls_config(ClientTlsConfig::new().with_webpki_roots())?.connect().await?;
                let mut client = Client::new(channel).max_decoding_message_size(8 * 1024 * 1024);
                let info = client.get_lightd_info(service::Empty {}).await?.into_inner();
                ensure!(info.chain_name == "test" && info.consensus_branch_id.eq_ignore_ascii_case("77190ad9"), "server is not NU7 public testnet");
                let tip = client.get_latest_block(service::ChainSpec {}).await?.into_inner();
                let height: u32 = tip.height.try_into()?;
                ensure!(height >= 4_465_026, "server is behind NU7");
                Ok::<_, anyhow::Error>((client, height, display_hash(&tip.hash)?))
            }.await;
            match result {
                Ok((client, height, hash)) => clients.push((url.clone(), client, height, hash)),
                Err(e) => errors.push(format!("{url}: {e:#}")),
            }
        }
        ensure!(!clients.is_empty(), "no testnet lightwallet server reachable: {}", errors.join("; "));
        let tip = clients[0].2;
        let hash = clients[0].3.clone();
        let mut sources = vec![clients[0].0.clone()];
        for i in 1..clients.len() {
            let common = tip.min(clients[i].2);
            let a = clients[0].1.get_block(block_id(common)).await?.into_inner();
            let b = clients[i].1.get_block(block_id(common)).await;
            if let Ok(b) = b {
                ensure!(a.hash == b.into_inner().hash, "testnet servers disagree at height {common}; chain view withheld");
                sources.push(clients[i].0.clone());
            }
        }
        Ok(Self { clients: clients.into_iter().map(|(s, c, h, _)| (s, c, h)).collect(), tip, hash, sources })
    }

    pub async fn block_hash(&mut self, height: u32) -> Result<String> {
        let block = self.clients[0].1.get_block(block_id(height)).await?.into_inner();
        for i in 1..self.clients.len() {
            if self.clients[i].2 < height { self.sources.retain(|s| s != &self.clients[i].0); continue; }
            match self.clients[i].1.get_block(block_id(height)).await {
                Ok(other) => ensure!(block.hash == other.into_inner().hash, "testnet block-hash disagreement at {height}"),
                Err(_) => self.sources.retain(|s| s != &self.clients[i].0),
            }
        }
        display_hash(&block.hash)
    }

    pub async fn seed(&mut self, height: u32) -> Result<Seed> {
        let tree = self.clients[0].1.get_tree_state(block_id(height)).await?.into_inner();
        ensure!(tree.height == height as u64 && tree.network == "test", "wrong tree-state height/network");
        let block = self.clients[0].1.get_block(block_id(height)).await?.into_inner();
        ensure!(display_hash(&block.hash)? == tree.hash, "chain changed while fetching frontier");
        let size = block.chain_metadata.ok_or_else(|| anyhow!("missing Ironwood tree-size metadata"))?.ironwood_commitment_tree_size;
        zcash_offline_core::wallet::check_frontier_size(&tree.ironwood_tree, size as u64)?;
        for i in 1..self.clients.len() {
            if let Ok(other) = self.clients[i].1.get_tree_state(block_id(height)).await {
                let other = other.into_inner();
                ensure!(tree.hash == other.hash && tree.ironwood_tree == other.ironwood_tree, "testnet tree-state disagreement at {height}");
            } else {
                self.sources.retain(|s| s != &self.clients[i].0);
            }
        }
        Ok(Seed { height, hash: tree.hash, frontier: tree.ironwood_tree })
    }

    pub async fn tip_time(&mut self) -> Result<u64> {
        Ok(self.clients[0].1.get_block(block_id(self.tip)).await?.into_inner().time as u64)
    }

    async fn range(client: &mut Client, start: u32, end: u32, predecessor: &str) -> Result<Vec<CompactBlock>> {
        let mut stream = client.get_block_range(service::BlockRange { start: Some(block_id(start)), end: Some(block_id(end)), pool_types: vec![] }).await?.into_inner();
        let mut blocks = Vec::new();
        let mut previous = hex::decode(predecessor)?;
        previous.reverse();
        let mut height = start;
        while let Some(block) = stream.message().await? {
            ensure!(block.height == height as u64 && height <= end, "nonconsecutive compact block range");
            ensure!(previous == block.prev_hash, "compact range changed forks");
            previous = block.hash.clone();
            blocks.push(compact(block)?);
            height += 1;
        }
        ensure!(height == end + 1, "truncated compact block stream");
        Ok(blocks)
    }

    /// Up to `max_blocks` compact blocks after `have` (fewer if they exceed the sync memory limit).
    pub async fn batch(&mut self, have: u32, hash: &str, checkpoints: &[(u32, String)], birthday: u32, max_blocks: u32) -> Result<Batch> {
        let (mut prev_height, mut prev_hash) = (have, hash.to_string());
        let seed = if have == 0 {
            ensure!(birthday >= 4_465_026 && birthday <= self.tip + 1, "testnet birthday must be between NU7 and the next block");
            let seed = self.seed(birthday - 1).await?;
            prev_height = seed.height;
            prev_hash = seed.hash.clone();
            Some(seed)
        } else {
            if have > self.tip || self.block_hash(have).await? != hash {
                let mut ancestor = None;
                let tip = self.tip;
                for (h, hash) in checkpoints.iter().rev().filter(|(h, _)| *h <= tip) {
                    if self.block_hash(*h).await? == *hash { ancestor = Some((*h, hash.clone())); break; }
                }
                let (h, hash) = ancestor.ok_or_else(|| anyhow!("reorg exceeds retained checkpoints; resync from the original birthday"))?;
                prev_height = h;
                prev_hash = hash;
            }
            None
        };
        let end = self.tip.min(prev_height + max_blocks.max(1));
        let mut batch = Batch { prev_height, prev_hash, seed, blocks: vec![], chain_tip: self.tip, sources: self.sources.clone() };
        if end <= prev_height { return Ok(batch); }
        let blocks = Self::range(&mut self.clients[0].1, prev_height + 1, end, &batch.prev_hash).await?;
        // Compare the actual compact answer, not just a claimed server tip.
        for i in 1..self.clients.len() {
            if self.clients[i].2 < end { batch.sources.retain(|s| s != &self.clients[i].0); continue; }
            if let Ok(other) = Self::range(&mut self.clients[i].1, prev_height + 1, end, &batch.prev_hash).await {
                ensure!(blocks == other, "testnet servers disagree on compact blocks; sync withheld");
            } else { batch.sources.retain(|s| s != &self.clients[i].0); }
        }
        for block in blocks { batch.push(block); }
        while bincode::serialized_size(&batch)? > zcash_offline_core::sync::MAX_BATCH_BYTES as u64 {
            ensure!(batch.blocks.len() > 1, "one compact block exceeds sync memory limit");
            batch.blocks.pop();
        }
        // A fork can change between GetLatestBlock and GetBlockRange.
        let last = batch.blocks.last().unwrap();
        ensure!(self.block_hash(last.height).await? == last.hash, "chain changed while serving compact range");
        Ok(batch)
    }

    /// Reconcile before expiry or resubmission. GetTransaction distinguishes mempool
    /// (0), orphaned (-1) and canonical mined heights; no server-side key scanning.
    pub async fn status(&mut self, raw: &[u8], txid: &str) -> Result<Option<TxStatus>> {
        let mut hash = hex::decode(txid)?;
        hash.reverse();
        let filter = service::TxFilter { hash, ..Default::default() };
        let tx = match self.clients[0].1.get_transaction(filter.clone()).await {
            Ok(tx) => Some(tx.into_inner()),
            Err(e) if e.code() == Code::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        for i in 1..self.clients.len() {
            let height = tx.as_ref().map_or(0, |t| t.height);
            if height != u64::MAX && (self.clients[i].2 as u64) < height { self.sources.retain(|s| s != &self.clients[i].0); continue; }
            match self.clients[i].1.get_transaction(filter.clone()).await {
                Ok(other) => {
                    let other = other.into_inner();
                    ensure!(other.data == raw, "server returned a different transaction variant");
                    ensure!(tx.as_ref().is_some_and(|t| t.height == other.height), "testnet transaction inclusion disagreement");
                }
                Err(e) if e.code() == Code::NotFound && tx.as_ref().is_some_and(|t| t.height > 0 && t.height != u64::MAX) => bail!("testnet servers disagree on transaction inclusion"),
                Err(e) if e.code() == Code::NotFound && tx.is_none() => {},
                Err(_) => { self.sources.retain(|s| s != &self.clients[i].0); }
            }
        }
        let Some(tx) = tx else { return Ok(None); };
        ensure!(tx.data == raw, "server returned a different transaction variant");
        if tx.height == u64::MAX { return Ok(None); }
        if tx.height == 0 { return Ok(Some(TxStatus::Mempool)); }
        let height: u32 = tx.height.try_into()?;
        ensure!(height <= self.tip, "transaction height is ahead of server tip");
        let block_hash = self.block_hash(height).await?;
        Ok(Some(if self.tip - height + 1 >= 10 { TxStatus::Mined { height, block_hash } } else { TxStatus::Included { height, block_hash } }))
    }

    pub async fn submit(&mut self, raw: &[u8]) -> Result<TxStatus> {
        let mut calls = tokio::task::JoinSet::new();
        for (_, client, _) in &self.clients {
            let mut client = client.clone();
            let tx = service::RawTransaction { data: raw.to_vec(), height: 0 };
            calls.spawn(async move { client.send_transaction(tx).await });
        }
        let mut results = Vec::new();
        while let Some(call) = calls.join_next().await {
            let result = match call? {
                Ok(r) => {
                    let response = r.into_inner();
                    Ok(match classify_submit(response.error_code as i64, &response.error_message) {
                        SubmitDisposition::Accepted | SubmitDisposition::Duplicate => TxStatus::Submitted,
                        SubmitDisposition::Retryable => TxStatus::Retryable { reason: response.error_message },
                        SubmitDisposition::Conflict => TxStatus::Conflict { reason: response.error_message },
                        SubmitDisposition::Rejected => TxStatus::Rejected { reason: format!("{}: {}", response.error_code, response.error_message) },
                    })
                }
                Err(e) if matches!(e.code(), Code::Unavailable | Code::DeadlineExceeded | Code::ResourceExhausted | Code::Unknown | Code::Internal) =>
                    Ok(TxStatus::Retryable { reason: e.to_string() }),
                Err(e) => Err(e).context("SendTransaction"),
            };
            results.push(result);
        }
        // Any accepting server may mine it. A failed peer must not hide that possibility.
        if results.iter().any(|r| matches!(r, Ok(TxStatus::Submitted))) { return Ok(TxStatus::Submitted); }
        if let Some(Ok(status)) = results.iter().find(|r| matches!(r, Ok(TxStatus::Retryable { .. }))) { return Ok(status.clone()); }
        results.into_iter().next().ok_or_else(|| anyhow!("no submission servers"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unavailable_tls_server_returns_an_error_instead_of_panicking() {
        let result = Lightwallet::connect(&["https://127.0.0.1:0".into()]).await;
        assert!(result.err().unwrap().to_string().starts_with("no testnet lightwallet server reachable:"));
        let result = Lightwallet::connect(&["http://127.0.0.1:0".into()]).await;
        assert!(result.err().unwrap().to_string().contains("requires TLS"));
    }

    #[tokio::test]
    async fn submission_reaches_the_other_server_when_primary_fails() {
        let mut clients = Vec::new();
        let mut servers = Vec::new();
        for status in ["14", "0"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let router = axum::Router::new().fallback(move || async move {
                axum::http::Response::builder().header("content-type", "application/grpc").header("grpc-status", status)
                    .body(axum::body::Body::from(vec![0u8; 5])).unwrap()
            });
            servers.push(tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); }));
            let channel = Endpoint::from_shared(url.clone()).unwrap().connect().await.unwrap();
            clients.push((url, Client::new(channel), 4_465_026));
        }
        let mut source = Lightwallet { clients, tip: 4_465_026, hash: String::new(), sources: vec![] };
        assert!(matches!(source.submit(b"unchanged transaction bytes").await.unwrap(), TxStatus::Submitted));
        for server in servers { server.abort(); }
    }
}
