# AGENTS.md

This repository holds research and a working proof of concept (releases v0.1.0–v0.7.2) for offline Zcash payments. A signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet broadcasts it. Start at `docs/README.md`: `docs/design.md` is the system view, `docs/components/` holds one file per component (decided, findings, open), and `docs/open-work.md` lists what is built and what to do next; `docs/how-it-works.md` shows the flow in diagrams. `archive/` keeps the original long report, research notes and reviews as deep sources; do not edit them, and when they conflict with `docs/`, `docs/` wins.

## Code (v0.7.2)

- `crates/zcash-offline-core`: wallet sync, offline builder, payee checks, `ozm1` sealing, signed messages. No transport code.
- `crates/ozm-mesh`: BitChat-style mesh (TTL flood, 512-byte frames, store-and-forward, acks) over TCP or attached BLE streams; see its README for the wire format.
- `crates/ozm-node`: library and daemon (roles payer, relay, payee, gateway; commands `fleet-init`, `join`, `start`, `fund`, `pay`), local HTTP API and web UI on `127.0.0.1:7780`. `ozm-mesh` LAN discovery uses UDP 7778 ([docs/components/mesh.md](docs/components/mesh.md)).
- `crates/ozm-ffi` and `android/`: UniFFI/Kotlin node API, Android 10+ arm64 BLE + LAN test shell. Build, install and host-JVM contract checks: [docs/android-install.md](docs/android-install.md). No real-phone/radio verification yet; B2–B7 remain open.
- `logos/zcash_offline_module` is the ONE shared Basecamp engine (logos-module-builder `8f56bf8`, Rust 1.97.1), with THREE thin `ui_qml` apps: `zcash_offline_ui` (wallet/pay/mesh), `zcash_offline_organiser` (fleet/invite export, optional join), `zcash_offline_gateway` (chain service check, gateway start/settings, balance/submissions). All depend on the same module; never make an engine per role. `fleet_init` lives in the ozm-node library. Roles come from the trusted organiser invite/fleet, not installed apps; the current enrollment file is not organiser-signed. See `logos/README.md`. Build each with `nix build .#lgx` / `.#lgx-portable`, sequentially on small orbs.
- `bench/mesh-bench.sh all` runs the seven gating tests on four containers against the running `ths` chain ([docs/mvp-v0.1.0.md](docs/mvp-v0.1.0.md)); `bench/lan-bench.sh all` tests invited discovery, sync and funds ([docs/mvp-v0.2.0.md](docs/mvp-v0.2.0.md)); `bench/open-bench.sh all` tests six invite-free devices, address payments and Max; `bench/ffi-bench.sh` tests Kotlin-only links against the real chain. Releases: keep workspace/Logos/Android versions in sync; v0.7.2 ships **four `.lgx` packages × three platforms plus four merged catalogue packages**, CLI tarballs and Android APK + checksums on a `v*` tag. Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` before committing.
- The owner tested v0.5.1 successfully on three real Macs; v0.7.0 needs retesting there. Basecamp Local Network permission support is pending upstream ([logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527)); fallback: dial `<operator-ip>:7777`. All three session laptops can run only Basecamp for device roles; the Regtest operator's Docker-backed `ths start` remains outside Basecamp. Public-testnet gateways use TLS lightwalletd, not ths or a full node.
- Invite-free wallets create local keys, select by hops then advertised tip, and pin the first used gateway; changes are blocked/warned. All envelopes and delivery acks authenticate their sender. Address bindings prove receiving (not spending) authority and pin the mesh key at first receipt routing. Open/unknown payers never get pending acceptance; inclusion, sync and mining remain gateway-reported. Invited gateways retain fleet authority; keep Regtest-only limits explicit ([docs/basecamp-install.md](docs/basecamp-install.md#open-mode-trust-and-faucet-limits)).
- v0.7.2 inventory needs a fresh key proof: older peers only live-forward, so upgrade store-and-forward peers together. New submissions use persisted nonces instead of counterpart names; signed plaintext broadcasts reveal txid/status (P12). Unix files, mesh resources and gateway work have finite limits; invited pending caps are provisional test-coin policy. See [privacy-and-threats.md](docs/privacy-and-threats.md#trust-assumptions-at-v070) for exact residual risks.
- Every user-visible change adds an entry under `Unreleased` in `CHANGELOG.md`; releases move those entries into the new version section.
- Do not include Amp thread links in commit messages, tracked files or release notes. Automatic thread attribution is disabled in `.amp/settings.json`.

## Current target

- **Current network: public testnet**, NU7 from **4,465,026**, branch `0x77190AD9`, v6 Ironwood, about 25-second blocks. Fleet `network` defaults to Regtest for backward compatibility. A zero birthday option resolves to 1 on Regtest and 4,465,026 on testnet; existing fleet settings are authoritative. Seed the tree at birthday−1 before compact scanning.
- **Gateway is a light client**, not a full node or VPS. Configurable TLS servers: `https://testnet.zec.rocks:443`, `https://zaino.testnet.unsafe.zec.rocks:443` (503 on 27 October). Compare reachable servers; warn on one source. No gateway-side viewing-key scanning. Keep JSON-RPC/mining/faucet only for the ths Regtest benches. Testnet uses external TAZ faucets and Prepare for offline self-splits, never unmined change.
- **v0.7.0 trust exception:** `included` at 1 and `mined` at 10 confirmations are explicitly gateway-reported, not independently proved. TLS lightwalletd chain state is trusted. PoW/MMR/inclusion proofs (tasks 7–9) remain deferred; ten confirmations are not finality. Persist 200 ShardTree checkpoints/roots, rewind on reorg, push result changes and new tips, retry unchanged bytes every 3 blocks until expiry. Sync/control padding uses 512 B / 4 KiB / 16 KiB buckets; payment/submission/receipt padding stays unchanged. Record compact payload separately from full framed wire traffic.
- **Stable-link traffic:** coalesce height/hash tips for 15 minutes or 200 blocks; exchange checkpoint hashes only on divergence. Signed presence is immediate on link-up, otherwise hourly with three-hour expiry. Gateway change/loss can take an hour to notice on stable links (expiry after missed beacons can take three). The dated replay measures 2.40 MB/day total BLE-link framing, 2.12 MB sync-only; this is not a physical-radio capture. See [mesh evidence](docs/components/mesh.md#v070-compact-sync).
- **Mainnet.** Mainnet activates NU7 at height **3,543,405**, planned for around 5 November 2026 ([ZIP 259](https://zips.z.cash/zip-0259); [Zebra 7.0.0](https://zfnd.org/zebra-7-0-0-release)), so MVP v1 and mainnet both use the NU7 branch ID. A transaction signed under NU6.3 and still unmined at activation becomes invalid. Keep the branch ID, expiry delta and fee in configuration, never hard-coded. Mainnet uses real ZEC: get explicit approval before any mainnet transaction.
- **Regtest environment:** [zcashlabs/thus-spoke-zakura](https://github.com/zcashlabs/thus-spoke-zakura) (`ths`) **v0.4.1**, which runs a Zakura node, lightwalletd and funded development accounts in Docker.
- **Platform order:** laptops first for the current experiments, then mobile. iOS and Android are both candidates for the first phone target; the owner said (26 October 2026) that iOS is not necessarily first. Testers today run `ozm-node` or the Logos Basecamp app on laptops ([docs/basecamp-install.md](docs/basecamp-install.md); Linux needs glibc 2.39).
- **Transport:** our own `ozm-mesh` with [BitChat](https://github.com/permissionlesstech/bitchat) semantics over TCP/LAN and a native Android BLE link; real-radio gates are next. The BitChat-fork plan is archived.

## thus-spoke-zakura issue log

Record every `ths` bug, missing feature or workaround here as you hit it, so it can be fixed or implemented in `ths` separately. Use one entry per issue with: what happened, where in `ths` (file and line, with the commit or release), the workaround used here, and the upstream fix or feature that would remove the workaround. Mark an entry **Resolved** with the `ths` release that fixed it instead of deleting it.

### 1. Network-upgrade heights are hard-coded (missing feature)

- **What:** fresh instances on v0.4.0 and later activate every upgrade through NU7 at height 1, with no CLI flag or config option. That suits MVP v1, but there is no way to create a chain that matches mainnet today (NU6.3 active, NU7 not), which is needed to rehearse the mainnet step locally.
- **Where:** `crates/ths-server/src/network.rs` L3–21 (`nu7: one`) and `crates/ths-server/src/bootstrap/config.rs` L50 (`"NU7" = 1`), at v0.4.1 (`4523e94`). The unit test `selects_nu7_from_the_first_regtest_block` asserts it.
- **Workaround:** none needed for MVP v1. For a mainnet-like chain, either run v0.3.0 (NU6.3 only, but missing later features) or patch v0.4.1 locally to `nu7: None`, drop the `"NU7" = 1` line, and rebuild images with `cargo run -p thus-spoke-zakura -- build --dev`.
- **Upstream fix:** a `ths start` option to choose the last activated upgrade or per-upgrade heights, for example `--network-upgrades nu6.3`, with both height definitions generated from one source (the code comments say they must be kept in sync by hand).

### 2. Prebuilt Linux launcher needed glibc 2.39 (bug) — Resolved in v0.4.1

- **What:** the v0.3.0 Linux launcher failed on Debian 12 (glibc 2.36) with ``version `GLIBC_2.39' not found``. Amp orbs run Debian 12.
- **Status:** the v0.4.1 `ths-x86_64-unknown-linux-gnu` release binary runs on Debian 12 (checked 2026-10-25).

### 3. Host services bind to 127.0.0.1 only (deliberate restriction, blocks phones)

- **What:** `ths` publishes the dashboard, Zakura RPC and lightwalletd on `127.0.0.1` only. Its own `AGENTS.md` requires this ("Bind every host-published service to `127.0.0.1`"). Phones on the same Wi-Fi therefore cannot reach lightwalletd to sync.
- **Workaround:** forward the port from the laptop, for example `socat TCP-LISTEN:9067,fork,bind=<lan-ip> TCP:127.0.0.1:9067`, `adb reverse tcp:9067 tcp:9067` for Android over USB, or an SSH tunnel. Keep it on a trusted network, because the development keys are public.
- **Upstream feature:** an explicit opt-in such as `ths start --bind <lan-ip>` with a warning, so mobile wallets can be tested against a local chain.

### 4. Wallet crate versions differ from other Ironwood code (compatibility note)

- **What:** `ths` v0.4.1 uses the Zakura forks of librustzcash: `zakura-keys`, `zakura-primitives` and `zakura-proofs` `=2.2.0`, `zakura-client-*` `=0.1.0-rc7` (`crates/ths-server/Cargo.toml` L36–40), the same pins as v0.3.0. [chainapsis/vizor-wallet](https://github.com/chainapsis/vizor-wallet) at `3e466fa` uses Zakura `3.0.0`, `zakura-client-*` `0.1.0-rc9` and `zakura-pczt` `0.1.0-rc6`. The logos-co wallet core uses upstream `zcash_primitives` 0.31.0 and `orchard` 0.16.
- **Impact:** the offline builder must agree with the chain on consensus rules, not on crate versions. Whether a transaction built with upstream crates is accepted by a Zakura node (and the reverse) is unverified.
- **Workaround:** none yet; record API differences here as they are hit.

### 5. No raw JSON-RPC command in the release (missing in v0.4.1)

- **What:** `ths rpc` exists on main but not in v0.4.1 (`unrecognized subcommand 'rpc'`). `reorg` and `rollback` are present in v0.4.1.
- **Workaround:** call Zakura JSON-RPC directly, for example `curl -s -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"sendrawtransaction","params":["<hex>"]}' http://127.0.0.1:18232`.
- **Upstream fix:** release `ths rpc`.

### 6. Wallet expiry delta is 40 blocks under NU7 (behaviour to override)

- **What:** a `ths wallet send` mined at height 106 had `expiryheight` 146, a delta of about 40 blocks. ZIP 218 recommends 120 at NU7, and offline mesh carriage needs far more (the report's fleet policy).
- **Where:** `pub const DEFAULT_TX_EXPIRY_DELTA: u32 = 40;` in `zakura-primitives` 2.2.0, `crates/zcash_primitives/src/transaction/builder.rs` L54 ([zakura-core/common v2.2.0](https://github.com/zakura-core/common/blob/b6cf4cefe38fdccc6d2b0dc868f7d6ef01683413/crates/zcash_primitives/src/transaction/builder.rs)), which `ths` v0.4.1 pins. Common 4.0.0 raises the NU7 default to 120 blocks. At NU7's 25-second target, 40 blocks is about 17 minutes.
- **Workaround:** the offline builder sets its own expiry from the fleet policy with `Builder::with_expiry_height`, or passes `Some(h)` to `create_proposed_transactions`.
- **Upstream feature:** a configurable expiry delta on `ths wallet send` and the payment API, so long-expiry transactions can be tested without a separate builder.

### 7. Externally submitted transactions are not mined automatically (behaviour to confirm)

- **Confirmed 26 October 2026 (v0.1.0 bench):** submitted transactions stay in the mempool until a block is generated; the v0.1.0 gateway calls `generate 1` after each submission (`--mine`). Resubmitting answers "transaction already exists in mempool" or "...transaction was committed to the best chain"; the gateway treats both as success.
- **What:** the chain stayed at one height while idle after `sendrawtransaction`; `ths` appears to mine only on demand (`crates/ths-server/src/payments/attempt.rs` L83–97 at v0.4.1, and `ths mine`). A gateway that submits raw bytes must therefore also trigger `generate` or `ths mine` for the demo. [unverified: observed once on 2026-10-25, not reproduced deliberately]
- **Workaround:** the gateway calls `ths mine` (or the `generate` RPC) after each submission.
- **Upstream feature:** an optional auto-mine interval, or mining whenever the mempool is non-empty.

### 8. Regtest headers have synthetic timestamps and no proof of work (limitation to document)

- **What:** Zakura Regtest sets `with_disable_pow(true)` (`zakura-network/src/parameters/testnet.rs` L1424–1425 at `ebda15e`), so blocks carry all-zero 36-byte Equihash solutions and nonces, headers are 177 bytes (mainnet: 1,487), and block times start from a 2011 epoch. A payee cannot exercise proof-of-work or timestamp checks against this chain.
- **Workaround:** the payee's Equihash/difficulty/timestamp check is disabled only under an explicit `regtest-no-pow` flag, which is itself tested with mainnet header vectors (a real header passes, a corrupted one fails); every other bundle check (MMR anchor paths, txid branches, conflict notices, two-gateway agreement, signatures) runs unchanged on Regtest. Never infer the network from the solution length ([docs/components/chain.md](docs/components/chain.md)).
- **Upstream feature:** none expected; record so nobody tries to validate Regtest headers.

### 9. No RPC returns a Merkle path or a list of recent roots (missing feature, Zakura)

- **What:** `gettxoutproof` returns "Method not found"; nothing returns a ZIP 221 history-tree path, and the node keeps only MMR peaks (`zakura-state/src/service/finalized_state/history_tree.rs` L71–85, `prune()` L373–445). `z_gettreestate` gives one height's roots at a time and returns the Sapling root byte-reversed.
- **Workaround:** the gateway rebuilds the history tree from `getblock h 0` plus `z_gettreestate` per height, persists the leaves, rebuilds after `ths reorg` / `ths rollback` and computes paths itself ([`poc/mmr-anchor-poc`](poc/mmr-anchor-poc/README.md)); for inclusion it builds the txid branch from `getblock h 1` and reads `authdigest` from verbose `getrawtransaction`. This is a gateway duty in the design ([docs/components/gateway.md](docs/components/gateway.md)), not a harness-only step.
- **Upstream feature:** a Zakura RPC for a history-tree path at height N (or a leaf list), and a recent-anchors RPC. This is a Zakura issue rather than a `ths` one.

### 10. Maximum-expiry rejection message describes an exclusive bound (wording bug, Zakura)

- **What:** on ths v0.4.1, expiry 500,000,000 is rejected with “must be less than the maximum Height(499999999)”, but **499,999,999 itself is accepted and mined**. The consensus maximum is inclusive; this is misleading error text, not a consensus failure.
- **Where:** Zakura v1.6.0 `ebda15e23ed0189787e21b4625c6e4f62d10932f`, `crates/zakura-consensus/src/transaction/check.rs` L628–645 (bound check; rejection text observed through `sendrawtransaction`). [Measured evidence and source revisions](docs/components/expiry-reissue.md#consensus-expiry-versus-mempool-policy).
- **Workaround:** test/use the inclusive bound from ZIP 203, not the error's “less than” wording. Application zero-expiry refusal and tighter fleet caps remain proposed policy, not node restrictions.
- **Upstream fix:** change the Zakura error wording to “at most” / “less than or equal to” and keep an equality regression case. No launcher/version change needed here.

## Offline builder

Build on Zakura Common **4.0.0**. `ths` v0.4.1 remains on 2.2.0; exchange compatible NU7 transaction bytes across processes, never mixed Rust types. Pin the whole family exactly and relock `logos/zcash_offline_module/rust-lib`. The module uses Rust **1.97.1**. Common 4 uses matching `zakura-note-encryption` and `zakura-pasta-curves`; `ProvingKey::build` returns a Result.

| Crate | Version | Features |
|---|---|---|
| `zakura-protocol` | `=4.0.0` | `local-consensus` |
| `zakura-primitives` | `=4.0.0` | `circuits`, `std`, `multicore` |
| `zakura-orchard` | `=4.0.0` | `circuit` |
| `zakura-keys`, `zakura-note-encryption`, `zakura-pasta-curves` | `=4.0.0` | |
| `zakura-client-backend` | `=0.1.0-rc9.5` | Compact service types and shard serialization |
| `zakura-pczt` (optional, not linked by app) | `=0.1.0-rc9` | |

Import under upstream names, for example `zcash_primitives = { package = "zakura-primitives", version = "=4.0.0" }`. The configured 10,000-zat fleet fee is a floor: builder and verifier require the ZIP 317 padded action-count fee for larger payments/splits. Expiry always comes from fleet policy. Locally scanned tip > expiry unlocks unspent reserved notes; an expiry report alone does not.

Function sequence (source at v2.2.0 / rc7; direct builder compiled/mined in scratch research, wallet-DB and production payee app still unverified; [evidence and pending expiry policies](docs/components/expiry-reissue.md)):

- **Network:** `LocalNetwork { .., nu6_3: Some(1), nu7: Some(1) }` matching the `ths` chain. `BranchId::for_height` then selects NU7.
- **Payer, offline:** `Builder::new(net, target_height, BuildConfig::Standard { ironwood_anchor: Some(anchor), .. })` → `add_ironwood_spend(fvk, note, merkle_path)` (notes must be `NoteVersion::V3`) → `add_ironwood_output(ovk, addr, value, memo)` → `with_expiry_height(h)` → `build(..)` → `BuildResult::transaction()` → `Transaction::write`. With the wallet DB instead: `propose_transfer` → `create_proposed_transactions(.., expiry_height: Some(h))` → `get_transaction(txid)` → `write`.
- **Payee, offline:** `Transaction::read(bytes, branch)` → `ironwood_bundle()` → `decrypt_outputs_with_keys(&[ivk])` → `signature_hash(.., SignableInput::Shielded, ..)` → `BatchValidator::new(&vk)`, `add_bundle`, `validate`. This proves the transaction is well formed and pays the payee; it cannot prove the anchor is a real chain root or that the notes are unspent, so the receipt says "pending".
- **Gateway:** lightwalletd `SendTransaction`, or Zakura `sendrawtransaction` at `127.0.0.1:18232`.

Limits to respect:

- Spending change that has not been mined is not supported (`zakura-client-backend` rc7 `wallet.rs` L1414–1418), so chained offline payments wait for each parent to be mined.
- `DeferredPcztBuilder` (sign before choosing the anchor) exists and accepts v6 only.
- No upstream Swift or UniFFI bindings exist; `ozm-ffi` provides our Kotlin bindings. iOS is not built.
- **Mainnet step:** Common 4 includes NU7 at 3,543,405, but the application supports only Regtest/testnet. Chain-proof, security and real-value approval gates remain; this migration does not authorize mainnet.

## Agent skills

`.agents/skills/` holds [zcashlabs/zcash-skills](https://github.com/zcashlabs/zcash-skills) at `b841047`, copied as the whole `skills/` tree because partial copies break its routing. Use `zcash-payments`, `zcash-protocol`, `zakura`, `zcash-light-client` and `zcash-wallet` for builder work. They do not cover offline signing, PCZT, FROST or mobile bindings; use the crate source for those. Known stale facts at that commit: it says `ths` is 0.4.0 (it is 0.4.1), and `docs/versioned-facts.md` says Regtest has `nu7 = None` (v0.4.x activates NU7 at height 1).

## Running ths in an orb

- `.agents/setup` installs Docker, stable Rust (rustup, `~/.cargo/bin`) and the v0.4.1 launcher at `~/.local/bin/ths` and pulls its images; `.agents/resume` starts the `dockerd` service. Change the pinned version in `.agents/setup`.
- Start the chain with `amp orb services ensure` (the `ths` service in `.amp/services.yaml`, with a dashboard portal). `amp orb service stop ths` deletes the chain.
- Check: `getblockchaininfo` shows `consensus.chaintip = 77190ad9`, NU7 active at height 1, and an `ironwood` value pool.

## When to change the ths version

Stay on v0.4.1 until a needed feature exists only in a later release (for example `ths rpc`) or a fix lands for an issue above. Tell the user before switching and say which feature requires it.

## Useful ths facts

- Development accounts derive from a fixed public mnemonic. `ths` prints each account's unified spending key (hex), which an offline builder can import.
- Account 1 starts with 5 Ironwood ZEC. `ths wallet faucet` and `ths wallet send` fund and pay between accounts 1 to 5. `ths tx <txid>` and `ths block` read the explorer.
- `ths endpoints --json` prints the dashboard (`127.0.0.1:32805`), Zakura RPC (`127.0.0.1:18232`) and lightwalletd (`127.0.0.1:9067`, plaintext) addresses. Zakura reports `"chain": "test"` for this Regtest chain.
- `ths reorg --depth N` and `ths rollback --depth N` test chain reorganizations.
- Measured on v0.4.1 (2026-10-25): `ths wallet send --from 1 --to 2 --amount 1` produced a v6 transaction (header `06000080`, branch bytes `d90a1977`, i.e. `0x77190AD9`) with only Ironwood actions, 2 actions, **9,166 bytes**, a **10,000-zat fee** (5,000 per action) and an expiry about 40 blocks out. The same send on v0.3.0 under NU6.3 was also 9,166 bytes with a 10,000-zat fee.
