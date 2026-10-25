# MVP v1 design

Status: agreed 25 October 2026, before implementation. Only the local chain runs today. This file is the system view; each component's details, findings and open items are in [components/](components/). History and full sources are in [../archive/](../archive/README.md).

**Rule: design for mainnet; the local chain is the harness.** Every check is specified for Zcash mainnet under NU7 with an untrusted gateway. The `ths` Regtest chain and the four Macs exist to run that design, not to simplify it. Where Regtest cannot exercise a check (proof of work), an explicit test flag covers the gap and mainnet vectors test the check itself ([chain.md](components/chain.md)). Phases below are a build order, never a security downgrade.

The mesh never moves coins. It moves a finished transaction, already signed and proven, from device to device until one with internet submits it. Value moves only when the transaction is mined.

## Harness: demo setup

Four Macs (macOS 13+), each running the BitChat fork, plus the local chain. A, B and C are offline; only D has internet. C plays two roles: the v1.1 escrow co-signer and, for the nullifier-sharing test, a second payee. On mainnet D is any fleet device with backhaul, and the design runs two of them.

```diagram
┌─────────────────────┐  private msg  ┌─────────────────────┐  private msg  ┌─────────────────────┐
│ A: payer (offline)  │──────────────▶│ B: payee (offline)  │──────────────▶│ D: gateway (online) │
│ offline builder     │ tx + statement│ verifier + pending  │  tx bytes     │ submitter, no keys  │
│ outbox / re-issue   │◀──────────────│ policy, stores tx   │◀──────────────│ signed bundles      │
└──────────┬──────────┘    receipt    └──────────┬──────────┘ headers/proofs└──────────┬──────────┘
           │ redacted PCZT (v1.1)                │ group msg: pending nullifiers        │ sendrawtransaction
           ▼                                     ▼                                      ▼
┌─────────────────────┐                ┌─────────────────────┐               ┌─────────────────────┐
│ C: co-signer (off-  │◀───────────────│ C as 2nd payee      │               │ Local chain: ths    │
│ line) FROST share,  │  same machine  │ (sharing test)      │               │ v0.4.1, NU7 at h=1  │
│ nullifier lock      │                └─────────────────────┘               └─────────────────────┘
└─────────────────────┘
```

| Component | Job | Doc | Status |
|---|---|---|---|
| Chain | Target: mainnet NU7 (`0x77190AD9`). Harness: local `ths` Zakura node, lightwalletd, funded dev accounts | [chain.md](components/chain.md) | Harness running |
| Offline builder | Builds, proves and signs a v6 Ironwood transaction with no network; outputs raw bytes | [offline-builder.md](components/offline-builder.md) | To build (Rust, Zakura Common `=2.2.0`) |
| Transport | BitChat fork: private messages for tx, receipts, signed chain bundles | [transport-bitchat.md](components/transport-bitchat.md) | To build |
| Payee verifier | Offline checks, chain-state checks against PoW and Merkle paths (no trust in the gateway), pending policy, receipts and states | [payee-verifier.md](components/payee-verifier.md) | To build; anchor verifier proven in [`poc/mmr-anchor-poc`](../poc/mmr-anchor-poc/README.md) |
| Pending tier | Device-bound PaymentStatement and Receipt formats, payee-to-payee nullifier sharing, limits | [pending-tier.md](components/pending-tier.md) | Spec decided |
| Gateway | Untrusted. Submits (over Tor on mainnet), rebuilds the history MMR, returns signed bundles: tip, anchor paths, txid branches, conflict notices | [gateway.md](components/gateway.md) | To build; MMR rebuild proven in [`poc/mmr-anchor-poc`](../poc/mmr-anchor-poc/README.md) |
| Double-spend controls | Pending tier now; nullifier-lock co-signer (2-of-2 FROST) in v1.1; recovery in v1.2 | [double-spend.md](components/double-spend.md) | Decided; FROST signing proven in [`poc/escrowcheck`](../poc/escrowcheck/README.md) |
| Privacy | Threat model and leaks | [privacy-and-threats.md](privacy-and-threats.md) | Reference |

## One payment

| # | Step | Network |
|---|---|---|
| 0 | Setup while online: sync, split funds into 10+ notes, exchange BitChat keys, load fleet settings (branch ID, expiry delta, fee, limits) | Once |
| 1 | B sends a ZIP 321 payment request to A | Mesh |
| 2 | A reserves notes and builds the transaction offline | None |
| 3 | A sends the transaction plus its signed PaymentStatement (invoice hash, amount, txid, auth digest, seq) to B as a private message. Relays and D only ever see raw transaction bytes | Mesh |
| 4 | B checks it, posts the pending nullifiers to the fleet-payee group, waits 5 s if another payee is connected, and replies with a signed Receipt: "passes checks at height H; pending" | Mesh |
| 5 | B forwards the bytes to D as a separate private message; D submits and returns a mempool receipt; the node mines | Gateway |
| 6 | A signed bundle returns: tip header, txid branch, anchor paths, conflict notices. B checks the signature, proof of work (mainnet) and the paths, then marks `included`, later `confirmed`. On expiry, A re-issues from the same notes | Mesh |
| 2′ | v1.1, escrow notes: A sends the redacted PCZT to C, which locks the nullifiers and returns its FROST share before A proves and continues at step 3 | Mesh |

## Finalized decisions

Both independent reviews agreed on these; build around them.

- A builds, proves and signs; B checks and stores; D (the gateway) submits bytes and holds no spending or viewing key. The co-signer C (v1.1) holds one FROST share and no viewing key.
- Payer and payee each have a device-bound Ed25519 payment key, enrolled in person; it signs the PaymentStatement and Receipt. The keys and formats are in [pending-tier.md](components/pending-tier.md).
- Txid identifies the payment for transport; resubmission is harmless. Caches key on txid plus auth digest, because a v6 re-anchor keeps the txid.
- A stuck payment is re-issued from the same notes, so at most one version can be mined. Never re-pay from different notes.
- Received funds cannot be re-spent offline; pre-split notes cap the number of offline payments.
- "Pending" is an IOU from a known payer. "Final" only after mining.
- Branch ID, expiry delta and fee come from configuration. Fee is 10,000 zats for 2 actions (what the wallet emits). A transaction unmined at a network upgrade dies and is re-issued.
- Design for mainnet; the local `ths` NU7 chain is the harness. Mainnet transactions only after NU7 activates (height 3,543,405), each send approved.
- The gateway is untrusted (ADV-G). B verifies every chain fact itself: tip by Equihash and difficulty against a pinned floor from a setup checkpoint, anchors by MMR path to header N+1, inclusion by txid branch, conflicts by targeted notices that carry the conflicting tx and its branch. Bundles are signed with the gateway's enrollment payment key. ≥ 2 gateways must agree on the tip; with one, the state is `checked (single-source)` with the stricter cap. Only the Equihash/difficulty/timestamp check is disabled on Regtest, under the tested `regtest-no-pow` flag. Decided 25 October 2026 ([payee-verifier.md](components/payee-verifier.md), [gateway.md](components/gateway.md)).
- Submission over Tor with a fresh circuit per transaction is part of the design, built once the loopback path works.
- Transport: BitChat fork on four Macs for the harness, later iPhones on the same codebase. Logos comes later ([later/logos.md](later/logos.md)).
- FROST quorum is out of the MVP. The next phase is one **nullifier-lock co-signer** component (2-of-2 FROST), deployed first with a mesh co-signer and also, as a second configuration, with the payee as co-signer for recurring pairs ([double-spend.md](components/double-spend.md)).
- The wallet has two kinds of notes: escrow notes (double-spend-proof, need the co-signer) and ordinary notes (pending tier).

## Stages

Stages are a build order. Every stage is designed for mainnet and run on the harness until the mainnet launch.

| Stage | Runs on | Change |
|---|---|---|
| MVP v1 | Harness (local NU7 chain, ths v0.4.1) | Everything above: pending tier with PaymentStatement, Receipt, nullifier sharing, gateway mempool receipt. Chain-state checks in three steps: (1) signed bundle format with every slot, anchor-in-window, expiry margin, txid branch, MMR anchor verifier; (2) Equihash/difficulty checker with the pinned floor and mainnet header vectors; (3) two-gateway agreement and `checked (single-source)` degradation. Tor submission path after loopback works |
| MVP v1.1 | Harness | Nullifier-lock co-signer component; escrow notes; bilateral configuration |
| MVP v1.2 | Harness | Co-signer recovery: 2-of-3 with a fenced cold share, or timelock-encrypted share release |
| Mainnet launch | Mainnet, NU7 from height 3,543,405 | Library step only: Zakura Common 4.x or custom mainnet params; public endpoints over Tor; `regtest-no-pow` off; real ZEC, each send approved. No design change (the earlier "MVP v2" row that deferred Tor and header validation to this stage is superseded) |
| Logos phase | Mainnet | Basecamps and Logos Delivery; Zebra module as gateway; quorum committees |

What to work on next: [open-work.md](open-work.md).
