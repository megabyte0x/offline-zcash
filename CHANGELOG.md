# Changelog

All notable changes to this project are documented here, following
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This is experimental software for NU7 public testnet and a local Regtest harness with test coins.
It is not security reviewed and does not support real funds.

## [Unreleased]

### Changed
- Fresh wallets record their creation time and choose their own scan start automatically. Online creation starts 100 blocks behind the tip; offline creation uses a day of overlap estimated from the release checkpoint and first gateway contact. Existing wallet scan starts remain unchanged. Birthday inputs are removed from Basecamp; the CLI override remains for benches.
- Wallets sync automatically at startup, on gateway discovery and on new chain tips. Refresh is optional. Frontier initialization checks the Ironwood tree size against compact-block metadata. Offline bootstrap uses an authenticated chain-clock exchange; upgrade gateways together with wallets.
- Pay, Max and Prepare remain available during sync. Block downloads no longer hold the payment/apply lock, and installed batches preserve reservations made during downloads. Wallet balances state their scanned height while catching up.
- Verified mesh payments immediately add durable incoming notes and a separate pending incoming balance. Scanning merges them by commitment, never counts them as spendable early, and included/mined reports trigger sync. Expired/conflicting reports remove the pending incoming notes.
- Catch-up builds tree fragments and retained roots in parallel. An 85,000-block live-testnet CLI scan fell from a 41.80-second median to 14.12 seconds on an 8-core orb. Online ranges download only from the primary; each applied root is checked against reachable secondary tree states. All 200 checkpoints remain. Sync snapshots persist every five seconds and on completion/stop, with an unconditional durable write before payments or receipts leave; interruption can replay the last uncommitted batches. Mesh batches and wire formats are unchanged.
- Deep reorgs recover from a conservative sparse common ancestor and the earliest note height, floored at the device's stored scan start. First-note metadata and reservations survive reset; empty runs retain 1,000-block hash samples without changing the mesh schema.
- Workspace, four Logos packages and Android version are 0.7.4; Android version code is 13.

## [0.7.4] - 2026-10-28

### Changed
- **Testnet is the default network.** New Organiser sessions, new invite-free wallets in Basecamp, and `ozm-node fleet-init`, `start` and `run` default to public testnet. Before, the Organiser's network selector and the wallet's Get started selector started on Regtest, so a session created without changing it (for example the default "Three laptops") was Regtest and its gateway asked for a ths RPC URL. Regtest stays available under a collapsed **Advanced (local Regtest for developers)** section (`--network regtest` on the CLI). Fleet files and invites without a `network` field still load as Regtest, so existing Regtest sessions keep working. The benches pass `--network regtest`.
- The header of every Basecamp app and the web UI shows **Regtest session (local ths chain)** with a pointer to Leave session (a new `--home` for the CLI) when the device is on Regtest. The Gateway shows the RPC URL, mining switch and `ths faucet` command, and the wallet the test-funds button, only on a Regtest session.
- **Leave session keeps your wallet.** When the device next starts without an invite on the same network, it restores the keys, address, wallet and payment history from the newest backup holding an invite-free wallet on that network, instead of creating a new wallet. The gateway pin, mesh store, events and start options stay behind, so the device selects a gateway again. Leave still moves the whole folder to `ozm-backup-*` and deletes nothing; invited wallets stay in their backup. Wallets left with v0.7.3 are restored too.
- Releases publish only the macOS and Linux Basecamp packages: four `.lgx` per platform (darwin-arm64, linux-amd64, linux-arm64), the four merged catalogue packages, the catalogue index and checksums. CLI tarballs and the Android APK are no longer built for releases (their code stays); `scripts/install.sh` installs the v0.7.3 CLI by default.
- CI runs the Rust and macOS build jobs only when crates, Cargo files or the CI workflow change, and the Android APK only when `android/`, `crates/ozm-ffi` or the Android or CI workflow change. Docs-only changes run only the thread-link and release-notes checks.
- Workspace, four Logos packages and Android version are 0.7.4; Android version code is 13.

### Fixed
- A device syncing over the mesh stopped with "Sync failed: mesh store quota exceeded", and later sends, payments included, failed the same way. Every received sync chunk (3 KiB) left one of the device's own delivery acks in the store-and-forward store for 24 hours, and nothing acknowledges an ack. After 512, the per-sender cap, the device could not store its next sync request; the gateway held the same acks and would also have dropped it. A testnet catch-up from 4,465,026 stopped at 4,467,225. A sender's new packet now evicts that sender's oldest stored acks first (and on the sending device, its own old broadcasts). Held messages and other senders' packets are never evicted, so the v0.7.2 limits still apply.
- A large sync batch (for example 186 chunks for busy testnet blocks) never completed on a stable link, so the catch-up stalled. The gateway's link queue holds 128 packets; the other 58 chunks were stored but not sent until the link reconnected. Packets that do not fit a link's queue are now sent as it drains, and a device that had to drop packets because its app was busy asks that neighbour for them again on the next 10-second sweep. A batch with more chunks than the gateway's 512-packet store quota (710 chunks for one busy testnet range) failed on the gateway with "mesh store quota exceeded"; the gateway now waits up to 30 seconds per chunk for the device's acks to free room.
- The Basecamp guide's new "Upgrade did not take effect" section (docs/basecamp-install.md) explains why Basecamp can keep showing an older version (for example "Offline Zcash v0.7.2" and a `uregtest1…` prefix warning on testnet after installing v0.7.3) and how to load the new service.

## [0.7.3] - 2026-10-28

### Added
- **Leave session** in the wallet, Organiser and Gateway apps (module method `leave`), behind a confirmation. It stops the node and moves the device folder `ozm` to `ozm-backup-YYYYMMDD-HHMMSS` (UTC) in the same persistence folder; nothing is deleted. The device can then start without an invite or join another one, for example to switch from Regtest to testnet. New Organiser exports go to `sessions/` beside `ozm`, and Leave moves older `ozm/sessions` exports there, so their invites stay reachable.

### Changed
- The Organiser has one **"This device is the gateway"** option per device instead of payer/payee/relay/gateway checkboxes, and requires at least one gateway. Every device could already pay, receive and relay; those roles had no effect. `ozm-node fleet-init` takes `--device NAME` or `--device NAME:gateway`. Existing invites, fleet files and legacy `NAME:payer+payee` input keep working; every role except gateway is ignored. Status `role` is now `gateway` or empty.
- Regtest gateways reject `https://` chain URLs (such as testnet lightwalletd) with an explanation that the device is on a Regtest session and that testnet needs Leave session and a testnet invite. A server that does not answer Zakura JSON-RPC (for example lightwalletd on 9067) gets a message naming the `ths endpoints` RPC URL instead of "is ths running?".
- On a testnet session the Gateway hides the RPC field, the mining switch and the `ths faucet` command, and shows the session's TLS lightwalletd servers instead.
- Workspace, four Logos packages and Android version are 0.7.3; Android version code is 12.

### Fixed
- A long wallet sync no longer blocks the Basecamp service. On a testnet session with birthday 0, the Gateway's first sync scanned from 4,465,026 inside `sync_now` on the module's call thread, so every other call failed ("Module source unavailable", then "timeout") and badges stayed on a stale "stopped". `sync_now` now starts the sync in the background and returns at once; `status` reports `syncing`, `sync_error` and `chain_tip`, and the apps show `syncing · height X of Y`. Payments and Prepare for offline are refused while a sync runs instead of waiting on the call thread; Leave cancels the sync. An online device resumes a background sync when Basecamp restarts.
- The apps show Basecamp's error detail (for example the CallError message) instead of only "Module source unavailable", say "service not answering" instead of a stale state, and only suggest reinstalling when the service is not loaded.
- The Organiser fills a testnet session's birthday with the current tip (new module method `testnet_tip`) and refuses a testnet birthday before 4,465,026, so wallets do not scan from NU7 activation.
- The recipient field placeholder follows the session network (`utest1…` on testnet) in the web UI, and the Basecamp wallet no longer falls back to `uregtest1…` while the network is unknown.
- The Organiser suggests a new export folder after each session, and the Gateway clears the mining switch after Leave.
- The Basecamp service no longer handles one call at a time (`concurrency: "multi"`). Before, one slow call held every app: a proof, a chain check against an unreachable server, `testnet_tip` or Leave waiting for a payment made `status` time out, so badges went stale. Start, join and Leave are still serialized with each other.
- Leave no longer leaves a half-moved device behind. Background tasks (gateway watch, retries, mesh store, trust pins, events) could write into `ozm/` after it moved, recreating a partial `ozm` folder. Leave now cancels a running sync, waits for an in-flight payment, and stops every writer before moving the folder.
- **Prepare for offline** failed in Basecamp with `Call refused (prepare_offline): expected integer at arg0, got number`; the app now passes the note count as an integer.
- A payment was sent twice in the same millisecond window: the retry loop saw `last_send_ms = 0` while the first send was still in flight (history showed "sent over mesh (attempt 2)" 3 ms after attempt 1). New payments now start their retry timer when they are built.
- A payee waited about 30 s for `mined` after a fast LAN payment. Payer and payee submit the same transaction moments apart; the gateway answered the second "gateway busy; retry submission", so the payee only learned the result from its retry. An identical submission that arrives while the first is processed now joins it and receives the same result. Chain checks (`check_rpc`, `testnet_tip`) give up after 15 s with the server name, before Basecamp's 20 s call timeout turns them into "Module source unavailable".
- Concurrent node events could interleave inside one line of `events.jsonl`, leaving unparseable lines; each event is now appended in one write under a lock.
- The first testnet scan is about 6× faster (20,000 testnet blocks: 113 s → 17 s on an 8-core orb). The online scan fetches 2,000-block batches while the previous one applies, trial-decrypts on all cores, and skips checkpoints and roots that pruning discards. The retained 200-checkpoint window is unchanged. Mesh sync replies keep 200-block batches.
- Organiser export folders are named `session-YYYYMMDD-HHMMSS` (local time) instead of a millisecond counter.

### Removed
- The LAN fleet tag and BLE fleet hint. Invited devices advertised them so they only auto-connected within their fleet, but every device relays and the tag authenticated nothing. All devices now advertise zero, which v0.7.2 peers already accept, and this removes a fleet-membership marker from beacons. The FFI `fleetHint()` is removed.

## [0.7.2] - 2026-10-28

### Security
- Rechecked every trust-assumption row against v0.7.1. Keep the existing strict Ed25519 verification, signed envelopes/acks, text-only UI, API token/Host check, address proofs, gateway pins, no open-mode pending acceptance, action-count fees, exact error classification and rollback-aware expiry locks.
- Enforce Unix 0600 on wallet, payments, mesh cache, keys, trust pins and event files, including upgrades and atomic temporary replacements. This is not encryption or protection from same-user malware; Windows still relies on account ACLs.
- Bound mesh packet/byte storage globally and per claimed sender, dedup sets, fragment reassembly, links, queued bytes and application handlers. Reject packets more than five minutes in the future before they can poison dedup. Refuse new work rather than evict held packets; sender headers remain spoofable and quotas do not defeat Sybils or guarantee availability.
- Require a fresh Ed25519 key-possession proof before sending or serving link inventory. **Compatibility:** adds link messages 0x12/0x13; older v0.7.x peers can live-forward but cannot inventory-sync with v0.7.2. Upgrade offline store-and-forward peers together. Authentication proves a key, not trusted fleet membership.
- Bound gateway submissions before expensive proof checks: per-sender rate/queue limits, global and stranger byte/entry caps, bounded notification targets and one active submission worker; reuse proof checks for exact held bytes. Preserve enrolled capacity and never evict held transactions for new work.
- Replace explicit payer/payee notification lists with persisted random request nonces and signed nonce/result broadcasts; retain legacy private results and the existing `Sub` schema. **Privacy tradeoff (P12):** txid/status/nonce are now plaintext mesh-wide, while amounts and payment statements remain sealed. The gateway still sees each submitter and can correlate identical txids and timing; this is not anonymity.
- Bound invited pending acceptance at three outstanding payments / 1 ZEC per payer and 3 ZEC per payee, serialized with payment construction. Expired debt still counts; mined debt releases the cap and a reorg downgrade counts again. These are provisional test-coin limits, not double-spend prevention.
- Remove the cached-mined fallback when a testnet transaction disappears after a reorg. Submit unchanged bytes concurrently to every connected configured lightwalletd server, including when the primary fails. Strict HTTPS certificate/hostname validation already shipped; TLS and source agreement remain trusted, not chain proofs.
- Pin all external GitHub Actions to full commit SHAs; tagged builds do not restore branch Rust/Gradle caches. Disable Android platform cleartext opt-in; native mesh TCP/BLE metadata is still plaintext.

### Changed
- Workspace, four Logos packages and Android version are 0.7.2; Android version code is 11.

### Known limitations
- **Funded public-testnet payment to mined is unverified for v0.7.2.** No funded wallet exists and faucets keep failing; the owner directed this release to use local checks and Regtest benches instead. No testnet wallet state was transferred.
- Deferred: Tor, PoW/MMR/inclusion proofs (tasks 7–9), nullifier sharing/co-signers, on-device fleet keys and signed enrollment, package/private APK signing, keystores, forward secrecy, diversified receipt routing, host-enforced Basecamp caller isolation, SPKI key/rotation policy, and coordinated BLE/beacon identity rotation. Rotating only a fleet hint would leave stable peer IDs visible. These are not contained compatible fixes; see the row-specific reasons in `docs/privacy-and-threats.md`.
- Real Macs, native Android screens and physical radios remain unverified. Ten testnet confirmations are not finality; first-contact Sybils, malicious pinned gateways and unverified chain views remain risks.

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

[Unreleased]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.4...HEAD
[0.7.4]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/megabyte0x/offline-zcash/compare/v0.7.1...v0.7.2
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
