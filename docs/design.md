# MVP v1 design

Status: agreed 25 October 2026, before implementation. Only the local chain runs today. This file is the system view; each component's details, findings and open items are in [components/](components/). History and full sources are in [../archive/](../archive/README.md).

The mesh never moves coins. It moves a finished transaction, already signed and proven, from device to device until one with internet submits it. Value moves only when the transaction is mined.

## Demo setup

Three Macs (macOS 13+), each running the BitChat fork, plus the local chain:

```diagram
┌─────────────────────┐  private msg  ┌─────────────────────┐  private msg  ┌─────────────────────┐
│ A: payer (offline)  │──────────────▶│ B: payee (offline)  │──────────────▶│ C: gateway (online) │
│ offline builder     │  tx + details │ verifier + pending  │  tx bytes     │ submitter           │
│ outbox / re-issue   │◀──────────────│ policy, stores tx   │◀──────────────│ chain bundles       │
└─────────────────────┘    receipt    └─────────────────────┘ headers/proofs└──────────┬──────────┘
                                                                                       │ sendrawtransaction
                                                                                       ▼
                                                                            ┌─────────────────────┐
                                                                            │ Local chain: ths    │
                                                                            │ v0.4.1, NU7 at h=1  │
                                                                            └─────────────────────┘
```

| Component | Job | Doc | Status |
|---|---|---|---|
| Local chain | Zakura node on NU7 (`0x77190AD9`), lightwalletd, funded dev accounts | [chain.md](components/chain.md) | Running |
| Offline builder | Builds, proves and signs a v6 Ironwood transaction with no network; outputs raw bytes | [offline-builder.md](components/offline-builder.md) | To build (Rust, Zakura Common `=2.2.0`) |
| Transport | BitChat fork: private messages for tx, receipts, chain bundles | [transport-bitchat.md](components/transport-bitchat.md) | To build |
| Payee verifier | Offline checks, pending policy, receipts and states | [payee-verifier.md](components/payee-verifier.md) | To build |
| Gateway | Submits, returns inclusion proofs and headers | [gateway.md](components/gateway.md) | To build |
| Double-spend controls | Pending tier now; bilateral escrow next | [double-spend.md](components/double-spend.md) | Policy only |
| Privacy | Threat model and leaks | [privacy-and-threats.md](privacy-and-threats.md) | Reference |

## One payment

| # | Step | Network |
|---|---|---|
| 0 | Setup while online: sync, split funds into 10+ notes, exchange BitChat keys, load fleet settings (branch ID, expiry delta, fee, limits) | Once |
| 1 | B sends a ZIP 321 payment request to A | Mesh |
| 2 | A reserves notes and builds the transaction offline | None |
| 3 | A sends the transaction plus payment details (invoice, amount, accountability signature) to B as a private message. Relays and C only ever see raw transaction bytes | Mesh |
| 4 | B checks it and replies "passes checks at height H; pending" | Mesh |
| 5 | B forwards the bytes to C as a separate private message; C submits; the node mines | Gateway |
| 6 | Inclusion proof and headers return; B marks it final. On expiry, A re-issues from the same notes | Mesh |

## Finalized decisions

Both independent reviews agreed on these; build around them.

- A builds, proves and signs; B checks and stores; C submits bytes and holds no spending or viewing key.
- Txid identifies the payment for transport; resubmission is harmless. Caches key on txid plus auth digest, because a v6 re-anchor keeps the txid.
- A stuck payment is re-issued from the same notes, so at most one version can be mined. Never re-pay from different notes.
- Received funds cannot be re-spent offline; pre-split notes cap the number of offline payments.
- "Pending" is an IOU from a known payer. "Final" only after mining.
- Branch ID, expiry delta and fee come from configuration. Fee is 10,000 zats for 2 actions (what the wallet emits). A transaction unmined at a network upgrade dies and is re-issued.
- Local `ths` NU7 chain first; mainnet only after NU7 activates (height 3,543,405), each send approved.
- Transport for the demo: BitChat fork on three Macs. Logos comes later ([later/logos.md](later/logos.md)).
- FROST quorum is out of the MVP; bilateral 2-of-2 escrow is the next phase.

## Stages

| Stage | Network | Change |
|---|---|---|
| MVP v1 | Local NU7 chain, ths v0.4.1 | Everything above, pending tier only |
| MVP v1.1 | Same | Bilateral payer–payee FROST escrow |
| MVP v2 | Mainnet, NU7 from height 3,543,405 | Zakura Common 4.x or custom mainnet params; gateway over Tor; real ZEC, each send approved |
| Logos phase | Mainnet | Basecamps and Logos Delivery; Zebra module as gateway; quorum committees |

What to work on next: [open-work.md](open-work.md).
