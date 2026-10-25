# Docs

Offline Zcash payments: a signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet submits it.

| Read | For |
|---|---|
| [design.md](design.md) | System view, payment flow, finalized decisions, stages |
| [open-work.md](open-work.md) | What to build and test next, in order |
| [components/chain.md](components/chain.md) | Local `ths` chain, Ironwood/v6 facts, fees, mainnet step |
| [components/offline-builder.md](components/offline-builder.md) | Crate pins, build sequence, note reservation |
| [components/payee-verifier.md](components/payee-verifier.md) | Offline checks, receipt states, what headers can prove |
| [components/gateway.md](components/gateway.md) | Submission, chain bundles, gateway trust |
| [components/transport-bitchat.md](components/transport-bitchat.md) | BitChat fork on three Macs, durability rules |
| [components/double-spend.md](components/double-spend.md) | Pending tier, dropped layers, bilateral escrow |
| [privacy-and-threats.md](privacy-and-threats.md) | Threat model and leaks |
| [later/logos.md](later/logos.md) | Logos phase (not MVP) |

Each component file has the same sections: **Decided** (build around it), **Findings** (with sources), **Open** (unverified items and tests). Repository rules and the `ths` issue log stay in [../AGENTS.md](../AGENTS.md). Original long-form research is in [../archive/](../archive/README.md).
