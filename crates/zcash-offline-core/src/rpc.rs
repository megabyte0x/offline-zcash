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
