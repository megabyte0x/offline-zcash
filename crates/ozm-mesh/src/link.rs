//! The link layer: framing, hello, fragmentation and keepalive over any byte stream.
//!
//! `run_link` works on any `AsyncRead + AsyncWrite` stream and talks to the node only through
//! `Node::link_up`, `Node::on_link_msg` and `Node::link_down`. The TCP listener and dialer below are
//! the only transport today; LAN discovery (`discovery.rs`) finds peers and dials them with
//! `dial_once`. A BLE link would add its own way to obtain a stream.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time,
};
use tracing::{debug, warn};

use crate::{
    node::{Counters, Node},
    packet::{LinkMsg, VERSION},
    PeerId,
};

const FRAME_HELLO: u8 = 1;
const FRAME_DATA: u8 = 2;
const FRAME_PING: u8 = 3;
const HELLO_MAGIC: &[u8; 3] = b"OZM";
/// Data frame body before the chunk: kind, fragment id, index, count.
const DATA_HEADER: usize = 1 + 16 + 2 + 2;
/// Largest frame body accepted from a peer (a peer may use a larger mtu than ours, up to u16::MAX).
const MAX_FRAME_BODY: usize = DATA_HEADER + u16::MAX as usize;
pub(crate) const MAX_FRAGMENTS: usize = 4096;

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const PING_INTERVAL: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(20);
const REASSEMBLY_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_BACKOFF: Duration = Duration::from_millis(100);
const MAX_BACKOFF: Duration = Duration::from_secs(3);

/// One link message queued for a link; `frag_id` is the packet's msg_id (or random).
pub(crate) struct Outgoing {
    pub frag_id: [u8; 16],
    pub body: Arc<Vec<u8>>,
}

pub(crate) type LinkTx = mpsc::UnboundedSender<Outgoing>;

pub(crate) async fn listen(listener: TcpListener, node: Arc<Node>) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                debug!(%addr, "inbound connection");
                let n = node.clone();
                node.spawn(async move {
                    stream.set_nodelay(true).ok();
                    run_link(stream, false, &n).await;
                });
            }
            Err(e) => {
                warn!(error = %e, "accept failed");
                time::sleep(MIN_BACKOFF).await;
            }
        }
    }
}

/// Dials `target` forever (DNS resolved on every attempt), backing off up to `MAX_BACKOFF`.
/// While the peer behind `target` is connected over another link, it waits instead of redialing.
pub(crate) async fn dial_forever(target: String, node: Arc<Node>) {
    let mut backoff = MIN_BACKOFF;
    let mut last_peer = None;
    loop {
        if last_peer.is_some_and(|p| node.is_connected(p)) {
            time::sleep(Duration::from_millis(500)).await;
            continue;
        }
        if let Some(out) = dial_once(&target, &node).await {
            last_peer = out.peer.or(last_peer);
            if out.accepted {
                backoff = MIN_BACKOFF;
            }
        }
        time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// One dial attempt; runs the link until it ends. `None` if the connection failed.
pub(crate) async fn dial_once(target: &str, node: &Arc<Node>) -> Option<Outcome> {
    match time::timeout(CONNECT_TIMEOUT, TcpStream::connect(target)).await {
        Ok(Ok(stream)) => {
            stream.set_nodelay(true).ok();
            Some(run_link(stream, true, node).await)
        }
        Ok(Err(e)) => {
            debug!(%target, error = %e, "dial failed");
            None
        }
        Err(_) => {
            debug!(%target, "dial timed out");
            None
        }
    }
}

pub(crate) struct Outcome {
    pub peer: Option<PeerId>,
    pub accepted: bool,
}

/// Runs one link to completion: hello, registration, then read and write loops until either ends.
pub(crate) async fn run_link<S>(stream: S, outbound: bool, node: &Arc<Node>) -> Outcome
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let (mut rd, mut wr) = tokio::io::split(stream);
    let peer = match handshake(&mut rd, &mut wr, node).await {
        Ok(p) => p,
        Err(e) => {
            debug!(error = %e, "handshake failed");
            return Outcome { peer: None, accepted: false };
        }
    };
    if peer == node.me {
        debug!("connected to ourselves, dropping link");
        return Outcome { peer: None, accepted: false };
    }
    let (tx, rx) = mpsc::unbounded_channel();
    let Some(link_id) = node.link_up(peer, outbound, tx) else {
        return Outcome { peer: Some(peer), accepted: false };
    };
    let res = tokio::select! {
        r = read_loop(&mut rd, peer, node) => r,
        r = write_loop(&mut wr, rx, node) => r,
    };
    node.link_down(peer, link_id, res.err());
    Outcome { peer: Some(peer), accepted: true }
}

async fn handshake<R, W>(rd: &mut R, wr: &mut W, node: &Node) -> Result<PeerId>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut hello = vec![FRAME_HELLO];
    hello.extend_from_slice(HELLO_MAGIC);
    hello.push(VERSION);
    hello.extend_from_slice(&node.me.0);
    write_frame(wr, &hello, &node.stats).await?;
    let body = time::timeout(HELLO_TIMEOUT, read_frame(rd, &node.stats))
        .await
        .context("hello timed out")??
        .context("closed before hello")?;
    ensure!(body.len() == 13 && body[0] == FRAME_HELLO && &body[1..4] == HELLO_MAGIC, "bad hello");
    ensure!(body[4] == VERSION, "unsupported mesh version {}", body[4]);
    Ok(PeerId(body[5..13].try_into().expect("length checked")))
}

async fn write_frame<W: AsyncWrite + Unpin>(wr: &mut W, body: &[u8], stats: &Counters) -> Result<()> {
    let mut buf = Vec::with_capacity(4 + body.len());
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body);
    wr.write_all(&buf).await?;
    stats.frame_sent(buf.len());
    Ok(())
}

/// `Ok(None)` on a clean EOF at a frame boundary.
async fn read_frame<R: AsyncRead + Unpin>(rd: &mut R, stats: &Counters) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match rd.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let n = u32::from_be_bytes(len) as usize;
    ensure!(n > 0 && n <= MAX_FRAME_BODY, "bad frame length {n}");
    let mut body = vec![0u8; n];
    rd.read_exact(&mut body).await?;
    stats.frame_received(4 + n);
    Ok(Some(body))
}

async fn write_loop<W: AsyncWrite + Unpin>(
    wr: &mut W,
    mut rx: mpsc::UnboundedReceiver<Outgoing>,
    node: &Node,
) -> Result<()> {
    let mut ping = time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    ping.tick().await;
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(out) => send_fragmented(wr, &out, node).await?,
                // The node dropped this link (duplicate replaced, or shutdown).
                None => return Ok(()),
            },
            _ = ping.tick() => write_frame(wr, &[FRAME_PING], &node.stats).await?,
        }
    }
}

async fn send_fragmented<W: AsyncWrite + Unpin>(wr: &mut W, out: &Outgoing, node: &Node) -> Result<()> {
    let mtu = node.cfg.mtu;
    let count = out.body.len().div_ceil(mtu).max(1);
    ensure!(count <= MAX_FRAGMENTS, "message needs {count} fragments");
    for (i, chunk) in out.body.chunks(mtu).enumerate() {
        let mut f = Vec::with_capacity(DATA_HEADER + chunk.len());
        f.push(FRAME_DATA);
        f.extend_from_slice(&out.frag_id);
        f.extend_from_slice(&(i as u16).to_be_bytes());
        f.extend_from_slice(&(count as u16).to_be_bytes());
        f.extend_from_slice(chunk);
        write_frame(wr, &f, &node.stats).await?;
        if !node.cfg.frame_delay.is_zero() {
            time::sleep(node.cfg.frame_delay).await;
        }
    }
    Ok(())
}

async fn read_loop<R: AsyncRead + Unpin>(rd: &mut R, peer: PeerId, node: &Node) -> Result<()> {
    let mut partial = HashMap::new();
    loop {
        let body = match time::timeout(IDLE_TIMEOUT, read_frame(rd, &node.stats)).await {
            Err(_) => bail!("no traffic for {IDLE_TIMEOUT:?}"),
            Ok(r) => match r? {
                Some(b) => b,
                None => return Ok(()),
            },
        };
        match body[0] {
            FRAME_PING => {}
            FRAME_DATA => {
                if let Some(msg) = reassemble(&mut partial, &body[1..])? {
                    match LinkMsg::decode(&msg) {
                        Ok(m) => node.on_link_msg(peer, m),
                        Err(e) => warn!(%peer, error = %e, "undecodable link message"),
                    }
                }
            }
            other => bail!("unexpected frame kind {other}"),
        }
    }
}

struct Partial {
    count: usize,
    have: usize,
    parts: Vec<Option<Vec<u8>>>,
    started: Instant,
}

fn reassemble(partial: &mut HashMap<[u8; 16], Partial>, b: &[u8]) -> Result<Option<Vec<u8>>> {
    ensure!(b.len() >= DATA_HEADER - 1, "short data frame");
    let id: [u8; 16] = b[..16].try_into().expect("length checked");
    let idx = u16::from_be_bytes([b[16], b[17]]) as usize;
    let count = u16::from_be_bytes([b[18], b[19]]) as usize;
    let chunk = &b[20..];
    ensure!(count > 0 && idx < count && count <= MAX_FRAGMENTS, "bad fragment header {idx}/{count}");
    partial.retain(|id, p| {
        let keep = p.started.elapsed() < REASSEMBLY_TIMEOUT;
        if !keep {
            debug!(frag_id = %hex::encode(id), have = p.have, count = p.count, "dropping incomplete reassembly");
        }
        keep
    });
    if count == 1 {
        return Ok(Some(chunk.to_vec()));
    }
    let new = || Partial { count, have: 0, parts: vec![None; count], started: Instant::now() };
    let p = partial.entry(id).or_insert_with(new);
    if p.count != count {
        *p = new();
    }
    if p.parts[idx].is_none() {
        p.have += 1;
    }
    p.parts[idx] = Some(chunk.to_vec());
    if p.have < p.count {
        return Ok(None);
    }
    let p = partial.remove(&id).expect("present");
    Ok(Some(p.parts.into_iter().flatten().flatten().collect()))
}
