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
- **Links today:** TCP between neighbours. LAN discovery uses UDP beacons on port 7778, sent on every interface as directed broadcast, multicast `239.255.77.77` and `255.255.255.255`, and tagged with the fleet. It works on a Wi-Fi with no internet route. The link layer is a single function over any byte stream, so a BLE link plugs in without touching routing.
- **App envelope `ozm1`:** every message is sealed to its recipient's X25519 key (derived from the Ed25519 payment key) with ChaCha20-Poly1305, and padded to a multiple of 4 KiB. A relay or a packet capture sees only the recipient's mesh ID and a size bucket. Test 6 checks this with a capture.
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

- 9,166-byte transaction → 12,353-byte `pay` envelope. With 20 ms per 512-byte frame, the full A→R→B→D→chain→A and B loop takes about 3.6 s ([../mvp-v0.1.0.md](../mvp-v0.1.0.md)).
- An internal Docker network, like an offline Wi-Fi, has no default route. Discovery beacons must therefore go out per interface, not to the default route; this was found and fixed in v0.2.0.

## Open

- A BLE link (iOS/macOS CoreBluetooth, Android) and real-radio timings. Nothing has run over radio yet.
- Interoperating with BitChat's packet format, if mixed fleets are wanted.
- Discovery beacons announce the mesh ID and fleet tag in clear on the LAN ([../privacy-and-threats.md](../privacy-and-threats.md)).
- Nullifier sharing between payees (`nf`), and the evidence messages (`evq`/`evr`).
