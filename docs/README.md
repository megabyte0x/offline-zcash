# Docs

Offline Zcash payments: a signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet submits it.

Rule: the design target is mainnet with an untrusted gateway; the local `ths` Regtest chain is a test harness and never a reason to weaken a check ([design.md](design.md)).

Current state (26 October 2026): problem #1 (double-spend) was **re-decided 26 October**: pending below the floor, payer + 3-of-4 co-signer group above it; #2 (chain trust) is settled; #3 expiry/re-issue and the #2 leftovers were decided 26 October for a minimal build (shutdown profile, A-initiated repair); next design problem is **#4 gateway protocol** ([open-work.md](open-work.md)).

**v0.1.0 is built (26 October 2026):** a payment built offline crosses a four-device mesh (A–R–B–D, each device in its own container) and is mined on the local chain. All seven gating tests pass. See [mvp-v0.1.0.md](mvp-v0.1.0.md), with code in [../crates/](../crates/) and the bench in [../bench/](../bench/README.md).

| Read | For |
|---|---|
| [mvp-v0.1.0.md](mvp-v0.1.0.md) | What v0.1.0 runs, measured timeline, gating test results, decisions and limits |
| [design.md](design.md) | System view, payment flow, finalized decisions, stages |
| [open-work.md](open-work.md) | What to build and test next, in order |
| [prior-art.md](prior-art.md) | Dated community sweep: borrow/don't-borrow inventory, flaws, negative searches and contacts (cutoff 25 October 2026) |
| [components/chain.md](components/chain.md) | Mainnet target, local `ths` harness and its gaps (`regtest-no-pow`), Ironwood/v6 facts, headers and history tree, fees |
| [components/offline-builder.md](components/offline-builder.md) | Crate pins, build sequence, note reservation |
| [components/expiry-reissue.md](components/expiry-reissue.md) | Measured expiry/renewal/re-anchor facts; twelve unratified proposals plus bucketed alternative; eight open items and ten gates |
| [components/payee-verifier.md](components/payee-verifier.md) | Offline checks, chain-state checks with no trust in the gateway, receipt states, what headers can prove |
| [components/pending-tier.md](components/pending-tier.md) | PaymentStatement and Receipt formats, payment keys, nullifier sharing, limits |
| [components/gateway.md](components/gateway.md) | Submission (Tor on mainnet), signed bundles, history-MMR rebuild, untrusted-gateway mitigations |
| [components/transport-bitchat.md](components/transport-bitchat.md) | BitChat fork on four Macs, durability rules |
| [components/double-spend.md](components/double-spend.md) | Acceptance tiers; payer + 3-of-4 co-signer group; rejected alternatives (26 Oct) |
| [privacy-and-threats.md](privacy-and-threats.md) | Threat model and leaks |
| [later/logos.md](later/logos.md) | Logos pathway (tried first; BitChat is the fallback) |
| [later/logos-modules.md](later/logos-modules.md) | Logos pathway module designs (proposed): `mesh_transport` contract and backends, `zcash_offline_module`, BLE mesh, upstream issue drafts |
| [later/logos-v0.1.0-plan.md](later/logos-v0.1.0-plan.md) | Logos pathway v0.1.0 build plan (proposed): `mesh_offlineprotocol` first, Basecamp UI included, milestones and gating tests |

Each component uses **Decided** (build around it), **Findings** (with sources), **Open** (unverified items/tests); **Proposed** means awaiting owner approval, not implemented. Provisional numerical policies are not measured safety guarantees. Repository rules and the `ths` issue log stay in [../AGENTS.md](../AGENTS.md). Original long-form research is in [../archive/](../archive/README.md), read-only. Throwaway feasibility crates live in [../poc/](../poc/): `escrowcheck` (FROST co-signing on the Zakura 2.2.0 pins) and `mmr-anchor-poc` (ZIP 221/258 anchor proofs against the live `ths` chain).
