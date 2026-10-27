# Changelog

All notable changes to this project are documented here, following
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This is experimental software for NU7 public testnet and a local Regtest harness with test coins.
It is not security reviewed and does not support real funds.

## [Unreleased]

## [0.7.1] - 2026-10-27

User-interface release: no changes to the core, mesh, wire formats, module API or node behaviour.

### Changed
- Every wallet UI (web, Basecamp and Android) labels payment states the same way, with a one-line explanation per state: submitted, in mempool, retrying, included and mined (both marked gateway-reported), refused by payee, rejected by gateway, expired and conflict, each with its own colour and a legend.
- The Basecamp wallet's first screen explains starting without an invite in three steps; the organiser invite and connection options (birthday, dial, listen, Bluetooth) are optional, collapsed sections.
- Pay forms warn when an address does not match the wallet's network prefix, check the amount before proving, and show the Max builder fee next to the amount.
- A chain-view box shows the selected gateway, hops, tip and mesh ID, whether chain sources are cross-checked, and the gateway trust warning as a highlighted note.
- Prepare for offline is its own card with the three steps (prepare online, wait for mined, sync then go offline), input checks and a visible error.
- Clearer loading, empty, unreachable-node and error messages that say what to do next; refusals note that the payment may still be mined until expiry.
- Organiser groups setup into Session, Devices and Export, checks names, roles, birthday and the join name before creating, warns when no device is the gateway, and highlights that invites contain private test keys.
- Gateway shows join, role refusal, chain-service checks, wallet funding and each submission's result with a gateway-reported label.
- One palette, button and field style across the web UI and the three Basecamp apps; layouts stack at narrow widths, and payment tables become cards on phones.
- Accessibility: every control has a label, focus is visible for keyboard users, status messages are announced, and muted text has higher contrast.
- Android version is 0.7.1 (version code 10).

## [0.7.0] - 2026-10-27

### Added
- Public-testnet fleets and persistent local identities, with `utest1` addresses, a configurable birthday at/after NU7 and a GetTreeState-seeded Ironwood tree. Regtest remains the serde/startup default, with birthday 1.
- Configurable TLS lightwalletd gateway servers, compact GetBlockRange sync and SendTransaction submission, without a full node or gateway viewing-key scanning. Keep ths v0.4.1 JSON-RPC benches working; refuse public mining and gateway faucet requests.
- Prepare for offline in CLI, web, Basecamp and Android: split one mined note into equal notes with the ZIP 317 action-count fee. Wait for mining and wallet sync before going offline; never spend unmined change.
- Gateway watcher persists exact transaction bytes, retries every three blocks until expiry, and pushes signed submitted/included/mined/expiry/conflict results to payer and payee. Testnet inclusion is 1 confirmation; mined is 10, and reorgs can downgrade either.

### Changed
- Pin the offline builder and receiving-authority proofs to Zakura Common 4.0.0, using its matching note-encryption and Pasta crates. The Basecamp service uses Rust 1.97.1.
- Compact mesh sync transports Ironwood nf/cmx/epk/52-byte ciphertext, run-length empty blocks and digest-checked 3 KiB chunks, including splits within a block. Persist a pruned ShardTree with 200 checkpoints/roots and rewind instead of clearing the wallet on ordinary reorgs.
- Sync/control messages use 512 B / 4 KiB / 16 KiB padding buckets; pay/sub/receipt padding is unchanged. Height/hash tips coalesce for 15 minutes or 200 blocks, with immediate payment results and manual refresh. Checkpoint history travels only on divergence. The dated daily replay measures 1.14 MB compact payload / 2.12 MB framed sync, versus 79.80 MB before batching; including hourly signed presence and idle control gives 2.40 MB per BLE link / 2.52 MB TCP. Radio/IP headers and reconnect/reorg traffic are excluded. Presence is immediate on link-up, otherwise hourly with three-hour expiry; gateway changes/loss may take an hour to be noticed on stable links. Payload and full-stream counters are distinct; sizes and timing still distinguish sync from payments.
- Android version is 0.7.0 (version code 9); all four Logos packages share the same engine and version.

### Security
- Keep chain state and included/mined explicitly gateway-reported. Compare reachable configured servers' hashes, compact ranges, frontiers and transaction answers, withholding disagreements and warning on single-source operation. TLS/agreement are not chain proofs; PoW/MMR/inclusion tasks 7–9 remain deferred.
- Match exact submit codes/messages; queued downloads, queue-full and behind-tip errors retry rather than claiming acceptance. Unknown error wording never becomes duplicate success.
- Release unspent reservations only when local compact scan passes the transaction's inclusive expiry, retaining enough history to relock after a rewind below expiry. A gateway expiry report alone cannot unlock notes. Refusal now warns that the transaction may still mine until expiry.

### Known limitations
- The Zaino secondary returned HTTP 503. Valar queued our claim but failed with `the wallet cannot pay this address`; zecpages timed out after 30001 ms. The funded public split/payment/ten-confirmation run and independent confirmation remain unverified; releasing without that run is an explicit owner exception.
- No macOS, physical radio or native Android-screen run establishes this version. Ten confirmations are not testnet finality. The approximate 1.2 MB/day budget must include mesh control, padding and framing, not just compact protobuf.

## [0.6.0] - 2026-10-27

### Added
- Invite-free devices generate and retain their own keys, discover open LAN/BLE peers, and sync from signed gateway announcements ranked by hops then advertised tip.
- Signed, sealed address bindings route optional payee receipts. Unknown mesh destinations still submit by Zcash address, with an explicit no-payee-receipt state.
- Max in Basecamp, web and Android uses the builder's padded Ironwood action count and ZIP 317 fee, excluding locked notes and uneconomic inputs.
- Six-device no-invite Regtest benchmark covering sync, address payments, an off-mesh destination, faucet throttling and a mined Max payment.

### Changed
- Address entry replaces fleet recipient selection in every wallet UI; CLI `pay --to` accepts addresses and retains legacy fleet names.
- Selected gateway identity, hops and tip are visible; sync and mining remain explicitly gateway-reported, not independently proved.
- Gateway test funds are limited to one request per device per 60 seconds, at most ten notes and the configured total cap. Fresh identities can evade this development-only limit.
- Android version is 0.6.0 (version code 8). Public catalogue and installer instructions remain authentication-free.

### Security
- Every inner envelope authenticates its sender and intended recipient; mesh acks authenticate the original destination. Unsigned legacy envelopes/acks are rejected, even from invited peers. v0.5.x nodes ignore unsupported messages without crashing, but payment endpoints must upgrade together.
- Address bindings carry a Schnorr proof of receiving authority, reveal no viewing key, and pin the mesh key at first receipt routing. They do not prove spending authority; the experimental proof implementation is not independently reviewed.
- Gateway selection pins the key at first use, survives restart, and warns/blocks instead of silently switching. Initial selection is still vulnerable to Sybil gateways: verify the mesh ID with the operator. Chain facts remain gateway-reported.
- Open-mode and unknown payers cannot receive pending acceptance; payees wait for their pinned gateway's inclusion report before issuing an Included receipt.
- Loopback HTTP requires a random per-run API token and a loopback Host header. Remote UI strings render as text, including history, refusal reasons and transaction IDs. Regression checks cover forged sync replies/acks, API access and browser script injection.
- A payment scanned before its mesh envelope is no longer mistaken for a competing spend; different transactions spending the same note still fail.

### Known limitations
- Host-JVM Kotlin links passed at 20/182/514-byte chunks on a fresh chain, but a 20-byte-link cold sync against the accumulated chain timed out; large-history sync/recovery remains unresolved. v0.6.0 has not been tested on real Macs, Android screens or physical radios. Chain proofs and an independent review of the address proof remain outstanding.

## [0.5.1] - 2026-10-27

### Added
- A public Logos catalogue for installing the service and all three Basecamp apps by name, with merged Linux x86_64, Linux arm64 and macOS arm64 packages, content-root verification and SHA-256 checksums.
- Official-catalogue submission instructions and a versioned changelog used as GitHub release notes.

### Changed
- Installation guides lead with custom-catalogue installation in Basecamp and `logosctl`; manual LGX installation remains available.
- Public downloads no longer require GitHub authentication.
- Android identifies this release as 0.5.1 (version code 7).

### Security
- Internal research links and automatic thread attribution were removed from published history; CI rejects internal thread links in tracked files.

## [0.5.0] - 2026-10-27

### Added
- Basecamp Organiser app to create fleets, combine device roles, export private invites and optionally join the organiser's device.
- Basecamp Gateway app to check RPC, start the gateway, change RPC/mining settings live, and inspect balances and submissions.
- Four LGX packages per platform, sharing one core identity, wallet and mesh across all three apps.

### Changed
- Fleet creation is available through the node library and Logos module API; invite roles remain authoritative.
- Gateway settings and submission history survive Basecamp restart.
- Android version is 0.5.0 (version code 6).

### Fixed
- Exported session folders open through the core module rather than blocked sandboxed QML file URLs, including paths containing `#`.

## [0.4.1] - 2026-10-27

### Added
- Experimental Linux BlueZ central/peripheral BLE transport, CLI `--ble`, and a Basecamp Bluetooth toggle.
- BLE/TCP neighbour labels and Bluetooth status in desktop views.
- Desktop BLE protocol checks for chunking, timeouts and acknowledgement handling; real-radio validation remains open.

### Changed
- libdbus is bundled; BlueZ and system D-Bus remain host requirements.
- Android version is 0.4.1 (version code 5).

## [0.4.0] - 2026-10-27

### Added
- UniFFI/Kotlin node API and Android 10+ arm64 BLE/LAN test app with invite join, sync, test funds, payments and a connected-device foreground service.
- BLE byte-stream attachment, per-link keepalives, and host-JVM Kotlin payment contract checks.
- Android APK/checksum release builds with a stable public Regtest signing key.

### Changed
- Mesh links support BLE-sized frames and BLE-specific timing alongside TCP.

### Fixed
- Android build/install guide and CI platform-tools setup, checked from a fresh orb.

## [0.3.0] - 2026-10-27

### Added
- Logos Basecamp core module and QML wallet app using the same node and mesh as the CLI.
- Portable LGX releases for Linux x86_64, Linux arm64 and macOS arm64, plus a tester installation guide.

### Changed
- The service has a distinct display name from the wallet app.
- Documentation records Basecamp 0.3.2's Linux glibc 2.39 requirement and portable/development variant distinction.

## [0.2.0] - 2026-10-26

### Added
- Installable `ozm-node` releases for macOS and Linux, a checksum-verifying install script, local HTTP API and web UI.
- Fleet invites, UDP LAN discovery, wallet sync and test funds carried over the mesh.
- LAN session and funding benchmarks.

## [0.1.0] - 2026-10-26

### Added
- Offline building, proving and signing of v6 Ironwood payments on Zakura Common 2.2.0.
- TCP mesh with TTL flooding, 512-byte frames, store-and-forward and acknowledgements.
- Offline payee checks, signed pending receipts, idempotent gateway submission/mining and signed gateway-reported results.
- Four-container benchmarks for multi-hop delivery, restart recovery and payment handling.

[Unreleased]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/megabyte0x/offline-zcash/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/megabyte0x/offline-zcash/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/megabyte0x/offline-zcash/releases/tag/v0.1.0
