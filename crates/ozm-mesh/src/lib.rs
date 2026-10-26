//! BitChat-style mesh over TCP links: TTL flooding, MTU-sized fragments, dedup,
//! store-and-forward with inventory sync, and end-to-end delivery acks.
//!
//! Each node listens and/or dials its neighbours; the network (for example Docker networks)
//! decides who is a neighbour. See `README.md` for the wire format and semantics.

mod ids;
mod link;
mod node;
mod packet;
mod persist;

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{ensure, Result};
use serde::Serialize;
use tokio::{net::TcpListener, sync::mpsc};
use tracing::info;

pub use ids::{MsgId, PeerId};
use node::Node;
use packet::Kind;

#[derive(Clone, Debug)]
pub struct MeshConfig {
    pub peer_id: PeerId,
    /// TCP listen address, e.g. `0.0.0.0:7777`. `None` (the default) only dials.
    pub listen: Option<SocketAddr>,
    /// `host:port` neighbours to dial; DNS is resolved on every attempt and dialing never gives up.
    pub dial: Vec<String>,
    /// Maximum fragment payload per frame (default 512, BLE-like). At most 65,535.
    pub mtu: usize,
    /// TTL given to packets this node originates (default 7); the number of links a packet may cross.
    pub default_ttl: u8,
    /// Sleep after every data frame sent, to emulate BLE throughput (default 0).
    pub frame_delay: Duration,
    /// How long stored packets and seen ids are kept, counted from packet creation (default 24 h).
    pub store_ttl: Duration,
    /// JSON file for the store-and-forward cache and seen/acked ids; `None` keeps them in memory.
    pub store_path: Option<PathBuf>,
}

impl Default for MeshConfig {
    fn default() -> Self {
        MeshConfig {
            peer_id: PeerId::random(),
            listen: None,
            dial: Vec::new(),
            mtu: 512,
            default_ttl: 7,
            frame_delay: Duration::ZERO,
            store_ttl: Duration::from_secs(24 * 60 * 60),
            store_path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshEvent {
    /// A directed message for this node, or a broadcast. `hops` is the number of links it crossed
    /// (1 = received straight from the originator).
    Message { from: PeerId, msg_id: MsgId, payload: Vec<u8>, hops: u8, broadcast: bool },
    /// End-to-end ack for a directed message this node sent.
    Delivered { msg_id: MsgId, by: PeerId },
    NeighborUp(PeerId),
    NeighborDown(PeerId),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MeshStats {
    pub frames_sent: u64,
    pub bytes_sent: u64,
    pub frames_received: u64,
    pub bytes_received: u64,
    /// Packets this node forwarded for others (live flooding).
    pub packets_relayed: u64,
    /// Stored packets handed to a neighbour during inventory sync.
    pub packets_synced: u64,
    /// Directed messages and broadcasts delivered to this node.
    pub messages_delivered: u64,
    /// Packets currently held for store-and-forward (messages, broadcasts, acks).
    pub stored: usize,
    pub neighbors: usize,
}

/// Cloneable handle to a running mesh node.
#[derive(Clone)]
pub struct Mesh {
    node: Arc<Node>,
    listen_addr: Option<SocketAddr>,
}

impl Mesh {
    pub async fn start(cfg: MeshConfig) -> Result<(Mesh, mpsc::Receiver<MeshEvent>)> {
        ensure!((1..=u16::MAX as usize).contains(&cfg.mtu), "mtu must be 1..=65535");
        ensure!(cfg.default_ttl >= 1, "default_ttl must be at least 1");
        let db = match &cfg.store_path {
            Some(path) => persist::load(path)?,
            None => persist::Db::default(),
        };
        let listener = match cfg.listen {
            Some(addr) => Some(TcpListener::bind(addr).await?),
            None => None,
        };
        let listen_addr = listener.as_ref().map(|l| l.local_addr()).transpose()?;

        // Unbounded internally so the node never blocks; a forwarder feeds the bounded receiver.
        let (utx, mut urx) = mpsc::unbounded_channel();
        let (tx, rx) = mpsc::channel(1024);
        tokio::spawn(async move {
            while let Some(ev) = urx.recv().await {
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
        });

        let dial = cfg.dial.clone();
        let node = Arc::new(Node::new(cfg, db, utx));
        info!(peer = %node.me, listen = ?listen_addr, ?dial, "mesh started");
        if let Some(l) = listener {
            node.spawn(link::listen(l, node.clone()));
        }
        for target in dial {
            node.spawn(link::dial_forever(target, node.clone()));
        }
        let n = node.clone();
        node.spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(10));
            loop {
                tick.tick().await;
                n.sweep();
            }
        });
        Ok((Mesh { node, listen_addr }, rx))
    }

    /// Sends a directed message; it is flooded, stored for later neighbours, and acked by `to`.
    pub async fn send(&self, to: PeerId, payload: Vec<u8>) -> Result<MsgId> {
        ensure!(to != self.node.me, "cannot send to ourselves");
        ensure!(to != PeerId::default(), "the zero PeerId is reserved for broadcasts");
        self.node.originate(Kind::Message, to, payload)
    }

    pub async fn broadcast(&self, payload: Vec<u8>) -> Result<MsgId> {
        self.node.originate(Kind::Broadcast, PeerId::default(), payload)
    }

    /// Currently connected neighbours, sorted.
    pub fn neighbors(&self) -> Vec<PeerId> {
        self.node.neighbors()
    }

    pub fn peer_id(&self) -> PeerId {
        self.node.me
    }

    pub fn stats(&self) -> MeshStats {
        self.node.stats()
    }

    /// The bound listen address (useful with port 0).
    pub fn listen_addr(&self) -> Option<SocketAddr> {
        self.listen_addr
    }

    /// Stops listening, dialing and all links, persists the store, and closes the event stream.
    pub fn shutdown(&self) {
        self.node.shutdown();
    }
}
