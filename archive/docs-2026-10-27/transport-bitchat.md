# Transport: BitChat fork

The MVP transport is a fork of BitChat on four Macs. A sends the finished transaction to B as a Noise-encrypted private message; B saves it and forwards it to D (the gateway) as a second private message; D submits it.
BitChat already has a sealed on-disk sender outbox for private messages, so B can be offline when A sends. It has no other delay-tolerant storage that suits a transaction, so the payee app must persist what it receives.
System view: [../design.md](../design.md). Gateway side: [gateway.md](gateway.md). Later transport: [../later/logos.md](../later/logos.md).

## Decided

- [decided] Four Macs (macOS 13+): payer A, payee B, second payee C (all offline), co-signer processes spread across the Macs, gateway D (online), each running a fork of [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat) at `04995f68`. iOS and macOS share one codebase, so the same fork later runs on iPhone (iOS 16+). Android would need a separate fork of [permissionlesstech/bitchat-android](https://github.com/permissionlesstech/bitchat-android) (Kotlin); the first phone platform is not decided, and wire compatibility between the two forks for our envelopes is [unverified]. There is no Linux, Windows or CLI client, which is why the laptops are Macs ([reconciliation G9](../../archive/reports/Design%20review%20reconciliation.md)).
- [decided] The transaction goes **A → B as a private message**, not a file or public broadcast. **Correction 26 Oct:** stock BitChat caps private text at **255 UTF-8 bytes** ([`Packets.swift` L242–296](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Protocols/Packets.swift#L242-L296); `encode()` returns nil above it), so the fork needs a `privateMessageV2` with a u16 length (proposed: ≤44,000 decoded bytes, outbox kept). **Approved 26 Oct**; needs transport tests.
- [decided] B writes the transaction to its own storage **on receipt**, before confirming to A. BitChat keeps conversations in memory only.
- [decided 26 Oct] **A and B both submit** the raw tx bytes to a gateway as separate private messages (`ozc1:sub`); a packet addressed to B stops at B. The PaymentStatement still goes only to B. Parallel delivery narrows A's sweep window.
- [decided 26 Oct] **`privateMessageV2`**: a new fork Noise payload type, u16 length, ≤ 44,000 decoded bytes, covered by the outbox. A 12.7 KB payment is ≈28 BLE fragments, a 32 KiB bundle chunk ≈95; both inside the 30 s reassembly window.
- [decided 26 Oct] **Gateway broadcast type** for `ozc1:ack`, `ozc1:tip`, `ozc1:cnf`: signed plaintext, ≤ ~2 KB, normal TTL, relayed only by fork nodes (stock apps ignore it). Each relay checks the gateway signature against enrolled gateway keys before relaying and drops anything else.
- [decided 26 Oct] **App retry queue**: every payment stored encrypted, keyed by `(txid, auth_digest)`; resent when the peer reappears and otherwise on a 10 min → 6 h backoff; stops only at final proof or `expired-unresolved` plus a margin, never on a timer alone. A BitChat ack means *carried*; only the signed app receipt means *saved* (measured on Offline Protocol: transport acks precede app persistence).
- [decided] Relays and the gateway never receive payment intent (invoice, amount, payer signature). Payment details travel only inside A's private message to B.
- [decided] All four Macs use the **same build configuration** (all Debug or all Release), because Debug and Release use different BLE service UUIDs.
- [decided, identity corrected] Exact-byte resends are idempotent; deduplicate transaction variants by **(txid, auth_digest)**, not txid alone: v6 re-anchoring keeps txid but changes authorizing bytes. Grouping variants into one payment-family obligation is **decided 26 Oct** ([expiry-reissue.md #2](expiry-reissue.md#proposals-and-owner-decisions-26-october)).
- [decided] Application messages are BitChat private-message texts with an `ozc1:` prefix: `ozc1:pay:` (tx + PaymentStatement, A→B), `ozc1:rcpt:` (Receipt, B→A), `ozc1:evq:`/`ozc1:evr:` (evidence request and reply, later). Pending-nullifier shares go as **group messages** (`0x25`) to a fleet-payee group, `ozc1:nf:`. Formats in [pending-tier.md](pending-tier.md).

## Findings

Source: `permissionlesstech/bitchat@04995f68` (Swift, Unlicense), read in code. Line numbers below are from the re-check on 25 October 2026 and supersede those in the archived note.

BitChat's sender outbox is bounded at **24 h / 8 attempts**, not a multi-day payment-delivery guarantee. Application persistence and re-enqueue beyond that limit are **required (proposal #12, approved 26 October)**; each holder keeps its copy until final proof, never deleting on a timer. Delivery is minimal for now: gateways at hubs (markets, bus terminals) plus app copies, no carrier protocol; private authenticated repair prefix `ozc1:rq:` is proposal #4, not an approved wire format ([expiry-reissue.md](expiry-reissue.md)). No restart-safe Zcash-over-BitChat payment queue was established by the [prior-art sweep](../prior-art.md).

### Store-and-forward

| Fact | Value | Source |
|---|---|---|
| Private messages go to a sealed on-disk outbox, resent until acknowledged | 100 messages per peer, 24 h TTL, 8 attempts | [MessageRouter.swift L128–135](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/MessageRouter.swift#L128-L135), L462–483 |
| Private file sends (`sendFilePrivate`) | Not outboxed | MessageRouter.swift at `04995f68` |
| Gossip archive restored from disk at start | Only fresh public `.message` packets | [GossipSyncManager.swift L658–692](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Sync/GossipSyncManager.swift#L658-L692) |
| Gossip-sync retention | Fragments, files and announces 15 min; public messages 6 h | [GossipSyncManager.swift](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Sync/GossipSyncManager.swift) |
| What gossip sync carries | Broadcast traffic only, never private messages | GossipSyncManager.swift |
| Relay spool | 60 s, single frames only; fragments are never spooled | [TransportConfig.swift L321–329](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/TransportConfig.swift#L321-L329) |
| Conversations | Memory only; lost on restart | bitchat at `04995f68` |
| Courier envelopes | Sealed text envelopes up to 16 KiB, held 24 h, iOS/macOS only | [CourierEnvelope.swift L39–46](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/CourierEnvelope.swift#L39-L46) |

### Size and framing

- The 60,000-character limit applies to the UI/outer layers, not to private text: `PrivateMessagePacket` content is TLV with a 1-byte length, ≤255 bytes ([`Packets.swift` L242–296](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Protocols/Packets.swift#L242-L296); called from `BLENoisePayloadFactory.swift` L4–9 and `BLEService.swift` L5233–5282). Verified in source 26 Oct.
- Large packets split into fragments with a 13-byte header and a 469-byte chunk, paced 25 ms (directed) or 30 ms (broadcast); fragment reassembly times out after 30 s ([BLEOutboundFragmentPlanner.swift L112–139](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEOutboundFragmentPlanner.swift#L112-L139); [TransportConfig.swift L7–9](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/TransportConfig.swift#L7-L9)).
- Inference: the Noise ciphertext does not compress, so a 12.3 KB message is about 27 fragments, roughly 0.7 s of pacing per hop on a live link [unverified].
- Default TTL is 7; directed packets always relay ([RelayController.swift L26–95](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/RelayController.swift#L26-L95)).

### Identity and visibility

- Peer IDs derive from the static Noise key. Announcements carry nickname, keys and neighbour list, so every nearby device learns who is present and who neighbours whom.
- Peer-ID rotation (`announceV2`) is ignored on receive, so rotation gives no unlinkability today.
- Public payloads are signed, not encrypted ([BLEPublicMessageHandler.swift L75–125](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEPublicMessageHandler.swift#L75-L125)). Private messages use `Noise_XX_25519_ChaChaPoly_SHA256`.
- Debug and Release builds advertise different BLE service UUIDs (`…4B5A` vs `…4B5C`), so mixed builds never see each other ([BLEService.swift L186–191](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEService.swift#L186-L191)).

### Physical link

| Fact | Value | Source |
|---|---|---|
| Link type | BLE GATT connections (every node central and peripheral), not Bluetooth SIG Mesh | [bitchat_mesh.md §2](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/bitchat_mesh.md) |
| Range per hop | Tens of metres (estimate, no measurement found) | [physical_mesh_links.md §1](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/physical_mesh_links.md) |
| Throughput | No measured BitChat number. 469 B / 25 ms ≈ 19 KB/s is scheduling arithmetic only. Planning budget per 12.7 KB payment hop: 1–5 s warm, 2–30 s cold. TTL 7 covers ~70–700 m; beyond that, people carry it: modelled ~1 h city, ~5 h at 20 km, 1–2 days at 100 km mountains, possibly never (latency research); private messages are not gossip-cached | physical_mesh_links.md §1 Gaps; fragment planner above |
| SIG BLE Mesh | 11-byte segments, 384-byte max PDU; unusable for a 9 KB transaction. Does not apply to BitChat, which uses GATT with MTU up to 512 | physical_mesh_links.md §1 |
| Multi-hop latency | Archived estimate "about 1 s over three hops" is scheduling arithmetic, not a measurement | [bitchat_mesh.md §3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/bitchat_mesh.md) |
| Cold start | Discovery, 8 s connect timeout and Noise XX handshake: 2–15 s (estimate) | bitchat_mesh.md §3 |

### Peers (xAlisher/peers `769fe2e`, 26 Oct, source only)

- Borrow: one payment timeline across transports, explicit per-transport size budgets and status (received / submitted / final), and **never truncate a financial payload**. Peers' MeshCore truncates text to 140 bytes with no fragmentation ([composerBudget.ts L18–76](https://github.com/xAlisher/peers/blob/769fe2e706a6dfef24ae6818f6b81ee855680aa3/src/composerBudget.ts#L18-L76)), and its cross-transport dedupe is content within ±10 min, not an authenticated ID; keep `(txid, auth_digest)`.
- Reject: its transport stack (Android-only alpha, RAM-only BLE reassembly, no durable outbox) and its MeshCore↔Logos group bridge, which decrypts and re-encrypts plaintext. LoRa: control messages only (≈95 s per 12.7 KB payload at SF11, duty-cycle limits per country). Peers research

### Corrections to the archive

- The archived note and report say private text is capped at 255 bytes and recommend sending the transaction as a private **file** or a new public broadcast type `0x2D`. Re-verified 26 Oct: the archive's 255-byte cap is **correct** for private text; the earlier "60,000 characters" correction was wrong, private files are not outboxed, and public payloads are signed plaintext. The tx goes as a private message ([reconciliation §4](../../archive/reports/Design%20review%20reconciliation.md); [report, BitChat section](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).
- The archive says the only store-and-forward that survives disconnection is the iOS courier. Superseded: private messages have a sealed on-disk outbox.
- Relay jitter: private directed fragments use 20–60 ms and keep TTL 7 (no dense TTL 5 clamp); 8–25 ms applies to broadcast fragments (`RelayController.swift` L26–66; corrected 26 Oct). Still scheduling arithmetic; measure it.
- The outbox constants were cited as MessageRouter.swift L129–137; the re-check places them at L128–135.

## Open

- [unverified] Whether the 12.3 KB base64 transaction fits a 16 KiB courier envelope once wrapped and sealed. Until checked, do not rely on couriers.
- [unverified] What "acknowledged" means for the outbox: transport `delivered` ack or app-level. B must confirm only after saving, so the app may need its own receipt message.
- [unverified] Outbox behaviour across A's app restart, and the order in which retries fire after a peer reappears.
- [unverified] Real one-hop and three-hop delivery time for the transaction ([MVP open check 9](../../archive/reports/MVP%20v1%20design.md)).
- [unverified] Whether reassembly within the 30 s fragment timeout holds on a busy or marginal link.
- [unverified] Group messages at `04995f68`: the `0x25` API, the 16-member roster, 6 h gossip retention (`GroupProtocol.swift` L350–510, `GossipSyncManager.swift` L73–94, `TransportConfig.swift` L391 as cited by R3), whether a 350–450 B group message reaches 15 peers within 5 s over BLE, and whether group messages are relayed across hops. Needed before the nullifier-sharing test.
- Packet capture: no plaintext amounts, addresses or keys on the BLE link.

### Decisive test

| # | Step | Pass condition |
|---|---|---|
| 1 | A sends the tx to B as a private message | B receives it |
| 2 | B saves the tx, then confirms to A | Confirmation arrives only after the tx is on B's disk |
| 3 | A disappears (app quit, Bluetooth off) | Nothing lost at B |
| 4 | B restarts | Tx still in B's own store (BitChat's memory timeline is gone) |
| 5 | D appears later | B delivers the tx to D as a private message |
| 6 | D submits to the local chain | Node accepts; tx mined |
| 7 | Confirmation returns over the mesh to B, then A | B marks final; A's outbox entry clears |

Variant: B offline when A sends; BitChat's outbox delivers only within 24 h and 8 attempts. Add a >24-hour / exhausted-attempt restart case now that application persistence/re-enqueue is approved (#12); current BitChat alone cannot meet that contract.
