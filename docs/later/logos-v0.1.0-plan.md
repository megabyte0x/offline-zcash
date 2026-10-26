# Logos pathway v0.1.0: implementation plan (proposed)

**Status update, 26 October 2026 (later):** the owner redefined v0.1.0 as the mesh proof in sandboxes. It was built from this plan's core, roles and tests, using our own mesh crate instead of the Offline Protocol sidecar ([../mvp-v0.1.0.md](../mvp-v0.1.0.md)). Milestones 5–7 below (Logos packaging, Basecamp UI, Macs) are no longer part of v0.1.0.

**Status:** Proposed, 26 October 2026. Owner chose `mesh_offlineprotocol` as the first backend. The design is in [logos-modules.md](logos-modules.md); this file orders the build. Nothing is built yet.

## Goal

Four nodes, each one runtime with our modules: **A** payer, **R** relay, **B** payee, **D** gateway with internet. A pays B offline over two hops (A–R–B). B checks and stores the payment, then sends a receipt to A. B forwards the transaction to D, which submits it to the local `ths` chain and mines it. D then sends the result back to A and B. No node except D reaches the internet or the chain.

**Done when (v0.1.0):** the tests below pass with the modules running under `logosctl` on Linux, and the same payment runs from the `zcash_offline_ui` screens in Basecamp (owner decision, 26 October 2026: the UI is inside v0.1.0). Macs and Bluetooth LE come after (v0.2.0).

## Scope

| In v0.1.0 | Later |
|---|---|
| `zcash-offline-core`: wallet sync (online), offline builder, payee checks, `ozm1` envelope with sealing, payment store, state machine | Pending-tier limits and conflict rules beyond a floor limit; problem #3 families and re-issue |
| Messages `pay`, `rcpt`, `sub`, `res` | `nf`, `evq`, `evr` (nullifier sharing, evidence), co-signer |
| PaymentStatement and Receipt signed with per-device Ed25519 keys; receipt only after fsync | Device-bound key storage (Keychain or Secure Enclave) |
| Gateway: `sendrawtransaction`, `generate 1`, signed `res` with txid, height and block hash ("gateway-reported") | Signed chain bundle with MMR anchor paths and txid branch (open-work tasks 7–9), Tor |
| `mesh_offlineprotocol` over `offline-protocol-service` (TCP peer streams) | Bluetooth LE, `mesh_delivery`, `ble_mesh_module`, phones |
| Headless node binary, then the same code as Logos modules under `logosctl`; `zcash_offline_ui` in Basecamp on Linux | Basecamp on Macs |

## Design choices this plan makes (reversible)

1. **Add `send(peer, envelope)` to `mesh_transport`.** The contract is topic publish/subscribe, but Offline Protocol is addressed messaging with an outbox and retries. Each payment message already has one recipient, so the core calls `send`. `publish` stays for Logos Delivery and returns "unsupported" on this backend. The fleet file maps each device to its `off1…` address.
2. **Keep `ozm1` sealing on.** Offline Protocol already encrypts end to end with MLS. Sealing again costs one X25519 per envelope but keeps the core identical across backends.
3. **The node talks to the sidecar over its local API:** JSON-RPC 2.0 over a WebSocket on a Unix socket. It is the same call set as the C binding, so moving in-process later only changes the adapter.
4. **Own wallet sync, not the wallet DB.** Scan blocks over Zakura RPC into a persisted `ShardTree`, as the expiry research did, but persisted rather than rescanned each time. Build with the Orchard-crate builder plus `TransactionData::from_parts_v6`, which was mined in that research. Then build an unsigned, proven bundle, apply signatures and keep the sighash seam for the co-signer.
5. **Fresh fleet keys, funded by `ths wallet faucet`.** Importing a `ths` dev account key is not needed.

## Layout

```
Cargo.toml                         workspace, Zakura =2.2.0 pins from AGENTS.md
crates/zcash-offline-core/         no Logos or transport code
crates/ozm-node/                   headless binary: roles A, R, B, D; fleet tool
crates/mesh-offlineprotocol/       local-API client implementing the Transport trait
logos/zcash_offline_module/        .lidl + Rust cdylib shell over the core (milestone 5)
logos/mesh_offlineprotocol/        .lidl provider module over the adapter (milestone 5)
logos/zcash_offline_ui/            ui_qml app over zcash_offline_module (milestone 6)
bench/                             scripts that start the sidecars, nodes and checks
```

## Milestones

| # | Work | Where | Done when |
|---|---|---|---|
| 0 | Workspace, pins, fleet tool: per device an Ironwood spending key, an Ed25519 payment key and an `off1` address; one fleet JSON with policy (branch ID, expiry delta, fee, floor limit) | this orb | `cargo build` passes; fleet file generated for A, R, B, D |
| 1 | Sync and builder: scan to tip, persist tree and notes, build a 2-action v6 payment with configured expiry and fee, offline | this orb | A's transaction is accepted by `sendrawtransaction` and mined on `ths` (open-work task 1) |
| 2 | Payee checks and records: parse at the NU7 branch, v6 and Ironwood-only, decrypt own output, batch-validate proofs and signatures, expiry margin, anchor known from B's last sync; PaymentStatement and Receipt; fsync-then-receipt store | this orb | B accepts milestone 1's bytes and refuses each tampered case with its reason code (open-work task 2, test 7 subset) |
| 3 | Envelope and transport: `ozm1` encode, seal and dedup; `mesh-offlineprotocol` client; node roles and resend loop keyed by (txid, auth_digest); D submits, mines, signs `res` | this orb | Happy path runs end to end on loopback, four sidecars in an A–R–B–D chain |
| 4 | Headless bench and tests (below) | this orb | Tests 1–7 pass; timings recorded in [logos-modules.md](logos-modules.md) |
| 5 | Logos packaging: `.lidl` contracts through `lidl check`, `logos-module-builder` with `codegen.rust`, signed `.lgx`, offline `logosctl install`, four runtimes with `--config-dir`; resolve the open items on interface-bound events | **large orb** (Nix build needs about 12 GB) | Tests 1–3 pass again under `logosctl` |
| 6 | `zcash_offline_ui` (`ui_qml`): Status, Pay, Payments, Receive and Settings screens ([logos-modules.md](logos-modules.md)); four Basecamp runtimes (`--user-dir`) in one desktop session | **large orb** | Test 1 run from the screens: Pay on A, the payment appears on B, and both end at `mined`; screenshots of each state; **v0.1.0** tagged |
| 7 | Basecamp on four Macs over Wi-Fi, then Bluetooth LE | owner's Macs | v0.2.0 |

Milestones 1–2 and the adapter half of 3 are independent and can run in parallel.

## Tests that gate v0.1.0

1. **Happy path:** A → R → B. B receipt to A; B → D; D mines; `res` reaches A and B. Record time per step.
2. **B offline:** A sends while B is down; B comes up later and receives it, and the flow completes.
3. **Restart (MVP test 3 on this transport):** B stores, then receipts; A stops; B restarts; D appears later; B delivers, D submits, and the result returns to B (and to A when it returns).
4. **Tampering:** a changed amount, wrong payee, bad signature, wrong branch or expired transaction is refused with its reason; nothing is sent to D.
5. **Duplicates:** the same envelope delivered twice, and the same (txid, auth_digest) in two envelopes, yield one payment.
6. **No plaintext on the wire:** a loopback packet capture shows no amount, address or transaction bytes.
7. **Gateway idempotence:** resubmitting a mined or mempool transaction ends in the same `res`, not an error state.

## Risks and checks first

- **Build on 2 vCPUs and 3 GB.** The Zakura crates compiled here before (about 2.5 min for `escrowcheck`). Watch memory; use `-j1` if linking runs out.
- **Sidecar per node.** The Python `offline-protocol-service` runs next to each node (about 43 MB each). On Macs it needs Python 3.10–3.13 and macOS 14 arm64.
- **Licence.** Offline Protocol is AGPL-3.0-only, so our modules must be AGPL with source shipped to users. [owner noted when choosing the backend]
- **Logos packaging unknowns.** These are carried from [logos-modules.md](logos-modules.md) Open: whether events reach a client bound through an interface, and how a provider declares that it implements a contract. Fallback: a `next_received` poll.
- **Double-spend thread.** The core keeps the two seams (unsigned plus sighash signing, acceptance-policy trait) so its result plugs in without changing the contracts.
