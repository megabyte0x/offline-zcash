# v0.1.0: offline payment over a multi-hop mesh, mined on the local chain

**Historical evidence, not current setup.** Measurements below describe v0.1.0
on 26 October. Current v0.7.7 uses Common 4, public testnet, invite-free wallets
and authenticated acks; plaintext txid/status notices now intentionally leak
metadata that test 6 below did not cover. Use the [current flow](how-it-works.md),
[install guide](basecamp-install.md) and [bench guide](../bench/README.md).

**Status:** built and passing, 26 October 2026. Code: [`crates/`](../crates/), bench: [`bench/`](../bench/README.md).

v0.1.0 combines the two plans written on 26 October:
- From the [Logos pathway plan](../archive/docs-2026-10-27/later/logos-v0.1.0-plan.md): the core crate, the node roles A/R/B/D, sealed `ozm1` envelopes, the four message kinds and the seven gating tests.
- From the [MVP v1 design](design.md) and [transport plan](../archive/docs-2026-10-27/transport-bitchat.md): BitChat mesh semantics, an app retry keyed by txid, A and B both submitting, signed statements and receipts, and the floor limit.

The owner set the target on 26 October: prove that a Zcash payment on the local chain really travels by hopping across a mesh, using several sandboxes or containers in place of radios.

## What runs

```diagram
┌───────────┐  ozm-ar  ┌───────────┐  ozm-rb  ┌───────────┐  ozm-bd  ┌───────────┐ ths-default ┌──────────────┐
│ A payer   │◀────────▶│ R relay   │◀────────▶│ B payee   │◀────────▶│ D gateway │◀───────────▶│ Zakura (ths) │
│ offline   │          │ no keys   │          │ offline   │          │ online    │  RPC        │ NU7 Regtest  │
└───────────┘          └───────────┘          └───────────┘          └───────────┘             └──────────────┘
```

- Each device runs in its own Docker container. Each link is an `--internal` Docker network with no internet, so A reaches B only through R, and only D can reach the chain. The bench checks this before every run.
- A and B sync their wallets once in a one-shot container ("online"). Their daemons then run with no route to the chain.
- **Mesh (`ozm-mesh`).** This is a BitChat-style mesh over TCP links:
  - TTL flooding, 512-byte frames with a per-frame delay to emulate BLE throughput (20 ms per frame in the bench), and de-duplication by message ID.
  - Store-and-forward with an inventory sync whenever a link comes up, plus end-to-end delivery acks.
  - The store is persisted across restarts, and dialing never gives up.
  - The link layer sits behind one function (`run_link` over any byte stream), so a BLE or LAN link can be added without touching routing.
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

1. **Own mesh crate instead of the Offline Protocol sidecar or a BitChat fork.**
   - Real Bluetooth is unavailable in an orb, and the BitChat apps are Swift (CoreBluetooth).
   - Offline Protocol is AGPL-3.0, a Python sidecar, and its custody refuses DMs (see the archived [transport plan](../archive/docs-2026-10-27/transport-bitchat.md)).
   - The crate implements BitChat's semantics (TTL flood, fragments, store-and-forward, acks) in about 1,200 lines of Rust, under MIT/Apache-2.0.
   - A BitChat-compatible packet layout and a BLE link stay open for later.
2. **No Logos packaging in v0.1.0.** The owner redefined v0.1.0 on 26 October as the mesh proof in sandboxes. Basecamp modules and the QML UI move to a later version.
3. **Own wallet sync.** The wallet scans `getblock`/`getrawtransaction`, keeps every commitment and the root at each height, and rebuilds witnesses offline. It does not use the wallet DB.
4. **Both A and B submit to every gateway** (transport decision 26 October). The gateway mines one block per submission on Regtest (`ths` issue 7).
5. **Gateway results are "gateway-reported".** A and B accept D's signed result without chain proofs. Signed chain bundles are [open-work](open-work.md) tasks 7–9.

## Limits (not fixed in v0.1.0)

- The mesh is emulated over TCP between containers on one host. No radio is involved.
- Unmined change and received funds can't be spent offline until mined and synced again.
- Double-spend protection covers only the floor limit (0.5 ZEC), per-payer sequence numbers, and a conflict check against the payee's own pending payments. Nullifier sharing, the co-signer group and chain bundles are not built.
- Keys are stored as plain JSON files (disposable Regtest keys).
- On a reorg the wallet rescans from scratch but keeps its offline locks.
