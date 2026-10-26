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
    ..MeshConfig::default()
}).await?;
let id = mesh.send(gateway, sealed_tx).await?;
while let Some(ev) = events.recv().await { /* Message, Delivered, NeighborUp, NeighborDown */ }
```

## Wire format

All integers are big-endian.

**TCP frame:** `len: u32` then `len` bytes of body. A body is never larger than `mtu + 21`.

| Body kind | Layout |
|---|---|
| Hello `0x01` | `"OZM"`, `version: u8` (= 1), `peer_id: [u8; 8]` — the first frame each side sends |
| Data `0x02` | `frag_id: [u8; 16]`, `index: u16`, `count: u16`, `chunk` (≤ mtu bytes) |
| Ping `0x03` | empty; sent every 5 s; a link with no frames for 20 s is dead |

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

An Ack has its own random msg_id, is addressed to the original sender, and its payload is the
16-byte msg_id it acknowledges.

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

## Link layer boundary

`src/link.rs` runs a link over any `AsyncRead + AsyncWrite` stream (`run_link`) and talks to the
mesh only through `Node::link_up`, `on_link_msg` and `link_down`. TCP listen and dial are the only
transports today. A LAN-discovery or BLE link only needs to produce a stream (or call those three
methods).
