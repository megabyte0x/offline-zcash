# Docs

Offline Zcash payments: a signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet submits it.

Rule: the design target is mainnet with an untrusted gateway; the local `ths` Regtest chain is a test harness and never a reason to weaken a check ([design.md](design.md)).

Current state (27 October 2026): **v0.1.0 to v0.6.0** run on the local NU7 chain: a payment built offline hops across the mesh, the payee checks it and sends a receipt, a gateway mines it and the result comes back. v0.2.0 is an installable `ozm-node` with LAN discovery; v0.3.0 runs the same device inside Logos Basecamp. **v0.4.0** adds an Android BLE + LAN test app and **v0.4.1** experimental Bluetooth on Linux (`ozm-node --ble`, Basecamp toggle); no real phone or radio has been tested yet. **v0.5.0** adds Basecamp Organiser and Gateway apps alongside the existing node app, all sharing one service; four `.lgx` packages per platform. **v0.5.1** adds public catalogue installation by name and changelog-driven releases. **v0.6.0** adds invite-free joining, payments by address and exact builder-fee Max, with signed envelopes/acks, pinned gateway/address identities and local API protection. Open devices wait for gateway-reported inclusion, never pending acceptance; chain facts are not independently proved. See [basecamp-install.md](basecamp-install.md), [android-install.md](android-install.md), [how-it-works.md](how-it-works.md) and [open-work.md](open-work.md#built). Design: #1 re-decided 26 October (pending below the floor, payer + 3-of-4 co-signer group above it), #2 settled, #3 decided for a minimal build; next design problem is **#4 gateway protocol**. Chain-state bundles, nullifier sharing and the co-signer group are designed but not built.

| Read | For |
|---|---|
| [how-it-works.md](how-it-works.md) | Three diagrams: the payment journey, inside the mesh, the gateway bridge |
| [mvp-v0.2.0.md](mvp-v0.2.0.md) | v0.2.0: install, invites, LAN discovery, sync and test funds over the mesh; how to run a session |
| [../logos/README.md](../logos/README.md) | Basecamp core module and QML UI: API, build, run, what was verified |
| [basecamp-install.md](basecamp-install.md) | Tester guide: install Offline Zcash in Logos Basecamp, join, pay |
| [android-install.md](android-install.md) | Android 10+ arm64 APK: install, permissions, laptop gateway, join and pay; fresh-orb build and Kotlin/Rust contract test |
| [install.md](install.md) | Installing `ozm-node` on macOS and Linux |
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
| [components/mesh.md](components/mesh.md) | `ozm-mesh`: flooding, frames, store-and-forward, acks, LAN discovery; Android BLE implementation (radio gates unverified) |
| [components/double-spend.md](components/double-spend.md) | Acceptance tiers; payer + 3-of-4 co-signer group; rejected alternatives (26 Oct) |
| [privacy-and-threats.md](privacy-and-threats.md) | Threat model and leaks |
| [../archive/](../archive/README.md) | Superseded plans (BitChat fork transport, Logos pathway plans), the original report and research notes |

Each component uses **Decided** (build around it), **Findings** (with sources), **Open** (unverified items/tests); **Proposed** means awaiting owner approval, not implemented. Provisional numerical policies are not measured safety guarantees. Repository rules and the `ths` issue log stay in [../AGENTS.md](../AGENTS.md). Original long-form research is in [../archive/](../archive/README.md), read-only. Throwaway feasibility crates live in [../poc/](../poc/): `escrowcheck` (FROST co-signing on the Zakura 2.2.0 pins) and `mmr-anchor-poc` (ZIP 221/258 anchor proofs against the live `ths` chain).
