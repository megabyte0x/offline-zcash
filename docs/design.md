# System design

The payer builds, proves and signs locally. The mesh carries the transaction to
the payee and gateway; value moves on chain, not on delivery.

**v0.7.7 supports public testnet and Regtest only.** It trusts gateway chain
reports. The mainnet design below requires independent proofs and double-spend
controls that are not built. [Payment flow](how-it-works.md).

## Current system

Basecamp runs one shared `zcash_offline_module` engine and three thin QML apps:
wallet, Organiser and Gateway. CLI and Android use the same node library.
Every device pays, receives and relays. Organiser exports a gateway invite;
ordinary wallets generate local keys without invites. Installing the Gateway
app does not grant gateway authority. `fleet-init` still supports named invites.

| Component | Shipped behavior | Still missing |
|---|---|---|
| [Chain](components/chain.md) | NU7 public testnet, configurable TLS lightwalletd; `ths` v0.4.1 for local Regtest | Mainnet support and independent chain evidence |
| [Builder](components/offline-builder.md) | Common 4.0.0 v6 Ironwood proving, configured expiry, ZIP 317 fee, durable reservations | Phone proving measurements, escrow signing and automatic repair |
| [Wallet sync](components/mesh.md#v070-compact-sync) | Per-device scan starts; local compact scanning; 200 checkpoints; deep-fork recovery; sync on startup, discovery and tips | Authenticated history/frontier proofs and immediate spending of unscanned receipts |
| [Transport](components/mesh.md) | `ozm-mesh` flooding, sealed `ozm1`, acks, finite store-and-forward, TCP/LAN discovery; Android/Linux BLE links implemented | Physical-radio gates, macOS BLE and iOS |
| [Payee](components/payee-verifier.md) | Proof/signature/output checks, local anchor/expiry/conflict policy, durable catch-up queue and incoming notes | Independent anchor/inclusion proofs, invoices and shared nullifier evidence |
| [Gateway](components/gateway.md) | Bounded raw-byte submission, exact-byte retry, signed nonce/status notices, compact sync and tips | Tor, proof bundles and independently verified chain selection |
| [Pending tier](components/pending-tier.md) | No open/unknown pending acceptance; invited-payer local caps | Device-bound keystore enrollment, binary v1 protocol and nullifier sharing |
| [Double-spend controls](components/double-spend.md) | Local reservations and conflict checks; no prevention against a dishonest owner | Payer + 3-of-4 co-signer group, durable exclusive locks and recovery |
| [Expiry/repair](components/expiry-reissue.md) | Configured finite expiry; exact bytes retained; local scan past expiry unlocks unspent notes | Payment families, safe replacement and signer fences |

See [tested cases and platform gaps](open-work.md#built). Releases publish only
Basecamp packages; CLI/APK downloads stop at v0.7.3.

## One payment

1. **Prepare online:** sync and obtain mined notes. Automatic splits must mine
   and be scanned before offline use; unmined change is not spendable.
2. **Build offline:** reserve notes, prove and sign with configured branch,
   expiry and action-count fee.
3. **Deliver and submit:** seal the transaction and statement to the payee;
   submit raw bytes independently, without waiting for a receipt.
4. **Check and save:** the payee authenticates and checks against its scanned
   view, or durably queues verification during catch-up. Only known invited
   payers can receive pending acceptance. The payee also submits the same bytes.
5. **Scan after inclusion:** the gateway reports `included` at 1 and `mined` at
   10 confirmations. Neither is independently proved or final. Incoming notes
   need scanned positions/witnesses before spending. Refusal cannot cancel a tx.

## Finalized decisions

- **Local secrets and proving.** Gateways receive no peer spending/viewing keys
  or proving material. Current gateways also have their own wallet; separating
  broadcaster identity is unfinished.
- **Gateway trust.** Invited wallets trust their named gateway; invite-free
  wallets pin the first used key. First contact can be hijacked.
  [Threat model](privacy-and-threats.md#trust-assumptions-at-v070).
- **Mainnet chain evidence (unbuilt).** PoW/difficulty from a pinned checkpoint,
  MMR anchor paths, txid branches and independent gateways. Sampled headers are
  a heuristic; `regtest-no-pow` is a planned harness exception.
  [Verifier](components/payee-verifier.md).
- **Persist before sending.** Save reservations before transmission and payee
  state before receipts. Delivery acks are not acceptance. Planned variant
  dedupe uses `(txid, auth_digest)`; current payment records use txid.
- **Repair without paying twice (unbuilt).** Retry identical bytes. Replacement
  families use the same real notes and count one obligation once. Expiry does
  not erase accepted debt; never repair with unrelated notes.
- **Offline prevention (unbuilt).** Above the chosen floor, require payer plus
  3 of 4 co-signers and durable exclusive nullifier locks. FROST PoCs do not
  implement this protocol. Ordinary pending payments remain IOUs.
- **Privacy.** Configure branch, expiry and fee. Tor with per-transaction
  isolation and encrypted result notices remain mainnet gates. Resource limits
  do not guarantee availability.

## Stages

| Stage | State |
|---|---|
| Local proof of concept | v0.1–0.6 established offline building, multi-hop TCP, receipts, restart and LAN/invite-free operation on `ths` |
| Public-testnet experiment | v0.7 uses Common 4.0 and TLS lightwalletd; v0.7.5 adds readiness/recovery, v0.7.6 notice/store and session fixes, v0.7.7 gateway responsiveness. Chain reports remain trusted |
| Complete MVP v1 | Finish pending enrollment/nullifier sharing, chain proof bundles and Tor; satisfy [gating tests](open-work.md#tests-that-gate-mvp-v1) |
| MVP v1.1 / v1.2 | Build and test the co-signer group, then safe group replacement; designs/PoCs only |
| Mainnet | Unsupported. Common 4 already knows NU7 at 3,543,405, but activation parameters alone do not close proof, security, hardware or real-value approval gates |

[Open work](open-work.md) separates design decisions from the build backlog.
The [archive](../archive/README.md) retains superseded BitChat-fork and full-node plans.
