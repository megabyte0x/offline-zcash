# ozm-mesh

BitChat-style mesh semantics over TCP links between neighbours: TTL flooding, MTU-sized
fragments, dedup, store-and-forward with inventory sync, and end-to-end delivery acks. Who is a
neighbour is decided by the network (for example Docker networks forming a line A–R–B–D). There is
no encryption at this layer; the application seals payloads.

```rust
let (mesh, mut events) = Mesh::start(MeshConfig {
    listen: Some("0.0.0.0:7777".parse()?),
    dial: vec!["relay:7777".into()],
    store_path: Some("/data/mesh.json".into()),
    // Optional: find same-fleet nodes on the LAN without `dial`.
    discovery: Some(Discovery { tag: *b"fleet-01", ..Discovery::default() }),
    ..MeshConfig::default()
}).await?;
let id = mesh.send(gateway, sealed_tx).await?;
while let Some(ev) = events.recv().await { /* Message, Delivered, NeighborUp, NeighborDown */ }
```

## Wire format

All integers are big-endian.

**Stream frame (TCP or BLE):** `len: u32` then `len` bytes of body. A body is never larger than `mtu + 21`.

| Body kind | Layout |
|---|---|
| Hello `0x01` | `"OZM"`, `version: u8` (= 1), `peer_id: [u8; 8]` — the first frame each side sends |
| Data `0x02` | `frag_id: [u8; 16]`, `index: u16`, `count: u16`, `chunk` (≤ mtu bytes) |
| Ping `0x03` | empty; TCP 5 s ping / 20 s idle timeout; BLE 15 s / 60 s |

Data chunks are joined in index order into one **link message**. `frag_id` is the packet's msg_id
(random for inventories). Incomplete reassemblies are dropped after 30 s; at most 4,096 fragments.

**Link message:** first byte `0x10` = packet, `0x11` = inventory.

- Inventory: `n: u32`, `n × msg_id` (ids already seen), `m: u32`, `m × msg_id` (message ids
  known to be acked).
- Packet (48-byte header + payload):

| Field | Size |
|---|---|
| version (= 1) | 1 |
| type: 1 Message, 2 Broadcast, 3 Ack | 1 |
| ttl: links this copy may still cross | 1 |
| hops: links this copy has crossed on arrival (1 = straight from the originator) | 1 |
| msg_id | 16 |
| sender PeerId | 8 |
| recipient PeerId (zero for broadcast) | 8 |
| created, unix ms | 8 |
| payload length `u32` | 4 |
| payload | n |

An Ack has its own random msg_id and is addressed to the original sender. With
`MeshConfig::ack_signer` (mandatory in `ozm-node` v0.6), its payload is the original
msg_id (16), BLAKE2b-256 of the delivered sealed payload (32), destination Ed25519
key (32), and signature (64). The signature covers the domain `ozm1-mesh-ack-v1`
and encoded ack with TTL/hops zeroed and signature omitted. The key must hash to
the destination's peer ID; origin/destination and payload hash must match the
queued message before it can be removed or confirmed. Forged/unsigned acks are
rejected. `ack_signer: None` retains the old 16-byte ack for transport-only legacy
tests; wallet nodes never use that mode. v0.5.x nodes drop new acks without crashing.

## Semantics

- **One link per neighbour.** Both sides send Hello. If a second link to the same peer comes up,
  a reconnect by the same dialer replaces the old link; for a simultaneous dial, the connection
  dialed by the smaller PeerId wins on both ends. Dial targets are resolved through DNS on every
  attempt and retried forever, with backoff from 100 ms up to 3 s. A dialer waits while its peer
  is already connected over another link.
- **Flooding.** Each msg_id is processed once (seen set). A Message for this node is delivered
  once as `MeshEvent::Message`, and an Ack is flooded with the default TTL. Broadcasts are
  delivered and forwarded. Anything else is forwarded to every neighbour except the one it came
  from, as long as `ttl > 1`; the forwarded copy has `ttl - 1` and `hops + 1`. `MeshEvent::Message.hops`
  is the hops field on arrival.
- **Store-and-forward.** Every packet a node originates or relays (messages, broadcasts and acks)
  is kept as its next-hop copy until `created + store_ttl`. A Message is dropped from the store
  once an Ack for it is seen. When a link comes up, each side sends an inventory and then sends
  the other side every stored packet it lacks. Stored copies already carry the decremented ttl.
  The sender gets `MeshEvent::Delivered` the first time it sees the Ack for its own message.
- **Persistence.** If `store_path` is set, the store, seen ids and acked ids are written as JSON
  (temp file, fsync, rename) after every change and on `shutdown`. A restarted relay keeps
  forwarding, and a restarted recipient does not deliver again.

## LAN discovery

Off by default. With `MeshConfig.discovery = Some(Discovery { port, tag, interval, advertise_port })`
(defaults: UDP port 7778, tag all zero, interval 2 s, advertise the bound listen port) nodes of
one fleet on the same Wi-Fi or LAN find and link to each other without `dial`.

**Beacon** (one UDP datagram, 23 bytes, sent every `interval`):

| Field | Size |
|---|---|
| `"OZMD"` | 4 |
| version (= 1) | 1 |
| fleet tag | 8 |
| sender PeerId | 8 |
| TCP port to dial, `u16` (0 = the node does not listen) | 2 |

Receivers ignore anything shorter or with another magic/version. Nonzero tags
match the same fleet; a zero tag on either side enables open discovery. Wallet
gateways advertise zero so both open and invited devices can find them. Tags are
discovery filters, not authentication. Extra bytes after the 23 are ignored so a
later version can append fields.

- **Interfaces.** If the TCP listen address is a specific IPv4 address, beacons use only that
  address. Otherwise every up IPv4 interface is used (including self-assigned 169.254.x.x),
  except loopback, which is used only when there is nothing else. Interfaces are listed again
  every 10 s, so a Wi-Fi joined later is picked up and a vanished one is dropped.
- **Sending.** Each interface gets its own socket bound to its address, with `SO_BROADCAST`,
  `IP_MULTICAST_IF` set to that address, multicast loop on and TTL 1. Every beacon goes to that
  interface's directed broadcast (`ip | !netmask`, e.g. `192.168.1.255`; none for /31 and /32),
  the multicast group `239.255.77.77:port`, and the limited broadcast `255.255.255.255:port`. No
  default route is needed (an offline Wi-Fi LAN or a Docker `--internal` network has none), and
  a multi-homed node beacons on every network. The source address of each beacon is the
  interface address, which is what peers dial.
- **Receiving.** One socket bound to `0.0.0.0:port` with `SO_REUSEADDR` (and `SO_REUSEPORT` on
  unix), so several nodes on one host share the port. It joins the group on each interface
  address (not `INADDR_ANY`, which fails without a default route).
- **Dialing.** A beacon from a same-tag PeerId that is not ours, not a connected neighbour and not
  already being dialed by discovery is dialed once at `source_ip:advertised_port`, through the same
  dial code as `dial` (`link::dial_once`). At most one discovery dial per peer per 2 s. While that
  link lives the peer counts as being dialed; once it drops, a later beacon dials again. When both
  sides discover each other at once, the duplicate-link rule above keeps one link per pair.
- **Visibility.** `Mesh::discovered()` lists `(PeerId, dial address)` for peers heard in the last
  3 intervals, and `MeshStats.discovered` counts them.
- **Failures never stop the node.** A failed interface listing, socket, group join or beacon
  send is logged as a warning at most once per target per minute (debug in between, info when it
  works again) and retried; the other destinations keep working. If the receive socket cannot
  be bound the node only beacons. A node ignores its own beacons by PeerId; beacons from another
  node on the same host (same source IP, other PeerId) are used, so several nodes can share a
  laptop.

Platform caveats:

- Checked with four containers on a Docker `--internal` network (no default route), with one node
  also on a second bridge: every node linked to the other three, and a restarted node relinked.
  Which of the three destinations carried the beacons there was not measured; multicast on a
  bridge depends on its IGMP snooping. Beacons are not routed between networks, and Docker
  Desktop on macOS runs containers in a VM that does not hear the LAN; use `dial` across those
  boundaries.
- Some Wi-Fi access points (guest networks, "client isolation" or "AP isolation") block all
  traffic between clients; discovery and TCP links both fail there.
- macOS asks the user to allow "local network" access the first time a binary sends to the LAN;
  until the user allows it, sends fail with a rate-limited warning and the node keeps running.
- The tests run three nodes on `127.0.0.1` with one shared UDP port: loopback is the listen
  address, so beacons go to `127.255.255.255`, to the group with `IP_MULTICAST_IF 127.0.0.1`
  and to `255.255.255.255` from a loopback-bound socket. All three reach the other nodes on
  Linux. On macOS, `lo0` supports multicast, so the group path should work there; that has not
  been run on macOS yet.

## Link layer boundary

`src/link.rs` runs a link over any `AsyncRead + AsyncWrite` stream (`run_link`) and talks to the
mesh only through `Node::link_up`, `on_link_msg` and `link_down`. TCP listen and dial retain their
5 s ping / 20 s idle policy. Public `Mesh::attach(stream, outbound)` runs an externally supplied
BLE stream to completion with 15 s / 60 s timers; `outbound` means central/dialer. EOF, framing
failure, duplicate rejection, task cancellation and shutdown close the link. Reassembly stays
at 30 s. `Mesh::neighbor_links()` returns sorted `(PeerId, LinkKind)` pairs; `neighbors()` is unchanged.

`../ozm-ffi` bridges native callbacks to bounded Tokio duplex streams. Android implements GATT
write-without-response/notify chunking and local callback flow control; routing is unchanged.
See [Android guide](../../docs/android-install.md) and [mesh design](../../docs/components/mesh.md#ble-link-android-first).
