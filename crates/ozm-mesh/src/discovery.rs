//! LAN peer discovery: UDP beacons from every IPv4 interface to its directed broadcast address,
//! the multicast group and the limited broadcast address, and one dial (through `link::dial_once`) to every same-fleet peer that is heard and not linked.
//!
//! Beacon (23 bytes): `"OZMD"`, `version: u8` (= 1), `tag: [u8; 8]`, `peer_id: [u8; 8]`,
//! `tcp_port: u16` (big-endian; 0 = this node does not listen). Longer beacons are accepted so a
//! later version can append fields.

use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use socket2::{Domain, Protocol, SockAddr, SockRef, Socket, Type};
use tokio::{net::UdpSocket, time};
use tracing::{debug, info, warn};

use crate::{
    link,
    node::{lock, Node},
    Discovery, PeerId,
};

pub(crate) const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 77, 77);
const MAGIC: &[u8; 4] = b"OZMD";
const VERSION: u8 = 1;
const BEACON_LEN: usize = 4 + 1 + 8 + 8 + 2;
/// At most one discovery dial per peer in this period.
const REDIAL: Duration = Duration::from_secs(2);
/// Interfaces are listed again this often, so a Wi-Fi joined later is used.
const REENUMERATE: Duration = Duration::from_secs(10);
/// A failing send or join is warned about at most this often.
const WARN_EVERY: Duration = Duration::from_secs(60);

struct Beacon {
    tag: [u8; 8],
    peer: PeerId,
    port: u16,
}

impl Beacon {
    fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(BEACON_LEN);
        b.extend_from_slice(MAGIC);
        b.push(VERSION);
        b.extend_from_slice(&self.tag);
        b.extend_from_slice(&self.peer.0);
        b.extend_from_slice(&self.port.to_be_bytes());
        b
    }

    fn decode(b: &[u8]) -> Option<Beacon> {
        if b.len() < BEACON_LEN || &b[..4] != MAGIC || b[4] != VERSION {
            return None;
        }
        Some(Beacon {
            tag: b[5..13].try_into().ok()?,
            peer: PeerId(b[13..21].try_into().ok()?),
            port: u16::from_be_bytes([b[21], b[22]]),
        })
    }
}

/// Starts beaconing and listening. Socket errors are logged and never stop the node.
/// `listen_ip` is the bound TCP listen IP: if it is a specific IPv4 address, beacons are sent only
/// from it (so their source address is the one peers must dial); otherwise from every interface.
pub(crate) fn start(d: Discovery, tcp_port: u16, listen_ip: Option<IpAddr>, node: Arc<Node>) {
    let local = match listen_ip {
        Some(IpAddr::V4(ip)) if !ip.is_unspecified() => Some(ip),
        _ => None,
    };
    info!(udp_port = d.port, tag = %hex::encode(d.tag), tcp_port, ?local, "lan discovery on");
    let recv = match recv_socket(d.port) {
        Ok(sock) => {
            let sock = Arc::new(sock);
            node.spawn(recv_loop(sock.clone(), d.tag, node.clone()));
            Some(sock)
        }
        Err(e) => {
            warn!(port = d.port, error = %e, "discovery: cannot listen for beacons; only beaconing");
            None
        }
    };
    let beacon = Beacon { tag: d.tag, peer: node.me, port: tcp_port }.encode();
    node.spawn(beacon_loop(local, recv, d.port, d.interval, beacon));
}

/// An IPv4 interface address beacons are sent from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Iface {
    ip: Ipv4Addr,
    /// Directed broadcast (`ip | !netmask`); none for /31 and /32.
    broadcast: Option<Ipv4Addr>,
}

/// The interfaces to beacon on: only `local` if set, else every up IPv4 interface except loopback,
/// falling back to loopback when there is nothing else.
fn interfaces(local: Option<Ipv4Addr>) -> io::Result<Vec<Iface>> {
    use if_addrs::{IfAddr, IfOperStatus::*};
    let mut all: Vec<(Iface, bool)> = Vec::new();
    for i in if_addrs::get_if_addrs()? {
        let IfAddr::V4(a) = i.addr else { continue };
        if matches!(i.oper_status, Down | NotPresent | LowerLayerDown) {
            continue;
        }
        let broadcast = (a.prefixlen < 31).then(|| Ipv4Addr::from(u32::from(a.ip) | !u32::from(a.netmask)));
        let iface = Iface { ip: a.ip, broadcast };
        if !all.iter().any(|(x, _)| x.ip == iface.ip) {
            all.push((iface, a.ip.is_loopback()));
        }
    }
    if let Some(ip) = local {
        let found = all.iter().find(|(i, _)| i.ip == ip).map(|(i, _)| *i);
        return Ok(vec![found.unwrap_or(Iface { ip, broadcast: None })]);
    }
    let lan: Vec<Iface> = all.iter().filter(|(_, lo)| !lo).map(|(i, _)| *i).collect();
    Ok(if lan.is_empty() { all.into_iter().map(|(i, _)| i).collect() } else { lan })
}

/// A socket bound to `ip` that sends broadcasts and multicast out of that interface.
fn send_socket(ip: Ipv4Addr) -> io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_broadcast(true)?;
    s.set_multicast_loop_v4(true)?;
    s.set_multicast_ttl_v4(1)?;
    s.set_multicast_if_v4(&ip)?;
    s.bind(&SockAddr::from(SocketAddrV4::new(ip, 0)))?;
    s.set_nonblocking(true)?;
    UdpSocket::from_std(s.into())
}

fn recv_socket(port: u16) -> io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(unix)]
    s.set_reuse_port(true)?;
    s.bind(&SockAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)))?;
    s.set_nonblocking(true)?;
    UdpSocket::from_std(s.into())
}

/// Warns once per key per minute; logs at debug level in between.
#[derive(Default)]
struct Warnings(HashMap<String, Instant>);

impl Warnings {
    fn fail(&mut self, key: String, what: &str, e: &io::Error) {
        let now = Instant::now();
        match self.0.get(&key) {
            Some(t) if now.duration_since(*t) < WARN_EVERY => debug!(on = %key, error = %e, "discovery: {what}"),
            _ => {
                warn!(on = %key, error = %e, "discovery: {what}; will keep trying");
                self.0.insert(key, now);
            }
        }
    }

    fn ok(&mut self, key: &str) {
        if self.0.remove(key).is_some() {
            info!(on = %key, "discovery: works again");
        }
    }
}

/// Every `interval`: beacon from each interface to its directed broadcast, the multicast group and
/// the limited broadcast. Every `REENUMERATE`: refresh interfaces, sockets and group memberships.
async fn beacon_loop(
    local: Option<Ipv4Addr>,
    recv: Option<Arc<UdpSocket>>,
    port: u16,
    interval: Duration,
    beacon: Vec<u8>,
) {
    let mut warns = Warnings::default();
    let mut socks: HashMap<Ipv4Addr, (Iface, UdpSocket)> = HashMap::new();
    let mut joined: HashSet<Ipv4Addr> = HashSet::new();
    let mut enumerated: Option<Instant> = None;
    let mut tick = time::interval(interval);
    tick.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if enumerated.is_none_or(|t| t.elapsed() >= REENUMERATE) {
            enumerated = Some(Instant::now());
            refresh(local, recv.as_deref(), &mut socks, &mut joined, &mut warns);
        }
        for (ip, (iface, sock)) in &socks {
            let dests = iface.broadcast.into_iter().chain([GROUP, Ipv4Addr::BROADCAST]);
            for dst in dests {
                let to = SocketAddr::from((dst, port));
                let key = format!("{ip}->{to}");
                match sock.send_to(&beacon, to).await {
                    Ok(_) => warns.ok(&key),
                    Err(e) => warns.fail(key, "beacon send failed", &e),
                }
            }
        }
    }
}

fn refresh(
    local: Option<Ipv4Addr>,
    recv: Option<&UdpSocket>,
    socks: &mut HashMap<Ipv4Addr, (Iface, UdpSocket)>,
    joined: &mut HashSet<Ipv4Addr>,
    warns: &mut Warnings,
) {
    let ifaces = match interfaces(local) {
        Ok(v) => {
            warns.ok("interfaces");
            v
        }
        Err(e) => return warns.fail("interfaces".into(), "cannot list network interfaces", &e),
    };
    let gone: Vec<Ipv4Addr> = socks.keys().filter(|ip| !ifaces.iter().any(|i| i.ip == **ip)).copied().collect();
    for ip in gone {
        info!(%ip, "discovery: interface gone");
        socks.remove(&ip);
        if joined.remove(&ip) {
            if let Some(r) = recv {
                let _ = SockRef::from(r).leave_multicast_v4(&GROUP, &ip);
            }
        }
    }
    for iface in ifaces {
        if socks.get(&iface.ip).is_none_or(|(old, _)| *old != iface) {
            match send_socket(iface.ip) {
                Ok(s) => {
                    info!(ip = %iface.ip, broadcast = ?iface.broadcast, "discovery: beaconing on interface");
                    warns.ok(&format!("socket {}", iface.ip));
                    socks.insert(iface.ip, (iface, s));
                }
                Err(e) => warns.fail(format!("socket {}", iface.ip), "cannot open beacon socket", &e),
            }
        }
        if let (Some(r), false) = (recv, joined.contains(&iface.ip)) {
            let key = format!("join {GROUP} on {}", iface.ip);
            match SockRef::from(r).join_multicast_v4(&GROUP, &iface.ip) {
                // Already a member (for example a second address on the same interface).
                Err(e) if e.kind() != io::ErrorKind::AddrInUse => {
                    warns.fail(key, "cannot join multicast group; relying on broadcast", &e)
                }
                _ => {
                    warns.ok(&key);
                    joined.insert(iface.ip);
                }
            }
        }
    }
}

#[derive(Default)]
struct Dials {
    /// Peers with a discovery dial in progress (including the link it opened, while it lasts).
    active: HashSet<PeerId>,
    last: HashMap<PeerId, Instant>,
}

async fn recv_loop(sock: Arc<UdpSocket>, tag: [u8; 8], node: Arc<Node>) {
    let dials = Arc::new(Mutex::new(Dials::default()));
    let mut buf = [0u8; 256];
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(e) => {
                debug!(error = %e, "discovery: receive failed");
                time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Some(b) = Beacon::decode(&buf[..n]) else { continue };
        // Zero is open discovery. Fleet tags remain filters, never authentication.
        if (b.tag != tag && b.tag != [0; 8] && tag != [0; 8]) || b.peer == node.me {
            continue;
        }
        let addr = SocketAddr::new(from.ip(), b.port);
        node.heard(b.peer, addr);
        if b.port == 0 || node.is_connected(b.peer) {
            continue;
        }
        {
            let mut d = lock(&dials);
            if d.active.contains(&b.peer) || d.last.get(&b.peer).is_some_and(|t| t.elapsed() < REDIAL) {
                continue;
            }
            d.last.retain(|_, t| t.elapsed() < REDIAL);
            if d.active.len() >= 32 || d.last.len() >= 256 { continue; }
            d.last.insert(b.peer, Instant::now());
            d.active.insert(b.peer);
        }
        info!(peer = %b.peer, %addr, "discovered peer, dialing");
        let (n, dials) = (node.clone(), dials.clone());
        node.spawn(async move {
            link::dial_once(&addr.to_string(), &n).await;
            lock(&dials).active.remove(&b.peer);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_round_trip() {
        let b = Beacon { tag: *b"fleet-01", peer: PeerId([7; 8]), port: 7777 }.encode();
        assert_eq!(b.len(), BEACON_LEN);
        assert_eq!(&b[..5], b"OZMD\x01");
        let d = Beacon::decode(&b).unwrap();
        assert_eq!((d.tag, d.peer, d.port), (*b"fleet-01", PeerId([7; 8]), 7777));
        assert!(Beacon::decode(&b[..BEACON_LEN - 1]).is_none());
        let mut wrong = b.clone();
        wrong[4] = 2;
        assert!(Beacon::decode(&wrong).is_none());
    }
}
