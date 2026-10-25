# Docs

Offline Zcash payments: a signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet submits it.

Rule: the design target is mainnet with an untrusted gateway; the local `ths` Regtest chain is a test harness and never a reason to weaken a check ([design.md](design.md)).

| Read | For |
|---|---|
| [design.md](design.md) | System view, payment flow, finalized decisions, stages |
| [open-work.md](open-work.md) | What to build and test next, in order |
| [components/chain.md](components/chain.md) | Mainnet target, local `ths` harness and its gaps (`regtest-no-pow`), Ironwood/v6 facts, headers and history tree, fees |
| [components/offline-builder.md](components/offline-builder.md) | Crate pins, build sequence, note reservation |
| [components/payee-verifier.md](components/payee-verifier.md) | Offline checks, chain-state checks with no trust in the gateway, receipt states, what headers can prove |
| [components/pending-tier.md](components/pending-tier.md) | PaymentStatement and Receipt formats, payment keys, nullifier sharing, limits |
| [components/gateway.md](components/gateway.md) | Submission (Tor on mainnet), signed bundles, history-MMR rebuild, untrusted-gateway mitigations |
| [components/transport-bitchat.md](components/transport-bitchat.md) | BitChat fork on four Macs, durability rules |
| [components/double-spend.md](components/double-spend.md) | Acceptance tiers, nullifier-lock co-signer (v1.1), recovery (v1.2), dropped layers |
| [privacy-and-threats.md](privacy-and-threats.md) | Threat model and leaks |
| [later/logos.md](later/logos.md) | Logos phase (not MVP) |

Each component file has the same sections: **Decided** (build around it), **Findings** (with sources), **Open** (unverified items and tests). Repository rules and the `ths` issue log stay in [../AGENTS.md](../AGENTS.md). Original long-form research is in [../archive/](../archive/README.md). Throwaway feasibility crates live in [../poc/](../poc/): `escrowcheck` (FROST co-signing on the Zakura 2.2.0 pins) and `mmr-anchor-poc` (ZIP 221/258 anchor proofs against the live `ths` chain).
