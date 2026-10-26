# Logos pathway: module designs (proposed)

Design for the Logos pathway on laptops ([logos.md](logos.md)). Modified Basecamp (or headless `logosctl`) nodes carry a Zcash offline module with its own UI. Payments hop node to node to a node with internet, which submits them and sends the result back. **Everything here is Proposed:** written 26 October 2026 and awaiting owner approval. Nothing is built; only the Offline Protocol transport was benched on its own. Formats have not been passed through `lidl check`.

Sources read: logos-rust-sdk `263d1c4`, logos-module-builder `8f56bf8` (`docs/configuration.md` L270–300), logos-lidl `5941b8f` (`docs/project.md` grammar), logos-chat-module `8e5b68d` (`rust-lib/chat_module.lidl`), logos-delivery-module `e2cb67a`, logos-libp2p-module `9e53e54`, xAlisher/peers `769fe2e`.

## Module map

```
┌──────────────────┐   ┌─────────────────────────┐   ┌───────────────────────┐
│ zcash_offline_ui │──▶│ zcash_offline_module    │──▶│ mesh_transport        │
│ (ui_qml)         │   │ (Rust cdylib, thin shell│   │ (interface contract)  │
└──────────────────┘   │  over zcash-offline-core)│   └──────────┬────────────┘
                       └─────────────────────────┘              │ bound at runtime
               ┌──────────────────────────┬──────────────────────┼────────────────────┐
               ▼                          ▼                      ▼
     ┌────────────────────┐   ┌─────────────────────┐  ┌──────────────────────┐
     │ mesh_delivery      │   │ mesh_libp2p         │  │ ble_mesh_module      │
     │ over delivery_module│  │ over libp2p_module  │  │ (our own BLE mesh)   │
     │ IP, Store          │   │ IP, no Store        │  │ no router needed     │
     └────────────────────┘   └─────────────────────┘  └──────────────────────┘
```

- **`zcash-offline-core`** is a plain Rust crate with no Logos code: builder, verifier, receipts, nullifier sharing, resend loop and catch-up, on the Zakura `=2.2.0` pins ([offline-builder.md](../components/offline-builder.md)). The BitChat pathway reuses it unchanged.
- **`zcash_offline_module`** is the Logos shell. It declares `mesh_transport` in `interface_dependencies` and binds a backend by name at runtime with `modules().bind_mesh_transport(name)` (logos-module-builder `docs/configuration.md` L282). Interface dependencies are "never loaded" by the host, so the chosen backend is listed in `optional_dependencies` to bring it up when installed.
- **One backend at a time** on the laptop bench; running two at once with a bridge is a later step (see [logos.md](logos.md) layer 4).

## Shared wire envelope `ozm1` (all backends)

The payment messages stay the ones already decided: `pay`, `rcpt`, `nf`, `evq`, `evr` ([transport-bitchat.md](../components/transport-bitchat.md), [pending-tier.md](../components/pending-tier.md)). On the Logos pathway they travel inside one binary envelope instead of BitChat private-message text.

| Field | Size | Notes |
|---|---|---|
| `version` | 1 | `0x01` |
| `kind` | 1 | `pay`, `rcpt`, `nf`, `evq`, `evr`, `sub` (payee → gateway), `res` (gateway result or chain bundle), `cuq`/`cur` (catch-up request and reply, BLE only) |
| `hop_limit` | 1 | Mutable; excluded from `envelope_id`. Relays clamp it to at most 8 |
| `envelope_id` | 32 | BLAKE2b-256, personalisation `ozm1-envelope`, over every other field except `hop_limit`. The dedup and loop key, checked on receipt |
| `body_len` | 4 | |
| `body` | n | Sealed or signed, see below |
| padding | to bucket | Zero bytes to 4 KiB multiples (a 2-action payment is 16 KiB) |

- **Confidentiality.** On Logos every node on the shared topic sees every envelope ([logos.md](logos.md) decided "sign-then-encrypt, fixed size, one shared topic"). Private kinds (`pay`, `rcpt`, `sub`, `evq`, `evr`) are sealed to the recipient: an ephemeral X25519 key plus ChaCha20-Poly1305, with no recipient hint, so each node tries to open each envelope (one X25519 per envelope). `nf` is sealed under the fleet-payee group key. `res` is public and signed with the gateway's enrollment payment key, as chain bundles already are.
- **[decided, owner 26 October 2026] The encryption key is derived, not enrolled.** Each device's X25519 key is converted from its enrolled Ed25519 payment key with the standard Ed25519 → Curve25519 map (libsodium `crypto_sign_ed25519_pk_to_curve25519` / `_sk_to_curve25519`), so the enrollment record does not change ([pending-tier.md](../components/pending-tier.md)). The sealing KDF uses its own personalisation (`ozm1-seal`) so encryption and signing outputs never share a domain. [unverified] the Rust crate choice on the Zakura pins (for example `curve25519-dalek` `MontgomeryPoint` conversion).
- **Dedup** is by `envelope_id` at the transport and by (txid, auth_digest) in the core, as decided.
- **No wall-clock dependency.** The envelope carries no timestamp the receiver must trust. Logos Store still needs node clocks within ±20 s, measured in [logos.md](logos.md), so the mesh still needs a local time source for Store.

## `mesh_transport` contract (draft `.lidl`)

```
; mesh_transport — the transport contract zcash_offline_module codes against.
; Payloads are ozm1 envelopes; backends never parse their bodies.
module mesh_transport {
  version "0.1.0"
  description "Topic publish/subscribe over a mesh, with optional history."
  category "network"

  type Peer {
    id: tstr
    ; "ip" or "ble"
    link: tstr
    ; true when the peer advertises that it has internet (gateway candidate)
    gateway: bool
    last_seen_ms: int
  }
  type Status {
    ; "stopped", "starting", "online", "offline" or "error"
    state: tstr
    detail: tstr
    backend: tstr
    peer_count: int
    ; false when the backend has no history (fetch_since then returns empty)
    has_history: bool
  }

  ; Backend-specific JSON, for example the private-cluster settings.
  method start(config: tstr) -> result
  method stop() -> result
  method status() -> Status
  method subscribe(topic: tstr) -> result
  method unsubscribe(topic: tstr) -> result
  ; Returns a request id; progress comes as send_state events.
  method publish(topic: tstr, envelope: bstr) -> result
  ; One page of history in [since_ms, until_ms); value is {"envelopes":[...],"cursor":"..."}.
  method fetch_since(topic: tstr, since_ms: int, until_ms: int, cursor: tstr) -> result
  method peers() -> [Peer]
  ; Re-adds a known peer after a partition (multiaddr, or a BLE id).
  method add_peer(address: tstr) -> result
  ; Sets the "has internet" bit this node advertises.
  method set_gateway(online: bool) -> result

  ; history is true for envelopes recovered from Store or a neighbour's cache.
  event received(topic: tstr, envelope: bstr, history: bool)
  ; state: "queued", "propagated", "stored" or "failed"
  event send_state(request_id: tstr, state: tstr, detail: tstr)
  event status_changed(state: tstr, detail: tstr, peer_count: int)
}
```

- [unverified] Whether events reach a client bound through an interface dependency. The SDK documents typed `on_<event>()` for dependencies (logos-rust-sdk README, "Typed events") but its interface example only shows method calls. Fallback: poll with a `next_received(timeout_ms)` method, as `libp2p_module`'s `gossipsubNextMessage` does.
- [unverified] How a provider module declares that it implements the contract (the doctest's `cpp_greeter_module` simply exposes the same methods).

## Backends

### `mesh_delivery` (first; works today)

Adapter module over `delivery_module`, with `dependencies: ["delivery_module"]`.

- `start`: `delivery_module.createNode` with no preset and `messagingOverrides`: `cluster-id`, `num-shards-in-network: 1`, `nodekey`, `entry-node` list, `nat: none`, `discv5-discovery: false`, `store` and `store-message-retention-policy` on Store nodes. If another module already created the node ("Context already initialized"), report `detail: adopted` with the node's real settings, as `chat_module` does.
- `publish` → `send`; `received` ← `messageReceived`; `send_state` ← `messagePropagated`, `messageSent` (stored), `messageError`.
- `fetch_since` → `storeQuery` with `contentTopics` only (adding `pubsubTopic` returns nothing, per Peers §10d), windows of at most 24 h, following `paginationCursor`. The core drives the windows from its last-seen time per topic.
- `add_peer`: `delivery_module` has no peer-add call (its methods: create, start, stop, send, subscribe, storeQuery, channels, node info). The adapter's only lever is `stop()` then `start()`, which restarts the node for every module sharing it. [unverified] whether that resets dial backoff and the 5-failure limit.
- Prerequisites on the mesh: DNS answering `one.one.one.one` on 1.1.1.1 (`dns-addrs-name-server` is not a `messagingOverrides` key) and a time source within ±20 s.

### `mesh_libp2p` (option; avoids logos-delivery's health monitor)

Adapter over `libp2p_module` (`gossipsubPublish`, `gossipsubSubscribe`, `gossipsubNextMessage`, `connectPeer`, `mountProtocol` plus streams; `src/plugin.h` L182–235).

- It has no online monitor, so the no-DNS reconnect blocker does not apply. It also never reconnects by itself: the adapter keeps a peer list and calls `connectPeer` on a timer, which is exactly what `add_peer` needs.
- No Store. History comes from a small request/response protocol over a stream (`/ozm1/catchup/1`): ask a neighbour for envelopes it holds since a time, the same exchange as BLE `cuq`/`cur`.
- Gossipsub over a sparse static mesh is untested; message size defaults to 1 MiB, ample for 16 KiB.

### `ble_mesh_module` (second; removes the need for routers)

Our own Bluetooth LE mesh, ported from Peers' design (MIT) and hardened.

- **Radio.** Each node is central and peripheral. Central: `btleplug` (central only, per its README). Peripheral: `ble-peripheral-rust` (MIT, 13 commits; young), with direct fallbacks `bluer` on Linux and `objc2-core-bluetooth` `CBPeripheralManager` on macOS. One fleet service UUID; at most 6 links (Peers `BleMeshModule.kt`); request MTU 247.
- **Fragments.** Binary header: `envelope_id` prefix 8 B, `idx` u16, `n` u16. About 232 B of payload per fragment, so a 16 KiB envelope is 71 fragments. Peers sends base64 text; binary saves about a quarter.
- **Relay whole envelopes, not fragments.** A node reassembles, checks that the BLAKE2b hash matches `envelope_id`, then decides to relay. Peers' flood keys its seen set on a 32-bit FNV hash anyone can forge (`src/native/bleFlood.ts`); keying on a verified hash means an attacker cannot block a payment by pre-claiming its ID, and corrupt fragments never propagate.
- **Missing fragments.** Ask the same neighbour for missing indices (Peers' `Reassembler.missing`), link-local, never flooded. Keep at most 32 partial envelopes (Peers `MAX_INFLIGHT`).
- **Store and forward.** Keep envelopes until their payment's expiry height or 24 h. When a link opens, exchange `envelope_id` prefixes (`cuq`/`cur`) and send what the neighbour lacks. This replaces Logos Store for delay tolerance; Peers has no such step.
- **Ingress gate** before anything is parsed: frame size cap, 100 frames per 10 s per source and 400 globally, replay dedup (Peers `BleIngressGate.kt` L35–40). Raise per-source limits if 71-fragment envelopes trip them.
- **Advert.** Service data: a rotating 6-byte ID, HMAC of the node's key over a 15-minute epoch (Peers `bleIdentity.ts`), plus a flags byte with a "has internet" bit, used to show reachable gateways.
- **No link authentication.** Envelopes authenticate themselves (verified ID, inner signatures, sealed bodies); the ingress gate bounds abuse. Peers' review lists the remaining denial-of-service surface (F-09); it applies here too.
- **macOS permission.** Add `NSBluetoothAlwaysUsageDescription` to our Basecamp fork's `app/macos/Info.plist.in`. [unverified] attribution to Terminal when run under `logosctl`.
- **To measure first:** a 71-fragment envelope across 3 hops with 6-link fan-out, GATT versus L2CAP channels (both exist on macOS and BlueZ), and time per hop.

## `zcash_offline_module` contract (draft `.lidl`)

```
module zcash_offline_module {
  version "0.1.0"
  description "Offline Ironwood payments over a mesh."
  category "wallet"
  optional_depends [mesh_delivery, mesh_libp2p, ble_mesh_module]

  type Config {
    ; "mesh_delivery", "mesh_libp2p" or "ble_mesh_module"
    transport: tstr
    ; JSON passed to mesh_transport.start
    transport_config: tstr
    ; fleet file: enrollment records, group key, gateways, policy, branch ID, expiry, fee
    fleet_file: tstr
    ; set on a node with internet: Zakura or lightwalletd endpoint
    ? gateway_endpoint: tstr
  }
  type Payment {
    payment_id: tstr
    ; "out" or "in"
    direction: tstr
    counterparty: tstr
    amount_zat: int
    ; receipt states from payee-verifier.md: sent, checked, checked_single_source,
    ; submitted, mined, conflicted, expired, rejected
    state: tstr
    ? reason: tstr
    txid: tstr
    updated_ms: int
  }
  type Status {
    transport_state: tstr
    peer_count: int
    gateways_reachable: int
    chain_height: int
    chain_view_age_s: int
    spendable_zat: int
    pending_out_zat: int
  }

  method init(config: Config) -> result
  method status() -> Status
  method get_address() -> tstr
  ; Online only: sync notes and witnesses before going offline.
  method sync_now() -> result
  ; Builds, proves, signs and sends; value is the payment_id.
  method pay(payee: tstr, amount_zat: int, memo: tstr, invoice_hash: tstr) -> result
  method list_payments() -> [Payment]
  method get_payment(payment_id: tstr) -> Payment

  event payment_updated(payment_id: tstr, state: tstr)
  event status_changed(transport_state: tstr, peer_count: int, gateways_reachable: int)
  event chain_view_updated(height: int, age_s: int)
}
```

### Flows on the mesh

1. **Pay (A).** Core builds the transaction with the configured expiry ([offline-builder.md](../components/offline-builder.md)), signs the PaymentStatement, seals `pay` to B, persists, publishes. Retries per the resend loop until a receipt arrives.
2. **Accept (B).** Opens `pay`, writes it to storage before anything else, runs checks 1–11 ([payee-verifier.md](../components/payee-verifier.md)), posts `nf` to the fleet-payee group, waits the sharing window, seals `rcpt` to A, seals `sub` to the enrolled gateways.
3. **Submit (gateway).** A node with `gateway_endpoint` opens `sub`, submits (Zakura `sendrawtransaction` on the `ths` harness; `zebrad_module` only after the Zebra 7 ↔ Zakura check), mines on Regtest (`ths mine`, issue log #7), and publishes a signed `res` chain bundle with the inclusion proof. **The gateway protocol is problem #4 and still open; this step is the minimum for the bench.**
4. **Result (A and B).** Both verify `res` themselves and move the payment to `submitted`, then `mined`.
5. **Catch-up.** On start and whenever `status_changed` returns to `online`, the core calls `fetch_since` from its last-seen time in ≤ 24 h windows, following the cursor, deduplicating by `envelope_id` and (txid, auth_digest).
6. **Partition recovery.** When the transport has been `offline` with known peers for more than 60 s, call `add_peer` for each. On `mesh_delivery` this is a node restart; rate-limit it to once per 5 minutes.

### Open seams (do not wait on other threads)

- **Signing:** the core builds to a proven, unsigned transaction plus sighash and applies signatures, so a co-signer from the double-spend thread plugs in without changing the module contract.
- **Acceptance policy:** a core trait fed by typed evidence messages; new `kind` values are added when the double-spend design settles.

### `zcash_offline_ui`

`ui_qml`, `interface: universal`, `dependencies: ["zcash_offline_module"]`, like `logos-zcash-wallet-ui`. Screens: Status (transport, peers, gateways, chain-view age, clock warning), Pay (payee from the fleet list or a scanned QR, amount, memo), Payments (list with states and reasons), Receive (address QR), Settings (transport choice, fleet file, gateway toggle). It never touches the network; Basecamp's sandbox forbids it.

### `mesh_offlineprotocol` (option; ready-made multi-hop store and forward)

Adapter over the [Offline Protocol](https://www.offlineprotocol.com/docs/tech/networking) Mesh SDK v0.30.0 (Offline Protocol, Inc.; AGPL-3.0-only or commercial). It brings what the other backends lack or we would build: multi-hop forwarding (TTL 8), an outbox with retries (10 retries, 1–300 s backoff, 7 days), MLS end-to-end encryption, first contact through an intermediate device, and delivery ACKs.

- **Shape.** Run `offline-protocol-service` (PyPI `offline-protocol-sdk[lan]`, wheel for macOS 14 arm64 and Linux glibc 2.34+) as a sidecar. The adapter speaks its local API (JSON-RPC 2.0 over a Unix socket): `send_message` for `publish`, `message_received` for `received`, `message_delivered`/`message_deferred` for `send_state`, `--peer host:port` for `add_peer`. The C binding (`offline_protocol.h` plus a dylib) can be linked in-process instead, but then the host must write its own transport I/O; only the Python, React Native and native phone packages ship transports.
- **Laptop links.** Peer streams use TCP over an existing IP network (configured peers or DNS-SD `_offlineprotocol._tcp`), so routers or a hotspot are still needed, as with `mesh_delivery`. Python BLE uses `bleak` and `bless` (central and peripheral). Its docs say it was run "between an Android phone and Macs and Raspberry Pis"; Mac-to-Mac is not claimed. [unverified]
- **Phones (later).** The same mesh reaches iPhones over Bluetooth LE, AWDL or a shared LAN, and Android over Bluetooth LE, Wi-Fi Direct or a shared LAN (Swift and Android packages are previews). That makes a phone a mesh peer, not only an Edge client.
- **Fits our messages.** Direct messages carry up to 256 KiB (frames up to 1 MiB); a 16 KiB `ozm1` envelope fits in one message. A protocol ACK comes before app persistence, so `rcpt` stays the acceptance signal.
- **No internet dependencies.** It needs no DNS answer and no account; local traffic is never metered. Control-frame freshness depends on clocks (threat model R7b); tolerance not measured. [unverified]
- **Costs.** The licence is AGPL-3.0-only: our module and fork must be AGPL and ship source to users, which works for an open-source fork. A closed build or the Apple App Store needs the commercial licence. It is a single vendor at 0.x, its phone packages are previews, and it is a second stack beside Logos rather than Logos itself.

**Orb bench (26 October 2026, `offline-protocol-sdk` 0.30.0, Python 3.11, loopback peer streams, chain A–B–C with no A–C link, 9,166 random bytes sent as 18,332 hex characters):**

| Case | Result |
|---|---|
| A → C, all up, never met | C received it `encrypted: true`, `hop_count: 1`, 0.97 s after send, including the session set up through B; A got `message_delivered` at 1.75 s. B logged `message_relayed`. |
| A → C while C was offline, never met | A's send was held. C came back 32 s later and received it 59 s after that (91 s total), then A got `message_delivered`. The gap is retry backoff. |
| Memory | About 43 MB RSS per node (Python process). |

Not measured: Bluetooth LE (no radio in the orb), more than 3 nodes, retry exhaustion (what happens after 10 retries), and clock skew.

## Upstream issue drafts (not filed)

Filing is an external action; ask the owner first.

1. **logos-delivery: a node with no internet DNS never reconnects.** After every peer drops, `connectToRelayPeers` returns early while the node is offline (`peer_manager.nim` L665–668), and "online" needs a connected peer or a DNS answer for `one.one.one.one` from `--dns-addrs-name-server` (`health_monitor/online_monitor.nim` L20–56). On an isolated network the node stays `Disconnected` forever. Reproduced on v0.39.2 (`fd07b72`): link restored about 10:17, still `Disconnected` at 10:42. Ask: treat "has dialable static or known peers" as reason to try, or make the check configurable; expose `dns-addrs-name-server` in `messagingOverrides`.
2. **logos-delivery: reconnect backfill fetches one page.** `checkStore` (`recv_service.nim` L156–197) issues one Store query for the whole offline interval and ignores `paginationCursor`; Store also rejects ranges over 24 h (`waku_store/common.nim` L159–163). Reproduced: 30 messages missed, 20 recovered (`missedCount=20`). Ask: page and window the reconnect backfill like the startup backfill (`backfill.nim` L149–182).
3. **logos-delivery: no way to tune dial backoff.** 120 s × 4ⁿ with a hard 5-failure limit (`peer_manager.nim` L71–77, L510–532) can drop a static neighbour for good during a multi-hour partition. Ask: configurable backoff, and never give up on `--staticnode` peers. (Read in code; not yet reproduced.)
4. **logos-delivery-module: no peer-management call.** No `addPeer`/`connect` method, so a module cannot recover a neighbour without restarting the node shared by all modules. Ask: expose `connect(multiaddr)`.

## Open

- [decided, owner 26 October 2026] First backend: `mesh_offlineprotocol`, toward v0.1.0 ([build plan](logos-v0.1.0-plan.md)). `mesh_delivery` and `ble_mesh_module` follow.
- [unverified] Offline Protocol Bluetooth LE between two Macs, retry exhaustion, clock-skew tolerance.
- [unverified] Events over interface-bound clients; provider conformance declaration.
- [unverified] `ble-peripheral-rust` advertising plus GATT server on macOS and Linux in one process alongside `btleplug`.
- [unverified] `delivery_module` `stop()`/`start()` resets dial backoff.
- [unverified] 71-fragment envelope over 3 BLE hops; gossipsub on a sparse static mesh.
