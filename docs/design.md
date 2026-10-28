# MVP v1 design

Status: 28 October 2026. Built and released: v0.1.0 (multi-hop mesh on four containers), v0.2.0 (installable `ozm-node`, LAN discovery, sync and test funds over the mesh) and v0.3.0 (Logos Basecamp module and app) on the local NU7 chain, through **v0.7.4**: public testnet with a TLS lightwalletd light-client gateway whose `included`/`mined` reports are trusted, not proved ([open-work.md](open-work.md#built)). In the built app every device pays, receives and relays; `gateway` is the only device role. Payer, payee and relay below name a device's part in one payment, not a setting. The design below targets mainnet; most chain-trust and double-spend parts are designed but not built. #1 re-decided 26 October (pending below the floor, payer + 3-of-4 co-signer group above it); #2 settled 25 October; #3 decided 26 October for a minimal build.

**Rule: design for mainnet; the local chain is the harness.** Every check is specified for Zcash mainnet under NU7 with an untrusted gateway. The `ths` Regtest chain and the four Macs exist to run that design, not to simplify it. Where Regtest cannot exercise a check (proof of work), an explicit test flag covers the gap and mainnet vectors test the check itself ([chain.md](components/chain.md)). Phases below are a build order, never a security downgrade.

The mesh never moves coins. It moves a finished transaction, already signed and proven, from device to device until one with internet submits it. Value moves only when the transaction is mined.

## Harness: demo setup

Four Macs (macOS 13+), each running the BitChat fork, plus the local chain. A, B and C are offline; only D has internet. C is the second payee for the nullifier-sharing test. In v1.1 four co-signer processes run across the Macs (two on one Mac for the demo only). On mainnet D is any fleet device with backhaul, and the design runs two of them.

```diagram
┌─────────────────────┐  private msg  ┌─────────────────────┐  private msg  ┌─────────────────────┐
│ A: payer (offline)  │──────────────▶│ B: payee (offline)  │──────────────▶│ D: gateway (online) │
│ offline builder     │ tx + statement│ verifier + pending  │  tx bytes     │ submitter, no keys  │
│ outbox / re-issue   │◀──────────────│ policy, stores tx   │◀──────────────│ signed bundles      │
└──────────┬──────────┘    receipt    └──────────┬──────────┘ headers/proofs└──────────┬──────────┘
           │ redacted PCZT (v1.1)                │ group msg: pending nullifiers        │ sendrawtransaction
           ▼                                     ▼                                      ▼
┌─────────────────────┐                ┌─────────────────────┐               ┌─────────────────────┐
│ Co-signers (v1.1):  │                │ C as 2nd payee      │               │ Local chain: ths    │
│ any 3 of 4 + A,     │                │ (sharing test)      │               │ v0.4.1, NU7 at h=1  │
│ nullifier locks     │                └─────────────────────┘               └─────────────────────┘
└─────────────────────┘
```

| Component | Job | Doc | Status |
|---|---|---|---|
| Chain | Target: mainnet NU7 (`0x77190AD9`). Harness: local `ths` Zakura node, lightwalletd, funded dev accounts | [chain.md](components/chain.md) | Harness running |
| Offline builder | Builds, proves and signs a v6 Ironwood transaction with no network; outputs raw bytes | [offline-builder.md](components/offline-builder.md) | Built (Rust); Zakura Common `=4.0.0` since v0.7.0 |
| Transport | `ozm-mesh`: BitChat-style flooding, store-and-forward and acks over TCP links with LAN discovery; sealed `ozm1` envelopes. Android and Linux BLE links built | [mesh.md](components/mesh.md) | Built over TCP/LAN; no real-radio test yet |
| Payee verifier | Offline checks, chain-state checks against PoW and Merkle paths (no trust in the gateway), pending policy, receipts and states | [payee-verifier.md](components/payee-verifier.md) | To build; anchor verifier proven in [`poc/mmr-anchor-poc`](../poc/mmr-anchor-poc/README.md) |
| Pending tier | Device-bound PaymentStatement and Receipt formats, payee-to-payee nullifier sharing, limits | [pending-tier.md](components/pending-tier.md) | Spec decided |
| Gateway | Untrusted. Submits (over Tor on mainnet), rebuilds the history MMR, returns signed bundles: tip, anchor paths, txid branches, conflict notices | [gateway.md](components/gateway.md) | To build; MMR rebuild proven in [`poc/mmr-anchor-poc`](../poc/mmr-anchor-poc/README.md) |
| Double-spend controls | Pending below the floor; payer + 3-of-4 co-signer group (stock weighted FROST) above it in v1.1; reconfiguration in v1.2 | [double-spend.md](components/double-spend.md) | Decided 26 Oct; weighted group signing mined on `ths` (research) |
| Expiry and re-issue | Settlement lifetime, families, repair, re-anchoring and signer fence | [expiry-reissue.md](components/expiry-reissue.md) | Decided 26 Oct for a minimal build; not implemented |
| Privacy | Threat model and leaks | [privacy-and-threats.md](privacy-and-threats.md) | Reference |

## One payment

| # | Step | Network |
|---|---|---|
| 0 | Setup while online: sync, split funds into 10+ notes, exchange BitChat keys, load fleet settings (branch ID, expiry delta, fee, limits) | Once |
| 1 | B sends a ZIP 321 payment request to A | Mesh |
| 2 | A reserves notes and builds the transaction offline | None |
| 3 | A sends the transaction plus its signed PaymentStatement (invoice hash, amount, txid, auth digest, seq) to B as a private message. Relays and D only ever see raw transaction bytes | Mesh |
| 4 | B checks it, posts the pending nullifiers to the fleet-payee group, waits 5 s if another payee is connected, and replies with a signed Receipt: "passes checks at height H; pending" | Mesh |
| 5 | A and B both send the bytes to D as separate private messages; D broadcasts its signed ack and tip to the mesh; D submits and returns a mempool receipt; the node mines | Gateway |
| 6 | A signed bundle returns: tip header, txid branch, anchor paths, conflict notices. B verifies signature, PoW (mainnet) and paths, then `included`, later `confirmed` (ten is provisional demo policy). Expiry is not proof of non-inclusion; exact repair/debt states await problem #3 | Mesh |
| 2′ | v1.1, escrow notes above the floor: A sends the redacted PCZT to nearby co-signers; any 3 of 4 lock the nullifiers and return FROST shares before A continues at step 3 | Mesh |

## Finalized decisions

Both independent reviews agreed on these; build around them.

- A builds, proves and signs; B checks and stores; D (the gateway) submits bytes and holds no spending or viewing key. Each v1.1 co-signer holds one FROST share and no viewing key.
- Payer and payee each have a device-bound Ed25519 payment key, enrolled in person; it signs the PaymentStatement and Receipt. The keys and formats are in [pending-tier.md](components/pending-tier.md).
- Exact-byte resubmission is idempotent. Caches/delivery dedupe use **(txid, auth_digest)** because a v6 re-anchor keeps txid. One-obligation payment families are proposed, not ratified (#3).
- A stuck payment uses the same real notes, so at most one conflicting version can mine on **one valid chain**, not across orphaned histories. Never re-pay from unrelated notes. Canonical-effects/family-wide rules and reservation resolution await #3.
- Received funds cannot be re-spent offline; pre-split notes cap the number of offline payments.
- "Pending" is an IOU from a known payer. "Final" only after mining.
- Branch ID, expiry delta and fee come from configuration. Fee is 10,000 zats for 2 actions (what the wallet emits). A transaction unmined at a network upgrade dies and is re-issued.
- Design for mainnet; the local `ths` NU7 chain is the harness. Mainnet transactions only after NU7 activates (height 3,543,405), each send approved.
- The gateway is untrusted (ADV-G). B verifies every chain fact itself: tip by Equihash and difficulty against a pinned floor from a setup checkpoint, anchors by MMR path to header N+1, inclusion by txid branch, conflicts by targeted notices that carry the conflicting tx and its branch. Bundles are signed with the gateway's enrollment payment key. ≥ 2 gateways must agree on the tip; with one, the state is `checked (single-source)` with the stricter cap. Only the Equihash/difficulty/timestamp check is disabled on Regtest, under the tested `regtest-no-pow` flag. Decided 25 October 2026 ([payee-verifier.md](components/payee-verifier.md), [gateway.md](components/gateway.md)).
- Submission over Tor with a fresh circuit per transaction is part of the design, built once the loopback path works.
- Transport: our own `ozm-mesh` (BitChat semantics) over TCP/LAN today, a BLE link later; phones after laptops, iOS or Android first (not decided). The same device also runs inside Logos Basecamp as a core module and QML app ([../logos/README.md](../logos/README.md)).
- Only key splitting stops A's online sweep with native ZEC and no special hardware. The next phase is the **co-signer group**: A plus any 3 of 4 co-signers near the payer; co-signers alone cannot sign. The payee as sole co-signer stays a configuration for recurring pairs ([double-spend.md](components/double-spend.md)).
- The app persists and re-queues every payment until final proof (#3 proposal 12, approved 26 Oct). Gateways sit at hubs; no carrier protocol yet.
- The wallet has two kinds of notes: escrow notes (double-spend-proof, need a co-signer quorum) and ordinary notes (pending tier, or receipt-gated near a gateway).

## Stages

Stages are a build order. Every stage is designed for mainnet and run on the harness until the mainnet launch.

| Stage | Runs on | Change |
|---|---|---|
| MVP v1 | Harness (local NU7 chain, ths v0.4.1) | Everything above: pending tier with PaymentStatement, Receipt, nullifier sharing, gateway mempool receipt. Chain-state checks in three steps: (1) signed bundle format with every slot, anchor-in-window, expiry margin, txid branch, MMR anchor verifier; (2) Equihash/difficulty checker with the pinned floor and mainnet header vectors; (3) two-gateway agreement and `checked (single-source)` degradation. Tor submission path after loopback works |
| MVP v1.1 | Harness | Co-signer group (payer + 3-of-4); escrow notes; bilateral configuration |
| MVP v1.2 | Harness | Co-signer reconfiguration: replace a lost co-signer by sweeping escrow notes to a new group key (proposed) |
| Public testnet | Testnet, NU7 from height 4,465,026 | Move once to Common 4.0 (PCZT rc9, clients rc9.3, `zakura-reddsa-frost`) before this stage. Same rules as the harness and mainnet; only difference is the testnet minimum-difficulty rule (block > 450 s after its parent) in the header verifier. Planned: own Zebra + lightwalletd gateway node. Built in v0.7.0: the gateway is a light client of public TLS lightwalletd servers, with chain state trusted ([gateway.md](components/gateway.md#built-in-v070)) |
| Mainnet launch | Mainnet, NU7 from height 3,543,405 | Library step only: Zakura Common 4.x or custom mainnet params; public endpoints over Tor; `regtest-no-pow` off; real ZEC, each send approved. No design change (the earlier "MVP v2" row that deferred Tor and header validation to this stage is superseded) |
| Logos phase | Mainnet | Basecamps and Logos Delivery; Zebra module as gateway; quorum committees |

What to work on next: [open-work.md](open-work.md).

Current design order: #1 double-spend re-decided 26 Oct and #2 chain trust settled; #3 expiry/re-issue decided for a minimal build (26 Oct); **#4 gateway protocol next**, then #5 BitChat durability, #6 fleet fingerprint, #7 stack compatibility, #8 offline limits/phone proving, #9 policy values. #2 leftovers decided: reset at `confirmed` on both networks; single-source is Receipt state bit 0x80. [Dated community prior art](prior-art.md) informs the remaining work.
