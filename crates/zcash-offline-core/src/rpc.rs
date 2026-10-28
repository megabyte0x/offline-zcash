//! Minimal Zakura JSON-RPC client (online steps only: sync and gateway).

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Rpc {
    url: String,
    agent: ureq::Agent,
}

/// A JSON-RPC error returned by the node (as opposed to a transport failure).
#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rpc error {}: {}", self.code, self.message)
    }
}
impl std::error::Error for RpcError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitDisposition { Accepted, Duplicate, Retryable, Conflict, Rejected }

/// Only documented code/message pairs count as duplicate success. Queued downloads
/// are not acceptance; unknown wording must never turn into success.
pub fn classify_submit(code: i64, message: &str) -> SubmitDisposition {
    use SubmitDisposition::*;
    if code == 0 { return Accepted; }
    if matches!(code, -28 | -342) { return Retryable; }
    let reason = message.strip_prefix("failed to validate tx: WtxId(\"private\"), error: ").unwrap_or(message);
    if code == -25 {
        if let Some(nf) = reason.strip_prefix("transaction did not pass consensus validation: could not contextually validate transaction on best chain: ironwood double-spend: duplicate nullifier: Nullifier(Nullifier(0x") {
            if [")), in finalized state: true", ")), in finalized state: false"].iter()
                .filter_map(|suffix| nf.strip_suffix(suffix))
                .any(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())) {
                return Conflict;
            }
        }
    }
    match (code, reason) {
        (-1 | -25, "transaction already exists in mempool" | "transaction was committed to the best chain" | "transaction is already in state") => Duplicate,
        (-1, "any transaction with the same effects will be rejected from the mempool until a chain reset: transaction was committed to the best chain") => Duplicate,
        (-1 | -25, "transaction dropped because it is already queued for download"
            | "transaction dropped because the queue is full"
            | "mempool is disabled since synchronization is behind the chain tip") => Retryable,
        _ => Rejected,
    }
}

impl Rpc {
    pub fn new(url: &str) -> Self {
        Rpc {
            url: url.to_string(),
            agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(3))
                .timeout(Duration::from_secs(60))
                .build(),
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let resp = match self.agent.post(&self.url).send_json(body) {
            Ok(r) => r,
            // Zakura answers JSON-RPC errors with a non-2xx status and a JSON body.
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(anyhow!("rpc {method} to {}: {e}", self.url)),
        };
        let v: Value = resp.into_json()?;
        if !v["error"].is_null() {
            return Err(RpcError {
                code: v["error"]["code"].as_i64().unwrap_or(0),
                message: v["error"]["message"].as_str().unwrap_or("").to_string(),
            }
            .into());
        }
        Ok(v["result"].clone())
    }

    pub fn tip(&self) -> Result<u32> {
        Ok(self.call("getblockcount", json!([]))?.as_u64().ok_or_else(|| anyhow!("getblockcount"))? as u32)
    }

    pub fn seed(&self, height: u32) -> Result<crate::sync::Seed> {
        let tree = self.call("z_gettreestate", json!([height.to_string()]))?;
        let block = self.call("getblock", json!([height.to_string(), 1]))?;
        let frontier = tree["ironwood"]["commitments"]["finalState"].as_str().unwrap_or("000000");
        crate::wallet::check_frontier_size(frontier, block["trees"]["ironwood"]["size"].as_u64().unwrap_or(0))?;
        anyhow::ensure!(tree["hash"] == block["hash"], "chain changed while fetching frontier");
        Ok(crate::sync::Seed { height, hash: self.block_hash(height)?, frontier: frontier.into() })
    }

    /// Block hash (display hex) and the raw transactions of block `h`.
    pub fn block_txs(&self, h: u32) -> Result<(String, Vec<Vec<u8>>)> {
        let block = self.call("getblock", json!([h.to_string(), 1]))?;
        let hash = block["hash"].as_str().unwrap_or_default().to_string();
        let mut out = vec![];
        for txid in block["tx"].as_array().ok_or_else(|| anyhow!("getblock {h}: no tx list"))? {
            let hex_tx = self.call("getrawtransaction", json!([txid, 0]))?;
            out.push(hex::decode(hex_tx.as_str().ok_or_else(|| anyhow!("raw tx"))?)?);
        }
        Ok((hash, out))
    }

    pub fn block_hash(&self, h: u32) -> Result<String> {
        Ok(self.call("getblockhash", json!([h]))?.as_str().unwrap_or_default().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_or_behind_tip_is_not_success_and_unknown_already_is_not_a_duplicate() {
        use SubmitDisposition::*;
        for reason in ["transaction dropped because it is already queued for download", "transaction dropped because the queue is full",
            "mempool is disabled since synchronization is behind the chain tip"] {
            assert_eq!(classify_submit(-25, reason), Retryable);
            assert_eq!(classify_submit(-25, &format!("failed to validate tx: WtxId(\"private\"), error: {reason}")), Retryable);
        }
        assert_eq!(classify_submit(-25, "failed to validate tx: WtxId(\"private\"), error: transaction is already in state"), Duplicate);
        assert_eq!(classify_submit(-1, "transaction was committed to the best chain"), Duplicate);
        let cached_mined = "any transaction with the same effects will be rejected from the mempool until a chain reset: transaction was committed to the best chain";
        assert_eq!(classify_submit(-1, cached_mined), Duplicate);
        assert_eq!(classify_submit(-22, cached_mined), Rejected);
        assert_eq!(classify_submit(-1, &(cached_mined.to_string() + " extra")), Rejected);
        assert_eq!(classify_submit(-22, "transaction already exists in mempool"), Rejected);
        assert_eq!(classify_submit(-25, "already spent by a different transaction"), Rejected);
        assert_eq!(classify_submit(-25, "transaction is already in state: conflicting variant"), Rejected);
        let conflict = format!("failed to validate tx: WtxId(\"private\"), error: transaction did not pass consensus validation: could not contextually validate transaction on best chain: ironwood double-spend: duplicate nullifier: Nullifier(Nullifier(0x{})), in finalized state: false", "ab".repeat(32));
        assert_eq!(classify_submit(-25, &conflict), Conflict);
        assert_eq!(classify_submit(-22, &conflict), Rejected);
        assert_eq!(classify_submit(-25, &(conflict + " extra")), Rejected);
    }
}
