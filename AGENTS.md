# AGENTS.md

This repository holds research and a working proof of concept (releases v0.1.0–v0.3.0) for offline Zcash payments. A signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet broadcasts it. Start at `docs/README.md`: `docs/design.md` is the system view, `docs/components/` holds one file per component (decided, findings, open), and `docs/open-work.md` lists what is built and what to do next; `docs/how-it-works.md` shows the flow in diagrams. `archive/` keeps the original long report, research notes and reviews as deep sources; do not edit them, and when they conflict with `docs/`, `docs/` wins.

## Code (v0.4.0 prepared; not tagged)

- `crates/zcash-offline-core`: wallet sync, offline builder, payee checks, `ozm1` sealing, signed messages. No transport code.
- `crates/ozm-mesh`: BitChat-style mesh (TTL flood, 512-byte frames, store-and-forward, acks) over TCP or attached BLE streams; see its README for the wire format.
- `crates/ozm-node`: library and daemon (roles payer, relay, payee, gateway; commands `fleet-init`, `join`, `start`, `fund`, `pay`), local HTTP API and web UI on `127.0.0.1:7780`. `ozm-mesh` LAN discovery uses UDP 7778 ([docs/components/mesh.md](docs/components/mesh.md)).
- `crates/ozm-ffi` and `android/`: UniFFI/Kotlin node API, Android 10+ arm64 BLE + LAN test shell. Build, install and host-JVM contract checks: [docs/android-install.md](docs/android-install.md). No real-phone/radio verification yet; B2–B7 remain open.
- `logos/zcash_offline_module` (Rust core module for Logos Basecamp, logos-module-builder `8f56bf8`, Rust 1.95) and `logos/zcash_offline_ui` (`ui_qml` app); see `logos/README.md`. Build with `nix build .#lgx`.
- `bench/mesh-bench.sh all` runs the seven gating tests on four containers against the running `ths` chain ([docs/mvp-v0.1.0.md](docs/mvp-v0.1.0.md)); `bench/lan-bench.sh all` tests discovery, sync and funds over the mesh ([docs/mvp-v0.2.0.md](docs/mvp-v0.2.0.md)); `bench/ffi-bench.sh` tests Kotlin-only links against the real chain. Releases: keep workspace/Logos/Android versions in sync; CI builds CLI tarballs, `.lgx` and APK + checksum on a `v*` tag. **The owner will tag v0.4.0 only after a fresh orb follows the Android guide.** Run `cargo clippy --workspace --all-targets` and `cargo test --workspace` before committing.

## Current target

- **MVP v1 network:** a local Regtest chain with NU7 active (consensus branch ID `0x77190AD9`, v6 transactions, Ironwood shielded pool). MVP v1 stays on this local chain until the mesh transport, payment communication (receipts, chain-state feedback) and offline transaction building all work end to end.
- **After MVP v1: public testnet, then mainnet.** Testnet activates NU7 at **4,465,026** with the same branch ID and runs the same rules, except the header verifier allows the testnet minimum-difficulty rule (ZIP 218: a block more than 450 s after its parent). Move the builder to Common 4.0 once, before testnet; stay on 2.2.0 for MVP v1. Run our own Zebra + lightwalletd gateway node on testnet.
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

Build on Zakura Common **2.2.0**, the version `ths` v0.4.1 uses. The 2.2.0, 3.x and 4.x type families cannot be mixed in one binary, so pin every crate exactly; `zakura-client-*` rc7 asks for `^2.0.0` and would otherwise float.

| Crate | Version | Features |
|---|---|---|
| `zakura-protocol` | `=2.2.0` | `local-consensus` |
| `zakura-primitives` | `=2.2.0` | `circuits`, `std`, `multicore` |
| `zakura-orchard` | `=2.2.0` | `circuit` |
| `zakura-keys` | `=2.2.0` | |
| `zakura-proofs` | `=2.2.0` | |
| `zakura-pczt` (optional) | `=0.1.0-rc4` | |
| `zakura-client-backend`, `zakura-client-sqlite` (optional, for the synced wallet DB) | `=0.1.0-rc7` | |

Import them under their upstream names, as `ths` does, for example `zcash_primitives = { package = "zakura-primitives", version = "=2.2.0" }`.

Function sequence (source at v2.2.0 / rc7; direct builder compiled/mined in scratch research, wallet-DB and production payee app still unverified; [evidence and pending expiry policies](docs/components/expiry-reissue.md)):

- **Network:** `LocalNetwork { .., nu6_3: Some(1), nu7: Some(1) }` matching the `ths` chain. `BranchId::for_height` then selects NU7.
- **Payer, offline:** `Builder::new(net, target_height, BuildConfig::Standard { ironwood_anchor: Some(anchor), .. })` → `add_ironwood_spend(fvk, note, merkle_path)` (notes must be `NoteVersion::V3`) → `add_ironwood_output(ovk, addr, value, memo)` → `with_expiry_height(h)` → `build(..)` → `BuildResult::transaction()` → `Transaction::write`. With the wallet DB instead: `propose_transfer` → `create_proposed_transactions(.., expiry_height: Some(h))` → `get_transaction(txid)` → `write`.
- **Payee, offline:** `Transaction::read(bytes, branch)` → `ironwood_bundle()` → `decrypt_outputs_with_keys(&[ivk])` → `signature_hash(.., SignableInput::Shielded, ..)` → `BatchValidator::new(&vk)`, `add_bundle`, `validate`. This proves the transaction is well formed and pays the payee; it cannot prove the anchor is a real chain root or that the notes are unspent, so the receipt says "pending".
- **Gateway:** lightwalletd `SendTransaction`, or Zakura `sendrawtransaction` at `127.0.0.1:18232`.

Limits to respect:

- Spending change that has not been mined is not supported (`zakura-client-backend` rc7 `wallet.rs` L1414–1418), so chained offline payments wait for each parent to be mined.
- `DeferredPcztBuilder` (sign before choosing the anchor) exists and accepts v6 only.
- No upstream Swift or UniFFI bindings exist; `ozm-ffi` provides our Kotlin bindings. iOS is not built.
- **Mainnet step:** Common 2.2.0's `MAIN_NETWORK` has `Nu7 => None` (its `TEST_NETWORK` already has NU7 at 4,465,026). Before mainnet, either move to Common 4.x (which sets 3,543,405) or supply custom mainnet parameters with the NU7 height. This does not block MVP v1.

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
