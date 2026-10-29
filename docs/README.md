# Documentation

Offline Zcash carries signed, proven shielded payments over a local mesh to an
internet-connected gateway. **Current release: v0.7.7, 29 October 2026.**
The experiment uses public-testnet TAZ; local Regtest is the development harness.
Mainnet is unsupported. Gateway signatures authenticate reports, not chain truth.

## Start here

| Goal | Read |
|---|---|
| Install, pay, receive or run a gateway | [Basecamp guide](basecamp-install.md) |
| Understand the payment flow | [How it works: three diagrams](how-it-works.md) |
| Understand what must be trusted | [Privacy and threats](privacy-and-threats.md#trust-assumptions-at-v070) |
| See what changed | [Changelog](../CHANGELOG.md) |
| Find verified cases and remaining work | [Open work](open-work.md) |
| Use or build the CLI | [ozm-node guide](install.md) |
| Build the Android test shell | [Android guide](android-install.md) — no phone/radio validation yet |

## What is available now

- **Basecamp on macOS and Linux:** one shared service and three apps: wallet,
  Organiser and Gateway. Wallets create their own keys without invites; Organiser
  exports one gateway invite. Every device pays, receives and relays.
- **Automatic readiness:** per-device scan starts, startup/discovery/new-tip sync,
  automatic economical note splitting, Pay during catch-up, durable incoming
  verification and pending incoming balances. Simple wallet first; technical
  controls under Debug.
- **Recovery:** Leave session backs up rather than deletes. A same-network
  invite-free restart restores a used wallet, including queued incoming payments;
  empty backups get fresh keys. Start a new wallet explicitly keeps the old backup.
- **Latest fixes:** v0.7.6 makes presence/tip notices replaceable so they cannot
  exhaust the sender's mesh quota; v0.7.7 bounds slow chain work and lets mesh
  submissions wait for a busy gateway. Upgrade the service before the apps.
- **Distribution:** releases publish four Basecamp packages for each of
  `darwin-arm64`, `linux-amd64` and `linux-arm64`, plus four merged catalogue
  packages and checksums. CLI/APK downloads stop at v0.7.3; build current source
  for later behavior. Packages remain unsigned.

Funded Linux Basecamp runs exercised splitting, offline delivery, Max, restart,
recovery and ten gateway-reported confirmations ([dated evidence](open-work.md#built)).
The owner tested v0.5.1 on three Macs; current Mac versions need retesting.
Physical BLE, native Android screens and independent chain proofs remain open.

## Architecture and components

[System design](design.md) separates the shipped testnet application from the
mainnet target. **Built** means implemented, **Decided** means an agreed design,
**Proposed** awaits a decision, and **Open** names unfinished work. Dated findings
retain their original versions and test scope; a research PoC is not an app feature.

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

[Repository guidance](../AGENTS.md) holds development rules and the `ths` issue log.
[PoCs](../poc/) hold research-only FROST and MMR experiments.
