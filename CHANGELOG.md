# Changelog

All notable changes to this project are documented here, following
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This is experimental software for a local NU7 Regtest chain with test coins.
It is not security reviewed and does not support real funds.

## [Unreleased]

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

[Unreleased]: https://github.com/megabyte0x/offline-zcash/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/megabyte0x/offline-zcash/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/megabyte0x/offline-zcash/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/megabyte0x/offline-zcash/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/megabyte0x/offline-zcash/releases/tag/v0.1.0
