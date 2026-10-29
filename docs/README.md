# Documentation

**v0.7.7 · public-testnet TAZ and local Regtest · no mainnet.**
Wallets carry shielded payments over a mesh to an internet-connected gateway.
Chain reports trust that gateway; independent chain proofs are not built.

## Start here

| Goal | Read |
|---|---|
| Install, pay, receive or run a gateway | [Basecamp guide](basecamp-install.md) |
| Understand the payment flow | [How it works](how-it-works.md) |
| Understand what must be trusted | [Privacy and threats](privacy-and-threats.md#trust-assumptions-at-v070) |
| See what changed | [Changelog](../CHANGELOG.md) |
| Find verified cases and remaining work | [Open work](open-work.md) |
| Use or build the CLI | [ozm-node guide](install.md) |
| Build the Android test shell | [Android guide](android-install.md) |

Releases ship Basecamp packages for macOS and Linux. CLI/APK downloads stop at
v0.7.3; build current source for later behavior. Funded Linux runs have
[dated evidence](open-work.md#built); current Mac, phone and radio tests remain open.

## Architecture and components

[System design](design.md) separates the current app from the mainnet target.
**Built** means implemented; **Decided** means agreed, not necessarily built;
**Proposed** awaits a decision; **Open** is unfinished. Research PoCs are not app features.

| Component | Owns |
|---|---|
| [Chain](components/chain.md) | Network parameters, transaction format, fees, service observations and Regtest gaps |
| [Offline builder](components/offline-builder.md) | Common 4.0 pins, local proving, reservations, sync and note preparation |
| [Payee verifier](components/payee-verifier.md) | Current offline checks and receipts; planned independent chain verification |
| [Mesh](components/mesh.md) | Flooding, sealed envelopes, store-and-forward, compact sync, BLE implementation and hardware gates |
| [Gateway](components/gateway.md) | TLS lightwalletd submission, retries, reports, admission limits and future proof bundles |
| [Pending tier](components/pending-tier.md) | Invited-payer admission; planned binary formats, enrollment and nullifier sharing |
| [Double-spend controls](components/double-spend.md) | Acceptance tiers and the unbuilt payer + 3-of-4 co-signer group |
| [Expiry and re-issue](components/expiry-reissue.md) | Expiry measurements, accepted repair design and unbuilt payment-family protocol |

## Source, builds and historical evidence

| Path or guide | Contents |
|---|---|
| [zcash-offline-core](../crates/zcash-offline-core) | Wallet, builder, verifier, sealing and signed messages; no transport |
| [ozm-mesh](../crates/ozm-mesh/README.md) | Transport library and wire format |
| [ozm-node](../crates/ozm-node) | Shared device engine, gateway, local API and web UI |
| [Logos developer guide](../logos/README.md) | Shared module API, package builds and Basecamp evidence |
| [Bench guide](../bench/README.md) | Regtest mesh, LAN, invite-free and Kotlin-link checks |
| [Catalogue submission](logos-catalogue-submission.md) | Draft official-catalogue contribution; nothing submitted |
| [v0.1.0](mvp-v0.1.0.md), [v0.2.0](mvp-v0.2.0.md) | Historical multi-hop and LAN results, not current setup instructions |
| [Prior art](prior-art.md) | External research at the stated 25 October cutoff |
| [Archive](../archive/README.md) | Read-only superseded reports and plans; current docs take precedence |

[Repository guidance](../AGENTS.md): development rules and the `ths` issue log.
[PoCs](../poc/): FROST and MMR experiments.
