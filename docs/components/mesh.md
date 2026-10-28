# Mesh transport

System view: [../design.md](../design.md). Gateway side: [gateway.md](gateway.md). Code: [`crates/ozm-mesh`](../../crates/ozm-mesh/README.md) (wire format) and [`crates/zcash-offline-core/src/envelope.rs`](../../crates/zcash-offline-core/src/envelope.rs) (sealing).

## Decided (built in v0.1.0–v0.3.0)

- **Own mesh crate (`ozm-mesh`), BitChat semantics.** It replaced both the BitChat fork plan and the Offline Protocol sidecar (26 October). The behaviour:
  - TTL flooding (default 7) with de-duplication by message ID;
  - 512-byte frames, to match BLE;
  - store-and-forward with an inventory sync whenever a link comes up;
  - end-to-end delivery acks, with stored copies dropped once the ack is seen;
  - a persisted store, and dialing that never gives up.

  The reasons: no radio exists in orbs, the BitChat apps are Swift-only, and Offline Protocol is an AGPL Python sidecar whose custody refuses DMs. The BitChat fork plan is archived at [../../archive/docs-2026-10-27/transport-bitchat.md](../../archive/docs-2026-10-27/transport-bitchat.md).
- **Links:** TCP between neighbours; v0.4.0 adds an Android BLE byte-stream implementation (built, no radio validation yet). LAN discovery uses UDP beacons on port 7778, sent on every interface as directed broadcast, multicast `239.255.77.77` and `255.255.255.255`. Through v0.7.2 invited devices put an 8-byte fleet tag in each beacon and auto-connected only to their own fleet, open devices and gateways; since v0.7.3 every device sends a zero tag and connects to any Offline Zcash device, because every device relays and the tag authenticated nothing. v0.7.2 peers accept zero tags, so mixed versions still connect. It works on a Wi-Fi with no internet route. Both transports share the link framing and routing.
- **App envelope `ozm1`:** every message authenticates its sender and intended recipient, and is sealed to the recipient's X25519 key (derived from the Ed25519 payment key) with ChaCha20-Poly1305. `pay`, `sub` and `rcpt` retain 4 KiB-multiple padding. v0.7.0 sync/control uses 512 B / 4 KiB / 16 KiB buckets (then 16 KiB multiples). Sender/recipient mesh IDs, size and timing remain visible; sync and payments are distinguishable by size.
- **Messages:**

  | Kind | From → to | What it carries |
  |---|---|---|
  | `pay` | A → B | Transaction and signed Statement |
  | `rcpt` | B → A | Signed Receipt, sent only after the payment is fsynced |
  | `sub` | A and B → gateway | Transaction to submit |
  | `res` | Gateway → A and B | Signed gateway result |
  | `syncq`/`syncr` | Device ↔ gateway | Blocks, so an offline wallet can sync |
  | `fundq`/`fundr` | Device ↔ Regtest gateway | Test funds |

- **Delivery:** the mesh stores and forwards. On top of that, the app retries by txid with backoff until it has a receipt (A) or the payment is mined (B). Both A and B submit to every gateway, and submission is idempotent.

## Findings

### v0.7.0 compact sync

- Fleet network defaults to Regtest. Testnet identities use `utest1` and a birthday
  chosen per device automatically, seeded from GetTreeState at birthday−1 and
  checked against the block's Ironwood commitment-tree size. Regtest uses RPC
  tip−100, at least 1. Offline-created devices retain creation time until chain
  contact, then conservatively interpolate creation−one day, not the contact tip.
  v0.7.4 appends authenticated `clockq`/`clockr` variants carrying only the
  gateway's tip height/time; creation time never leaves the wallet. Existing
  compact batch, envelope and frame layouts are unchanged. Upgrade gateways for
  automatic offline-created wallet bootstrap; older nodes drop the new variants.
- `syncq` normally requests only the current height/hash; the gateway asks for
  the 200-hash retained history **only on predecessor mismatch**. `syncr`
  carries Ironwood nf/cmx/epk/52-byte ct, not full transactions. Consecutive empty
  blocks are runs ending at a height/hash. Binary batches split into 3 KiB parts,
  including **inside a block or transaction**, with a complete-batch digest and
  bounded 8 MiB assembly. No partial batch advances the wallet cursor.
- Wallets persist a pruned incremental ShardTree, 200 checkpoints, recent roots,
  hashes and recent nullifiers with their cursor. Reorgs rewind a common retained
  checkpoint and replace orphaned effects; deeper forks require resync from the
  original birthday. Reservations survive a rewind. No unmined change is spendable.
- Since v0.7.4 gateway tip pushes carry height/hash on every observed new tip
  (polling every five seconds), so devices sync without a user action. This
  intentionally prioritises readiness over v0.7.0's 15-minute coalescing budget;
  its dated traffic measurements below do not describe v0.7.4. A device requests
  blocks only when its view differs; mesh batches still contain at most 200
  blocks. Payment-result transitions push immediately. A 15-minute fallback backs
  off to an hour when unchanged; an unchanged answer is a small height/hash tip,
  not a compact batch. Full signed presence refreshes **hourly** on stable links,
  expires after three hours, and is immediate on link-up. A gateway role/change
  or loss can now take up to an hour to be noticed on a stable link; missed
  beacons can delay expiry to three hours. TCP/BLE keepalives still detect a dead
  direct link on their transport timers. This is not cover traffic.
- `sync-served` / `synced` events count binary **payload bytes**; `mesh-send`
  also records sealed bytes. `status.mesh.bytes_sent/bytes_received` count full
  framed mesh-stream bytes, including packet headers, fragmentation, acks,
  inventory and keepalives. TCP/IP and BLE radio overhead are additional.
- The 27 October 18:35 UTC sample, heights 4,544,868–4,548,324, contained
  3,457 blocks over 86,390 seconds, 5,753 Ironwood actions and 2,515 empty blocks:
  **1,340,049 protobuf bytes**. SHA-256 of length-prefixed protobuf records:
  `cf430ef90b6dd2fbfc3797eb7f669177401f339e9ba8da9cae374da4f29763c5`.
  Replaying those same blocks through the production binary batches, authenticated
  sealing, padding and 512-byte framing gives:

  | Mode | Compact payload bytes | Framed sync bytes | Total BLE-link framed bytes | Total TCP-link framed bytes |
  |---|---:|---:|---:|---:|
  | Initial one-request-per-block implementation, history on every request | 1,744,294 | 79,804,833 | Not replayed with idle traffic | Not replayed with idle traffic |
  | Final 200-block catch-up, including a day's idle control | 1,130,924 | 1,776,714 | 2,063,030 | 2,178,220 |
  | Final steady-state input, one block at a time with 15-minute coalescing | 1,144,132 | 2,118,573 | **2,404,889** | **2,520,079** |

  The steady-state replay uses the recorded block timestamps and flushes the last
  partial batch. Framed counts include both-direction sync requests, responses,
  tip announcements and signed delivery acks on **one lossless direct link**:
  each sealed payload adds 49 packet/link bytes and 25 bytes per 512-byte fragment;
  each ack has 144 payload bytes plus the same framing. Total columns also include
  **228,624 B** for 24 hourly routing-key broadcasts/full signed presence exchanges
  and their acks in both directions, **57,590 B BLE / 172,780 B TCP** for 5-byte
  keepalive frames every 15 s / 5 s in both directions, and **102 B** for two hellos
  and empty initial inventories. Inventories are sent on link-up, not periodically.
  A stalled tip can add fallback probes: one small request/reply plus acks is
  1,788 B; 24 additional hourly probes would add 42,912 B/day. Ordinary busy-tip
  replay groups already request the accumulated blocks at the fallback interval.
  Reconnect inventories, reorg exchanges, retransmissions, payment traffic, UDP
  discovery and TCP/IP/BLE radio headers are **excluded** from this stable-link
  model. This is serialization/framing replay, not a 24-hour or physical-radio
  capture. **Total** steady-state framing is 2.40 MB/day BLE (2.00× the approximate
  1.2 MB/day target), or 2.52 MB/day TCP (2.10×), within the agreed **about 2×** bound.
  Actual full-stream counters still need measurement on physical BLE.

- 9,166-byte transaction → 12,353-byte `pay` envelope. With 20 ms per 512-byte frame, the full A→R→B→D→chain→A and B loop takes about 3.6 s ([../mvp-v0.1.0.md](../mvp-v0.1.0.md)).
- An internal Docker network, like an offline Wi-Fi, has no default route. Discovery beacons must therefore go out per interface, not to the default route; this was found and fixed in v0.2.0.

### v0.7.4 store quota during long syncs

On v0.7.3 a non-gateway device catching up over the mesh stopped with "mesh store
quota exceeded" (public testnet, birthday 4,465,026: stuck at 4,467,225 after about
1,800 blocks). Its store held 510 of its own acks, one per received 3 KiB sync
chunk, plus two broadcasts: 512, the per-sender cap. Acks are kept for the 24-hour
store TTL so an offline sender can still learn of delivery, and nothing acks an
ack, so they never left the store. Its sync requests were not the cause: they are
removed when acked. The gateway held the same 510 acks, so it would also have
dropped the device's next request. Since v0.7.4 a sender's new packet evicts that
sender's oldest stored acks first, and the sending device may also evict its own
old broadcasts. Held messages and other senders' packets are never evicted, so the
v0.7.2 per-sender and global limits are unchanged.

The same catch-up then stalled on busy testnet ranges for two more reasons. A
200-block batch can be hundreds of 3 KiB chunks (186, 352 and 710 were seen). The
link queue holds 128 packets, and `flood` dropped the rest; they were stored but only
re-sent after a reconnect, so the device received exactly 128 of 186 chunks and never
completed the batch. Since v0.7.4 a packet that does not fit an authenticated link's
queue joins that link's pending list and is sent as the queue drains, and a device
that dropped packets because its app queue was full sends that neighbour its
inventory on the next 10-second sweep. A 710-chunk batch also exceeded the gateway's
own 512-packet quota of unacknowledged messages; the gateway now waits for acks
(`StoreFull`, up to 30 s per chunk) instead of abandoning the batch.

## BLE link (Android first)

### Decided (27 October)

- **Option A: keep `ozm-mesh` and add a native BLE link per platform; do not fork the BitChat apps.** The link only moves bytes between neighbours; routing, store-and-forward, acks, sealing and receipts stay in the Rust core, unchanged, so TCP, LAN and BLE neighbours mix in one mesh.
- **Android first; Linux desktops next, then macOS and iOS.** Linux uses BlueZ via `bluer`, which supports both central and peripheral roles. btleplug and bluest are central-only.
- **Why not a BitChat fork** (read at bitchat `04995f68`, bitchat-android `eed88190`, 27 October; research):
  - private messages are capped at 255 bytes on both apps, so a 12.3 KB payment needs a new payload type in two forks;
  - only the sender stores a private message (outbox: 100 per peer, 24 h, 8 attempts); relays hold single frames for 60 s and never hold fragments, and conversations are memory-only. Our mesh lets every device hold a sealed message until the recipient appears;
  - BitChat's private messages need a live Noise XX handshake with the recipient (our reading, not traced in code); our `ozm1` envelope is sealed ahead of time;
  - the BLE code is entangled with the app (iOS `BLEService.swift` alone is 7,187 lines; its link-layer split is planned upstream, not done) and bitchat-android is **GPLv3**.
- **Licences for borrowed code:** BitChat iOS is Unlicense and Berty's BLE drivers (berty/weshnet `71f6981d`, berty/berty) are Apache-2.0/MIT, so code may be copied with attribution. bitchat-android is GPLv3: read it for patterns, copy nothing.
- **Not compatible with BitChat users.** Our service UUID and packet format are our own. An interop mode (BitChat's UUID, header, fragments and signed announces) is possible later, about 3–5 extra person-weeks (estimate), and stock nodes would still never store our payments.

### Implementation (v0.4.0, hardware gates open)

`crates/ozm-ffi` and `android/` implement the Android-first design below. [Tester/build guide](../android-install.md). B1 runs without a radio; B2–B7 remain unverified. Numbers marked *estimate* come from source reading and arithmetic, not radio measurements. No code was borrowed from either BitChat app or Berty.

```diagram
┌──────────── Android app (Kotlin) ─────────────┐
│ UI (join, pay, payments)                       │
│ BleLink: scan + advertise + GATT client/server │──── BLE ──── neighbour phones
│ MeshService (foreground service)               │
└───────────────┬────────────────────────────────┘
                │ uniffi (JNI)
┌───────────────▼────────────────────────────────┐
│ Rust: ozm-node app + ozm-mesh (unchanged)      │──── TCP/LAN ── laptops, gateway
│ Mesh::attach(stream, outbound) → run_link      │
└────────────────────────────────────────────────┘
```

**1. Rust side: attach any stream.** `run_link` takes any `AsyncRead + AsyncWrite` ([`link.rs`](../../crates/ozm-mesh/src/link.rs)). Public `Mesh::attach(stream, outbound)` runs it. The FFI layer (uniffi, Kotlin bindings, built with `cargo-ndk`) keeps a `tokio::io::duplex` per BLE connection and exposes four calls:

| Call | Direction | Meaning |
|---|---|---|
| `link_opened(handle, outbound)` | Kotlin → Rust | A GATT connection is ready; Rust attaches a duplex for it |
| `link_received(handle, bytes)` | Kotlin → Rust | Bytes arrived; written into the duplex in order |
| `LinkSink.write(handle, bytes)` | Rust → Kotlin (callback) | Bytes to send; Kotlin chunks them to the ATT payload size |
| `link_closed(handle)` | either way | The connection ended; the other side drops its half |

The Hello frame identifies the peer, and the existing rule keeps one link per pair, so the BLE layer never needs to know PeerIds. Hello, Data and Ping frames are unchanged (21 bytes of framing per 512-byte frame).

`Mesh::attach` runs to completion with the BLE timer policy; TCP entry points retain their own policy. Cancellation also unregisters the neighbour. `Mesh::neighbor_links` and node status `neighbor_links` expose `tcp`/`ble` without changing the existing `neighbors` list. UniFFI uses a per-device Tokio runtime. Device operations return the same JSON objects/errors as the Logos module (join, start, status, fleet, pay, payments, sync, funds); they run on a Kotlin worker, not the UI/BLE handler. Link handles must be unique across reconnects. Incoming chunks (1–514 bytes) use a bounded 128-chunk queue and a 64 KiB duplex; oversize/overflow closes the link rather than silently dropping data. `LinkSink.write` returns only after local platform callbacks have accepted every chunk; false closes the link. Rust calls `LinkSink.link_closed` on EOF, duplicate rejection, bad framing, timeout or explicit closure. Kotlin serializes receive callbacks per link and unblocks pending writes on close.

**2. GATT layout.** One service with our own random 128-bit UUID, **the same in Debug and Release** (BitChat's differ, so mixed builds never meet). One characteristic:

- central → peripheral: *write without response*;
- peripheral → central: *notify*.

Each side treats its direction as an ordered byte stream: the length-prefixed frames are cut into chunks of (ATT MTU − 3) bytes and joined on the other side, with no extra header. The sender waits for the platform's flow-control callback before the next chunk (Android `onCharacteristicWrite` / `onNotificationSent`; iOS `peripheralIsReady(toSendWriteWithoutResponse:)` / `isReady(toUpdateSubscribers:)`). These callbacks mean **local queue readiness, not remote/application delivery**; write-without-response and notify provide no application ack. Rust's end-to-end ack and inventory repair remain necessary. A callback error or 15 s local write timeout closes the stream. Any framing error closes the link; inventory sync on the next link-up repairs whatever was in flight.

Service UUID `b91d6e31-89f5-4a38-9761-72d1305a4f12`, stream characteristic `b91d6e32-89f5-4a38-9761-72d1305a4f12`, standard CCCD `0x2902`. Both APK variants share them. Legacy advertisements put 128-bit service data + four hint bytes in the primary packet, and the UUID list in the scan response: both fields together would exceed the 31-byte advertisement limit. Through v0.7.2 the hint was the first four bytes of the fleet's LAN discovery tag, a filter, not authentication. Since v0.7.3 it is always zero, and the Android FFI `fleetHint()` is removed.

**Android correction to the proposed MTU arithmetic:** actual GATT chunks are `min(ATT MTU − 3, 512)`, because Android's [`GATT_MAX_ATTR_LEN`](https://android.googlesource.com/platform/packages/modules/Bluetooth/+/refs/heads/main/framework/java/android/bluetooth/BluetoothGatt.java) rejects attribute values above 512 bytes even at MTU 517. The transport/FFI tests still cover 514-byte stream chunks. Failure to negotiate MTU does not disconnect; the link keeps its default 20-byte payload. The service UUID and the four hint bytes fit without extended advertising.

**3. MTU and speed.** Request MTU 517 and 2M PHY; if negotiation fails, keep the link at the default (23-byte ATT MTU) rather than disconnecting, as bitchat-android does. A 12.3 KB `pay` envelope is about 25 frames of 512 bytes, or about 27 ATT writes at MTU 517. Planning figure (*estimate*): 1–3 s per hop warm, up to ~15 s cold (scan, connect, MTU, Hello). Later option: an L2CAP connection-oriented channel (Android 10+, iOS 11+) is a real byte stream and likely faster; add it only if GATT measurements fall short.

**4. Discovery and connections.**

- Every device scans and advertises at once (central and peripheral). The advertisement carries the service UUID and a 4-byte hint in service data. Through v0.7.2 the hint was a fleet filter, so devices from other fleets were skipped before connecting; since v0.7.3 it is zero and devices connect to any Offline Zcash device. The PeerId is not advertised; it is learned from Hello after connecting.
- At most 6 BLE links per device, counting pending central and peripheral connections. Scans skip already connected device addresses and do not evict healthy links when full; a new inbound connection evicts the longest-idle link. Hello applies the existing one-link-per-PeerId rule across TCP/BLE. Transport preference for peers already reachable over LAN is not implemented (PeerIds are deliberately absent from advertisements); measure this before adding peer/address caching.
- Both devices may connect to each other at once; the existing duplicate-link rule keeps one. Measure how often this wastes a connection slot before adding a tie-break.
- Scan duty: continuous while the app is open, 25 % (for example 5 s on, 15 s off) when backgrounded. *Estimate*; tune against battery.

**5. Timers per link kind.** TCP keeps the 5 s ping and 20 s idle timeout. BLE uses a 15 s ping and 60 s idle timeout, set when the stream is attached, so slow or briefly stalled radios are not dropped. Reassembly stays at 30 s.

**6. Android app shell.**

- `minSdk` 29 (L2CAP and modern BLE APIs); `BLUETOOTH_SCAN` (with `neverForLocation`), `BLUETOOTH_ADVERTISE` and `BLUETOOTH_CONNECT` on Android 12+; location permission only below 12.
- A foreground service of type `connectedDevice` holds the Rust mesh and the BLE link, so relaying continues with the screen off.
- The first app is a test shell: join with an invite, pay, list payments, show neighbours and link kind. The payment logic is `ozm-node`'s, called through the same uniffi layer.
- A phone on Wi-Fi with no internet still uses LAN discovery, so one phone can bridge BLE neighbours to laptops and the gateway.

**7. Then iOS.** Same GATT layout in Swift (CoreBluetooth), Rust core through uniffi Swift bindings. Borrow BitChat iOS's state restoration (`bluetooth-central` and `bluetooth-peripheral` background modes, restore identifiers). Known limits: in the background, scanning must name our exact UUID, the advertisement cannot change, and the UUID is only sent in Apple's overflow area. The payer is foreground-only on iOS (decided earlier); relaying in the background must be tested early with two phones.

### Milestones and gating tests

| # | Milestone | Passes when |
|---|---|---|
| B1 | `Mesh::attach` and the FFI duplex, no radio — built | Rust test: 20,123-byte payload over 20/182/514-byte bounded streams, callback stalls, delayed second hop/store-and-forward and ack; timer tests preserve TCP 5/20 s and BLE 15/60 s. `bench/ffi-bench.sh`: A–B–D through Kotlin callbacks only, funds + sync + payment/receipt/mined at all three chunk sizes. Container mesh/LAN benches are regression gates |
| B2 | Two Android phones, one hop | Hello completes; a 12.3 KB message is delivered and acked; delivery time measured at MTU 517 and at the default MTU |
| B3 | Three phones in a line | A debug allow-list forces A–R–B; a payment reaches B through R, the receipt comes back; 1-hop and 2-hop times recorded |
| B4 | Full payment | A and B on phones, offline; the gateway is a laptop reached by a phone on a no-internet Wi-Fi; the payment is mined and `mined` reaches both phones |
| B5 | Durability | B's app is killed and restarted mid-transfer; R is out of range for 10 minutes then returns; nothing is lost and no duplicate payment appears |
| B6 | Screen off | R relays with its screen off for 15 minutes under the foreground service |
| B7 | Capture | A BLE sniffer capture (for example nRF Sniffer) shows no plaintext amount, address or key |

Rough effort (*estimate*): B1 1.5–2 weeks; Android link and app to B4 about 3 weeks; tuning and field tests 2–3 weeks. iOS adds about 3 weeks.

## Linux desktop BLE (implementation, radio gates open)

`crates/ozm-ble` runs a BlueZ GATT server and connectable LE advertisement alongside
an LE scanner and GATT client. `ozm-node start --ble` and the Basecamp module's
`start('{"ble":true}')` opt in; TCP/LAN remains available. Startup fails clearly if
the system bus, bluetoothd, powered adapter, GATT registration, advertising or scan
is unavailable. With BLE off, neither BlueZ nor an adapter is required. No version
bump or new release is implied by this implementation.

- **Android wire compatibility:** the exact service/stream UUIDs above, BlueZ's
  automatic CCCD, write-without-response central → peripheral and notify in the
  reverse direction. No extra chunk header. Four hint bytes in 128-bit service
  data (zero since v0.7.3; a fleet filter before) authenticate nothing. Linux omits the UUID list to
  keep the primary advertisement within 31 bytes; Android's scan filter uses service
  data. No extended advertising is required.
- **Per-device streams:** BlueZ `AcquireWrite`/`AcquireNotify` supply separate
  flow-controlled fds per device, including Android clients using ordinary GATT.
  The shared characteristic Value/StartNotify callback would broadcast bytes to all
  subscribers and is deliberately not used. Both directions feed a bounded 64 KiB
  Tokio duplex into `Mesh::attach`, retaining BLE 15/60 s timers and mesh dedup/acks.
  One chunk is submitted at a time, with a 15 s local-write deadline; failure, EOF,
  duplicate rejection or cancellation drops both halves. Local queue acceptance is
  not remote delivery. Mesh inventory repairs interrupted transfers.
- **MTU:** BlueZ negotiates it; default ATT MTU 23 remains usable. Chunks are
  `min(ATT MTU − 3, 512)`. Pinned BlueR 0.17.4 reports writer MTU minus **five**;
  the backend takes ownership of its fd and restores the Android-compatible minus
  three calculation instead of sending 18-byte chunks at the default MTU. PHY is
  left to BlueZ/controller policy, not forced to 2M.
- **Lifecycle:** at most six application GATT links, counting pending central
  attempts and peripheral setups from the first acquired fd. Scans skip active
  addresses and full slots. Incoming setup evicts the longest-idle slot; half-open
  setup expires after 15 s. BlueZ owns physical ACL connections (including other
  desktop Bluetooth applications), so this is not a system-wide six-ACL cap.
  Simultaneous central/peripheral streams are allowed during a race; mesh Hello
  selects one peer link across BLE and TCP. Linux scans continuously while running.
  Stop/restart the node to change BLE; Basecamp saves the start option for restart.
- **Packaging:** `dbus`'s `vendored` feature statically bundles libdbus into the CLI
  and module staticlib; no new host libdbus dependency or Nix-store path is needed
  by the portable `.lgx`. BlueZ and the system D-Bus daemon are still host services.
  Backend modules are platform-separated; unsupported platforms return a clear
  error. A macOS CoreBluetooth backend is not implemented.

Protocol tests run without BlueZ: chunk limits/fleet filtering, bounded writes and
stalls, cancellation/disconnect, slot limits, stale completion after reconnect,
multi-hop store-and-forward and end-to-end ack. Existing in-memory mesh and real
Regtest TCP/LAN/Kotlin benches remain regression gates, not radio evidence.
The orb has no `/dev/vhci`. BlueZ's [test-runner](https://github.com/bluez/bluez/wiki/test%E2%80%90runner)
can use `btvirt` with a VHCI-enabled kernel (or a custom-kernel QEMU VM), but a
working hosted GitHub Actions controller setup has not been established here;
CI runs the protocol tests and compiles the Linux backend, not a speculative radio
job. Virtual controllers would not close the following real-hardware gates.

| # | Milestone | Passes when |
|---|---|---|
| D1 | ozm-node Linux ↔ Linux and Linux ↔ Android | Real adapters/phone, both central/peripheral directions; discovery (v0.7.3 has no fleet filter); 12.3 KB payment/receipt/mined; default and negotiated MTU; no LAN shortcut; per-hop timings and six-slot/reconnect behaviour recorded. **Implementation present, hardware unverified** |
| D2 | Basecamp module on Linux | Portable `.lgx` installed in released Basecamp, Setup Bluetooth toggle enables BLE, restart restores it, BLE/TCP labels are correct; payments to Linux CLI and Android work with LAN off at BLE endpoints, reaching a gateway through a third BLE/LAN device. **Implementation present, hardware unverified** |
| D3 | macOS backend | `objc2-core-bluetooth` implements the same streams, discovery, chunking and lifecycle; CLI Linux/Android interop measured. **Not implemented** |
| D4 | macOS in Basecamp | After Basecamp adds `NSBluetoothAlwaysUsageDescription` ([logos-co/logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527)), module BLE works in the distributed host and permission denial is usable. **Blocked on host support and D3** |
| D5 | Durability and capture | Interrupted transfer, process restart, ten-minute disconnection and mixed BLE/LAN routing lose no payment or ack; sniffer capture contains no plaintext amount, address or key. **Hardware unverified** |

## Open

- D1/D2/D5: Linux hardware, BlueZ versions, scan/advertise coexistence, MTU/PHY, fd flow control under saturation, duplicate races and actual adapter connection limits remain unverified. D3/D4 are future macOS work, not part of this Linux backend.
- B2–B7: nothing has run over a radio yet. The native Android UI, permissions, actual MTU fallback, 2M PHY, scan/advertise coexistence, phone LAN bridging, proving time/RAM, per-hop times, connection slots, battery cost and screen-off/vendor background limits remain unverified. No emulator was used (the orb has no Bluetooth or KVM).
- Tracking through BLE advertisements and stable peer IDs. v0.7.3 removed the fleet hint; the service UUID and peer ID stay stable ([../privacy-and-threats.md](../privacy-and-threats.md)).
- Interoperating with BitChat's packet format, if mixed fleets are wanted.
- Discovery beacons announce the mesh ID in clear on the LAN; the fleet tag is zero since v0.7.3 ([../privacy-and-threats.md](../privacy-and-threats.md)).
- Nullifier sharing between payees (`nf`), and the evidence messages (`evq`/`evr`).
