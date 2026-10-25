# Transport: BitChat fork

The MVP transport is a fork of BitChat on three Macs. A sends the finished transaction to B as a Noise-encrypted private message; B saves it and forwards it to C as a second private message; C submits it.
BitChat already has a sealed on-disk sender outbox for private messages, so B can be offline when A sends. It has no other delay-tolerant storage that suits a transaction, so the payee app must persist what it receives.
System view: [../design.md](../design.md). Gateway side: [gateway.md](gateway.md). Later transport: [../later/logos.md](../later/logos.md).

## Decided

- [decided] Three Macs (macOS 13+), payer A, payee B, gateway C, each running a fork of [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat) at `04995f68`. iOS and macOS share one codebase, so the same fork later runs on iPhone (iOS 16+). There is no Linux, Windows or CLI client, which is why the laptops are Macs ([reconciliation G9](../../archive/reports/Design%20review%20reconciliation.md)).
- [decided] The transaction goes **A → B as a private message**: base64 text, about 12.3 KB, under the 60,000-character message limit. Not a file, not a public broadcast.
- [decided] B writes the transaction to its own storage **on receipt**, before confirming to A. BitChat keeps conversations in memory only.
- [decided] B forwards the bytes to C as a **separate private message**. A packet addressed to B stops at B.
- [decided] Relays and the gateway never receive payment intent (invoice, amount, payer signature). Payment details travel only inside A's private message to B.
- [decided] All three Macs use the **same build configuration** (all Debug or all Release), because Debug and Release use different BLE service UUIDs.
- [decided] Txid is the transport identity. Resending the same bytes is harmless.

## Findings

Source: `permissionlesstech/bitchat@04995f68` (Swift, Unlicense), read in code. Line numbers below are from the re-check on 25 October 2026 and supersede those in the archived note.

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

- Maximum message length is 60,000 characters, so a 9,166-byte transaction fits as base64 (about 12.3 KB) in one private message.
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
| Throughput | No measured BLE number found. Fragment pacing caps BitChat at about 469 B / 25 ms ≈ 19 KB/s per hop (inference) | physical_mesh_links.md §1 Gaps; fragment planner above |
| SIG BLE Mesh | 11-byte segments, 384-byte max PDU; unusable for a 9 KB transaction. Does not apply to BitChat, which uses GATT with MTU up to 512 | physical_mesh_links.md §1 |
| Multi-hop latency | Archived estimate "about 1 s over three hops" is scheduling arithmetic, not a measurement | [bitchat_mesh.md §3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/bitchat_mesh.md) |
| Cold start | Discovery, 8 s connect timeout and Noise XX handshake: 2–15 s (estimate) | bitchat_mesh.md §3 |

### Corrections to the archive

- The archived note and report say private text is capped at 255 bytes and recommend sending the transaction as a private **file** or a new public broadcast type `0x2D`. Superseded: the message limit is 60,000 characters, private files are not outboxed, and public payloads are signed plaintext. The tx goes as a private message ([reconciliation §4](../../archive/reports/Design%20review%20reconciliation.md); [report, BitChat section](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).
- The archive says the only store-and-forward that survives disconnection is the iOS courier. Superseded: private messages have a sealed on-disk outbox.
- The archive's ~1 s three-hop figure used 20–60 ms relay jitter; the code's jitter for this case is 8–25 ms. A review found the figure was scheduling arithmetic. Measure it instead.
- The outbox constants were cited as MessageRouter.swift L129–137; the re-check places them at L128–135.

## Open

- [unverified] Whether the 12.3 KB base64 transaction fits a 16 KiB courier envelope once wrapped and sealed. Until checked, do not rely on couriers.
- [unverified] What "acknowledged" means for the outbox: transport `delivered` ack or app-level. B must confirm only after saving, so the app may need its own receipt message.
- [unverified] Outbox behaviour across A's app restart, and the order in which retries fire after a peer reappears.
- [unverified] Real one-hop and three-hop delivery time for the transaction ([MVP open check 9](../../archive/reports/MVP%20v1%20design.md)).
- [unverified] Whether reassembly within the 30 s fragment timeout holds on a busy or marginal link.
- Packet capture: no plaintext amounts, addresses or keys on the BLE link.

### Decisive test

| # | Step | Pass condition |
|---|---|---|
| 1 | A sends the tx to B as a private message | B receives it |
| 2 | B saves the tx, then confirms to A | Confirmation arrives only after the tx is on B's disk |
| 3 | A disappears (app quit, Bluetooth off) | Nothing lost at B |
| 4 | B restarts | Tx still in B's own store (BitChat's memory timeline is gone) |
| 5 | C appears later | B delivers the tx to C as a private message |
| 6 | C submits to the local chain | Node accepts; tx mined |
| 7 | Confirmation returns over the mesh to B, then A | B marks final; A's outbox entry clears |

Variant: B offline when A sends; A's outbox delivers when B appears (within 24 h and 8 attempts).
