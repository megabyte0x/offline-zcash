# BitChat (permissionlesstech) as the phone-to-phone mesh for an offline Zcash MVP

Scope: what the BitChat Bluetooth mesh provides, the exact constants that matter for carrying a 9,166-byte Ironwood transaction plus a receipt between phones, and what a fork must change. The MVP goal is a transaction moving from one phone to another with no internet until some phone with internet broadcasts it, before porting the design to Logos nodes on basecamps.

Source read at: [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat) `04995f68e470a18d1fc99f86fa326c2a87bab4f2` (iOS/macOS, Swift, Unlicense) and [permissionlesstech/bitchat-android](https://github.com/permissionlesstech/bitchat-android) `137ce6d01bba0b92f03ea49a9d6c9d3411a79706` (Kotlin, GPL-3.0). The org also hosts `georelays` (MIT). "Verified" means read in code at those commits; "inferred" means derived from constants.

---

## 1. Takeaway

BitChat can carry a 9,166-byte transaction today, unmodified, as a private file with MIME `application/octet-stream`. It moves in about 20 BLE fragments, roughly half a second per hop on a live link. Its weakness is delay tolerance: fragments are never spooled, reassembly times out after 30 seconds, and the only store-and-forward that survives disconnection is iOS-only, text-only and capped at 16 KiB. A small fork of both clients is needed: a signed public broadcast type for transactions (so any phone with internet can submit them), a Noise payload type for receipts, a broadcast type for block headers, longer reassembly and sync retention, and an app-level persistent store with resend.

---

## 2. Cited findings (verified in code)

**Transport**
- BLE GATT service UUID `F47B5E2D-…-8E1D2C3A4B5C`; iOS DEBUG builds use `…4B5A`, so they do not see Android or release builds. One characteristic `A1B2C3D4-E5F6-4A5B-8C9D-0E1F2A3B4C5D` with notify, write, write-without-response and read. — [BLEService.swift#L186-L191](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEService.swift#L186-L191); [AppConstants.kt#L27-L29](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/util/AppConstants.kt#L27-L29); [PeripheralRole.swift#L37-L45](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEService%2BLinkLayerPeripheralRole.swift#L37-L45)
- Every node is both central and peripheral. Android requests MTU 517; iOS uses `bleMaxMTU = 512` and fragments above `maximumWriteValueLength`. — [GattClientManager.kt#L475](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/mesh/BluetoothGattClientManager.kt#L475); [BLEService.swift#L2125-L2135](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEService.swift#L2125-L2135)
- Android also has Wi-Fi Aware with a hard-coded `PSK = "bitchat_secret"`. No Multipeer or Wi-Fi Direct on iOS. — [WifiAwareMeshService.kt#L69-L70](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/wifi-aware/WifiAwareMeshService.kt#L69-L70)
- Internet side: Nostr kinds 1059/13/14 (private), 20000/20001 (geohash chat and presence), 1401 (`courierDrop`); 5 geo relays. — [NostrProtocol.swift#L22-L36](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Nostr/NostrProtocol.swift#L22-L36); [TransportConfig.swift#L220](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/TransportConfig.swift#L220)
- Gateway mode (iOS only): a mesh-only phone hands a signed Nostr event to a connected peer via `nostrCarrier` (0x28), for public geohash events only. — [GatewayService.swift#L14-L30](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/Gateway/GatewayService.swift#L14-L30)
- Tor via in-process Arti, on by default and fail-closed: "requests queue rather than falling back to clearnet". — [TOR-INTEGRATION.md#L5-L7](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/docs/TOR-INTEGRATION.md#L5-L7)

**Packet format** — [BinaryProtocol.swift#L24-L130](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/BinaryProtocol.swift#L24-L130)
- Header: version, type, TTL, 8-byte timestamp, flags, payload length (2 bytes in v1, 4 in v2), 8-byte sender ID, optional 8-byte recipient, optional route (v2), payload, optional 64-byte Ed25519 signature. Flags: recipient 0x01, signature 0x02, compressed 0x04, route 0x08, RSR 0x10.
- zlib compression above 100 bytes. PKCS#7 padding to 256/512/1024/2048, skipped when more than 255 pad bytes would be needed, so a 9 KB payload is effectively unpadded. — [MessagePadding.swift#L24-L32](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/MessagePadding.swift#L24-L32)
- Receive caps: Noise-encrypted and file transfer 1 MiB, courier envelope 64 KiB, unknown types 128 KiB and still relayed. — [PacketPayloadLimits.swift#L24-L97](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/PacketPayloadLimits.swift#L24-L97)
- Fragments: 13-byte header (8-byte ID, index, total, original type) plus 469-byte chunk; paced 25 ms (directed) or 30 ms (broadcast). Max fragments 10,000 on iOS (256 for private media) and 256 on Android; concurrent assemblies 128 / 64; **assembly timeout 30 s** on both. — [FragmentPlanner.swift#L112-L139](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLEOutboundFragmentPlanner.swift#L112-L139); [TransportConfig.swift#L7-L9](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/TransportConfig.swift#L7-L9); [AppConstants.kt#L38-L45](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/util/AppConstants.kt#L38-L45)

**Routing**
- `messageTTLDefault = 7`. iOS floods with degree-based jitter (10–40 ms at degree ≤2, up to 220 ms when dense); directed packets and handshakes always relay; fragment relays cap TTL at 5 when degree ≥6. Optional source routing with at most 4 intermediate hops, falling back to flooding. — [RelayController.swift#L26-L95](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/RelayController.swift#L26-L95)
- Android relays everything when TTL ≥4 or the network has ≤3 peers; otherwise with probability 1.0 down to 0.4 by network size. — [PacketRelayManager.kt#L143-L165](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/mesh/PacketRelayManager.kt#L143-L165)
- Dedupe by message-ID cache (300 s, 1,000 entries) plus GCS-filter gossip sync. iOS keeps at most 6 central links, Android 8 connections. Announces every 4 s, then 15–30 s with jitter once connected.

**Store-and-forward**
- Sender outbox: 24 h, 100 messages per peer, 8 attempts, up to 3 couriers, persisted encrypted. — [MessageRouter.swift#L129-L137](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/MessageRouter.swift#L129-L137)
- Courier envelopes (iOS only): 16 KiB ciphertext cap ("Couriered messages are text-sized; media transfers are out of scope"), 24 h lifetime, spray-and-wait with at most 8 copies; courier store 40 envelopes with per-depositor quotas. — [CourierEnvelope.swift#L39-L46](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/CourierEnvelope.swift#L39-L46); [CourierStore.swift#L102-L108](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/Courier/CourierStore.swift#L102-L108)
- Relay spool: 60 s, single frames only; "fragments, which are never spooled". — [TransportConfig.swift#L321-L329](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/TransportConfig.swift#L321-L329)
- Android `StoreForwardManager`: 12 h, 100 messages (1,000 for favourites), skips `NOISE_ENCRYPTED` (inferred: does nothing for private messages). — [StoreForwardManager.kt#L54-L62](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/mesh/StoreForwardManager.kt#L54-L62)

**Security**
- `Noise_XX_25519_ChaChaPoly_SHA256`; persistent Ed25519 signing key alongside the static X25519 key; packet signatures exclude TTL and the RSR flag; QR verification valid 5 minutes. Public messages are signed plaintext; private messages are Noise-encrypted. — [NoiseProtocol.swift#L117-L122](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Noise/NoiseProtocol.swift#L117-L122); [NoiseEncryptionService.swift#L158-L160](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/NoiseEncryptionService.swift#L158-L160); [BitchatPacket.swift#L51-L66](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/localPackages/BitFoundation/Sources/BitFoundation/BitchatPacket.swift#L51-L66)
- Android's [security review](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/docs/security-review-jul-27.md) listed key logging (C1, appears fixed), a decompression bomb (C2) and a broken replay window (H1); C2 and H1 status not checked. `BCH-01-004` labels in code suggest an external audit (inferred). Open issues: [android#699](https://github.com/permissionlesstech/bitchat-android/issues/699) (messages recoverable after reset), [bitchat#1618](https://github.com/permissionlesstech/bitchat/issues/1618) (iOS silently drops payloads above 1.13 MiB; Android allows 10 MiB).

**Carrying a binary payload**
- Private text messages cap content at 255 bytes on both platforms. — [Packets.swift#L256-L264](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Protocols/Packets.swift#L256-L264)
- `privateFile` (0x20) wraps a file packet inside Noise; `application/octet-stream` is allowed with no magic-byte check; limits 1 MiB (iOS), about 9.87 MiB (Android). — [MimeType.swift#L102-L118](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/MimeType.swift#L102-L118)
- Unknown inner Noise payload types are logged and dropped, so a new private type needs both clients patched. Unknown outer packet types are still relayed (Android) or relayed up to 128 KiB (iOS), so stock phones would relay a new broadcast type (inferred). — [BLENoisePacketHandler.swift#L225-L228](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/BLE/BLENoisePacketHandler.swift#L225-L228); [PacketProcessor.kt#L139-L171](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/mesh/PacketProcessor.kt#L139-L171)
- Receipts: Noise payload types `delivered` (0x03) and `readReceipt` (0x02), sent automatically. — [BitchatProtocol.swift#L76-L99](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Protocols/BitchatProtocol.swift#L76-L99)

**Background and battery**
- iOS uses `bluetooth-central` and `bluetooth-peripheral` background modes with state restoration; backgrounded iOS moves the advertised UUID into Apple's overflow area, which limits discovery. — [SERVICE-UUID-DETECTABILITY.md#L48-L49](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/docs/SERVICE-UUID-DETECTABILITY.md#L48-L49)
- Scan duty cycle on iOS 5 s on / 10 s off (3/15 when dense), continuous with ≤2 links. Android runs a foreground service with four power modes, from 8 s / 2 s scanning down to 1 s / 29 s at ultra-low.

**Status and interop**
- iOS v1.8.0 (23 Oct 2026), Android v2.0.3 (22 Oct 2026); both active. iOS 16+, macOS 13+, Android API 26+.
- Shared wire format, but Android lacks `courierEnvelope`, `nostrCarrier`, group messages and board posts. — [BinaryProtocol.kt#L13-L22](https://github.com/permissionlesstech/bitchat-android/blob/137ce6d01bba0b92f03ea49a9d6c9d3411a79706/app/src/main/java/com/bitchat/android/protocol/BinaryProtocol.kt#L13-L22)

**Payments precedent**
- Cashu tokens and a `/pay` command shipped; the app "never contacts a mint". A PR adding Lightning/Cashu Noise payload types was closed unmerged. Nothing Zcash-specific beyond a donations request. — [bitchat#1376](https://github.com/permissionlesstech/bitchat/pull/1376); [android#776](https://github.com/permissionlesstech/bitchat-android/pull/776); [CashuTokenDecoder.swift#L5-L8](https://github.com/permissionlesstech/bitchat/blob/04995f68e470a18d1fc99f86fa326c2a87bab4f2/bitchat/Services/CashuTokenDecoder.swift#L5-L8); [bitchat#1053](https://github.com/permissionlesstech/bitchat/pull/1053); [bitchat#903](https://github.com/permissionlesstech/bitchat/issues/903)

---

## 3. Timing estimates (inferred from constants)

- 9,166-byte transaction plus about 100 bytes of TLV, Noise tag and header is about 9,270 bytes, or 20 fragments of 469 bytes. Halo 2 proofs do not compress meaningfully.
- One hop on a connected link: 20 × 25 ms ≈ 0.5 s.
- Three hops: relays forward fragments as they arrive with 20–60 ms jitter each, about 0.7–1 s.
- Cold start dominates: discovery, an 8 s connect timeout and a multi-hop Noise XX handshake give 2–15 s, longer with background duty cycling.
- Each extra 2 actions adds about 14 fragments (≈0.35 s per hop). The iOS courier's 16 KiB cap fails at about 6 actions.

---

## 4. Recommended fork for the MVP (design inference)

| Change | Why |
|---|---|
| New signed public broadcast type, e.g. `0x2D zcashTx` (avoid 0x2A/0x2B courier spray-ack and 0x2C `announceV2`) | Any phone with internet can submit it, not only the named recipient. The transaction is shielded, so broadcasting its bytes reveals no amounts or addresses. Stock phones already relay unknown outer types. |
| Payload cap for the new type in `PacketPayloadLimits`, about 64 KiB | Covers up to about 10 actions |
| New Noise payload type (around 0x30) for the signed payment receipt, or a plain DM under 255 B | Payee → payer acknowledgement with acceptance tier |
| Broadcast type for block headers (1,487 B) and inclusion proofs | Feeds chain state from the internet-connected phone back into the mesh |
| `bleFragmentLifetimeSeconds` 30 s → about 120 s | Multi-hop reassembly on slow or busy links |
| Gossip-sync retention for the new type of hours (like `syncPublicMessageMaxAgeSeconds = 6h`), not 900 s | Late-joining phones still receive the transaction |
| App-level persistent store with resend keyed by txid until mined or expired | BitChat never spools fragments and its courier is iOS-only, text-only and 16 KiB |
| Keep TTL 7 | Sufficient for a single-room or campsite mesh |
| Gateway phone: submit over Tor (Arti is already embedded) to a lightwalletd/Zaino endpoint, then broadcast a signed receipt and later the inclusion proof | Mirrors the basecamp gateway role |

Day-0 alternative with no fork: send the raw transaction to a known peer as a `privateFile` with `application/octet-stream`; the automatic `delivered` ack is the transport-level receipt. Only that peer can then broadcast it.

---

## 5. Risks and open questions

- Delay tolerance: no fragment spooling and a 30 s reassembly timeout mean a multi-hop send needs the whole path at once. The app-level store is mandatory.
- Platform asymmetry: courier and gateway are iOS-only; fragment and payload limits differ; iOS DEBUG builds use a different service UUID.
- iOS background discovery is weak; payments are most reliable with the app in the foreground.
- Android Wi-Fi Aware uses a hard-coded PSK; status of the decompression-bomb and replay-window findings is unchecked.
- Public broadcast of the transaction makes every relaying phone a potential first broadcaster and reveals the sending phone's mesh neighbourhood; peer-ID rotation helps but does not remove timing correlation.
- Licensing: iOS is Unlicense, Android is GPL-3.0, so an Android fork must stay GPL-3.0.
- Phone proving: more than 3 s on iPhone on today's stack; a vendor claims about 200 ms with Zakura. Needs benchmarking with a mobile Zcash SDK that supports Ironwood v6 on post-NU7 testnet, which is itself unverified.
