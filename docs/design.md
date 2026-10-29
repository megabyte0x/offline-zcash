# System design

The mesh moves a finished Zcash transaction, not coins or spending keys. The
payer builds, proves and signs locally; the payee checks and stores; a gateway
submits the same bytes. Value moves on the selected chain, not on delivery.

**Current application: v0.7.7, public testnet and Regtest only.** The intended
mainnet system treats gateways as untrusted, but its independent chain proofs
and double-spend controls are not all built. Do not read a design decision below
as a security property of the shipped app. [User-facing flow](how-it-works.md).

## Current system

Basecamp runs one shared `zcash_offline_module` engine and three thin QML apps:
wallet, Organiser and Gateway. The CLI and Android shell use the same node
library. Every device pays, receives and relays; **gateway is the only role**.
Installing an app never grants it. Organiser now exports one gateway invite;
wallets normally generate their own keys without invites. Named invited devices
remain available through `fleet-init`.

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

TCP/LAN and funded Linux Basecamp paths have dated evidence. Current Mac,
physical-radio and Android-screen verification remain open. Releases publish
only Basecamp packages; CLI/APK downloads stop at v0.7.3.

## One payment

1. **Prepare while connected.** Create local keys, sync and obtain mined notes.
   Economical note splitting runs automatically; wait for mining and scanning
   before going offline. Never spend unmined change.
2. **Build offline.** Alice reserves notes and builds/proves/signs the v6
   transaction with configured branch, expiry and action-count fee.
3. **Deliver and submit independently.** Alice seals transaction + signed
   statement to Bob and submits raw bytes to the gateway without waiting for a
   receipt. An off-mesh destination still submits without a payee receipt.
4. **Check and persist.** Bob authenticates the sender/statement and verifies
   against his scanned chain view. Catch-up can defer verification durably.
   An open/unknown payer waits for inclusion; a known invited payer can receive
   a pending receipt under local caps. Bob also submits the original bytes.
5. **Report and scan.** The testnet gateway reports inclusion at 1 and mined at
   10 confirmations, then wallets sync. Incoming outputs stay non-spendable
   until scanning supplies positions and witnesses. Refusal never cancels a
   signed transaction. Ten confirmations are not finality.

## Finalized decisions

These are the retained design requirements, with their implementation boundaries.

- **Local secrets and proving.** Gateways receive no peer spending/viewing keys
  or private proving material. The current gateway may also operate its own
  wallet; separating broadcaster identity remains privacy work.
- **Trust is explicit.** Invited devices trust their named gateway; invite-free
  devices pin the first used gateway key and block silent changes. First contact
  can be hijacked. Signed reports and reachable-source consistency do not prove
  chain truth. See [threat model](privacy-and-threats.md#trust-assumptions-at-v070).
- **Mainnet chain evidence.** The design requires PoW/difficulty checks from a
  pinned checkpoint, MMR anchor paths, txid inclusion branches and independent
  gateways. Sampled-header security is a labelled heuristic. These checks are
  unbuilt; `regtest-no-pow` is a planned explicit harness exception, not a shipped
  validator switch. [Verifier](components/payee-verifier.md).
- **Durability before release.** Persist reservations and payment state before
  sending; persist on the payee before a receipt. Delivery acks alone do not
  establish durable payment acceptance. Future replacement variants must dedupe
  by `(txid, auth_digest)`; current payment records are keyed by txid.
- **No double payment as repair.** Retry identical bytes. The approved family
  design repairs with the same real notes and counts one obligation once; never
  repay with unrelated notes. Family accounting and automatic renewal remain
  unbuilt. Expiry does not erase an accepted debt.
- **Conditional offline prevention.** Native ZEC without trusted hardware needs
  split spending authority: payer plus any 3 of 4 co-signers above the chosen
  floor, with exclusive durable nullifier locks. FROST research is not a deployed
  co-signer protocol. Ordinary pending payments remain IOUs.
- **Configuration and privacy.** Branch, expiry and fee come from policy. Tor
  with per-transaction isolation and encrypted result notices are mainnet gates,
  not current features. Finite resource limits do not guarantee availability.

## Stages

| Stage | State |
|---|---|
| Local proof of concept | v0.1–0.6 established offline building, multi-hop TCP, receipts, restart and LAN/invite-free operation on `ths` |
| Public-testnet experiment | v0.7 uses Common 4.0 and TLS lightwalletd; v0.7.5 adds readiness/recovery, v0.7.6 notice/store and session fixes, v0.7.7 gateway responsiveness. Chain reports remain trusted |
| Complete MVP v1 | Finish pending enrollment/nullifier sharing, chain proof bundles and Tor; satisfy [gating tests](open-work.md#tests-that-gate-mvp-v1) |
| MVP v1.1 / v1.2 | Build and test the co-signer group, then safe group replacement; designs/PoCs only |
| Mainnet | Unsupported. Common 4 already knows NU7 at 3,543,405, but activation parameters alone do not close proof, security, hardware or real-value approval gates |

The design order and implementation backlog are separate in [open-work.md](open-work.md).
Historical BitChat-fork and full-node gateway plans remain in the read-only
[archive](../archive/README.md); the current transport is our own `ozm-mesh` and
the testnet gateway is a light client.
