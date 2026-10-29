# v0.1.0: offline payment over a multi-hop mesh, mined on the local chain

**Historical results: v0.1.0, 26 October 2026, containers—not radios.**
See the [current flow](how-it-works.md), [setup](basecamp-install.md) and
[bench guide](../bench/README.md). Current plaintext txid/status notices leak
metadata not covered by test 6 below.

This release combined the [Logos pathway plan](../archive/docs-2026-10-27/later/logos-v0.1.0-plan.md)
and [transport plan](../archive/docs-2026-10-27/transport-bitchat.md): sealed
envelopes, BitChat-style routing, app retries, dual submission and signed receipts.
The target was a real local-chain payment carried across a multi-hop container mesh.

## What runs

```diagram
┌───────────┐  ozm-ar  ┌───────────┐  ozm-rb  ┌───────────┐  ozm-bd  ┌───────────┐ ths-default ┌──────────────┐
│ A payer   │◀────────▶│ R relay   │◀────────▶│ B payee   │◀────────▶│ D gateway │◀───────────▶│ Zakura (ths) │
│ offline   │          │ no keys   │          │ offline   │          │ online    │  RPC        │ NU7 Regtest  │
└───────────┘          └───────────┘          └───────────┘          └───────────┘             └──────────────┘
```

- Devices occupy separate Docker containers on `--internal` networks. Only D
  reaches the chain; A reaches B only through R. The bench checks isolation.
- A and B sync once online, then run without a route to the chain.
- **Mesh (`ozm-mesh`):** TTL flooding, message-ID dedupe, 512-byte TCP frames
  delayed 20 ms to model BLE throughput, persisted store-and-forward, link-up
  inventory and end-to-end acks. Dialing retries indefinitely; `run_link` accepts
  any byte stream.
- **Core (`zcash-offline-core`):**
  - Wallet sync over Zakura RPC, persisted.
  - Offline builder on Zakura Common `=2.2.0`: one or two notes, two actions, fleet fee, fleet expiry, Ironwood v3 bundle, `from_parts_v6`.
  - Payee checks.
  - Ed25519-signed statements, receipts and gateway results.
  - `ozm1` sealing: X25519 derived from the Ed25519 key, ChaCha20-Poly1305, padding to 4 KiB.
- **Node (`ozm-node`):** a daemon with the four roles, a JSON payment store fsynced before any receipt leaves the device, an app retry loop, a local HTTP API and a minimal web UI.

## One payment (measured, test 1)

| Time after `pay` | Device | Step |
|---|---|---|
| +0.55 s | A | Built and proved offline: 9,166 bytes, proving 373 ms. Sealed `pay` is 12,353 bytes |
| +1.57 s | B | Received `pay` after **2 hops** (A→R→B) |
| +1.69 s | B | Checked offline in 110 ms; fsynced; state `accepted-pending` |
| +2.03 s | A | Signed receipt from B (2 hops back) |
| +2.40 s | D | `sub` from B (1 hop) |
| +2.67 s | D | Checked proofs, `sendrawtransaction`, `generate 1`: **mined at height 143** |
| +3.04 s / +3.19 s | B / A | Signed gateway result, state `mined` |

A also submits through the mesh (3 hops). The gateway answers it with the same mined result.

## Gating tests (all pass: `bench/mesh-bench.sh all`, 23 checks)

| # | Test | Result |
|---|---|---|
| — | A, R, B cannot reach the chain; A has no direct link to B; D reaches the chain | pass |
| 1 | Happy path A→R→B; receipt; B→D; mined; result reaches A and B; chain confirms the height; `pay` crossed 2 hops | pass, 3.6 s end to end |
| 2 | B offline when A pays; R holds the packets; B returns and receives; mined | pass, 7.2 s including B's restart |
| 3 | No gateway; B stores and receipts; A stops; B restarts and keeps the payment; D appears; mined; result reaches B, then A when A returns | pass |
| 4 | Tampered amount (code 11), wrong payee (4), bad signature (5), wrong branch (1, refused at parse), expired (7) each refused; **nothing reaches D** | pass |
| 5 | Same txid in new envelopes: one payment; the duplicates are answered with the stored receipt | pass |
| 6 | Packet capture on R: no 24-byte chunk of the transaction, txid, payee address or amount appears in plaintext | pass |
| 7 | Resubmitting a mined transaction: the gateway reports the same mined height and A stays `mined` | pass |

Reason codes are in [`verify.rs`](../crates/zcash-offline-core/src/verify.rs).

## Decisions made for v0.1.0 (reversible)

1. **Own mesh:** about 1,200 Rust lines, MIT/Apache-2.0. Orbs lack Bluetooth;
   BitChat used CoreBluetooth. Offline Protocol was an AGPL Python sidecar whose
   custody refused DMs ([transport research](../archive/docs-2026-10-27/transport-bitchat.md)).
   BLE and BitChat-compatible packets were deferred.
2. **Defer Logos packaging:** v0.1.0 was the container proof; Basecamp/QML came later.
3. **Own wallet sync.** The wallet scans `getblock`/`getrawtransaction`, keeps every commitment and the root at each height, and rebuilds witnesses offline. It does not use the wallet DB.
4. **Both A and B submit to every gateway** (transport decision 26 October). The gateway mines one block per submission on Regtest (`ths` issue 7).
5. **Gateway results are "gateway-reported".** A and B accept D's signed result without chain proofs. Signed chain bundles are [open-work](open-work.md) tasks 7–9.

## Limits (not fixed in v0.1.0)

- The mesh is emulated over TCP between containers on one host. No radio is involved.
- Unmined change and received funds can't be spent offline until mined and synced again.
- Double-spend protection covers only the floor limit (0.5 ZEC), per-payer sequence numbers, and a conflict check against the payee's own pending payments. Nullifier sharing, the co-signer group and chain bundles are not built.
- Keys are stored as plain JSON files (disposable Regtest keys).
- On a reorg the wallet rescans from scratch but keeps its offline locks.
