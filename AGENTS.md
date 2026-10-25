# AGENTS.md

This repository holds research and, next, a proof of concept for offline Zcash payments. A signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet broadcasts it. See `reports/Offline Zcash over Logos mesh.md` for the design and `research_notes/` for sources.

## Current target

- **MVP v1 network:** a local Regtest chain with NU7 active (consensus branch ID `0x77190AD9`, v6 transactions, Ironwood shielded pool). MVP v1 stays on this local chain until the mesh transport, payment communication (receipts, chain-state feedback) and offline transaction building all work end to end.
- **After MVP v1: mainnet.** Mainnet runs NU6.3 (`0x37A5165B`) and has not activated NU7, so moving to mainnet means building with the NU6.3 branch ID unless mainnet activates NU7 first. Keep the branch ID, expiry delta and fee in configuration, never hard-coded. Mainnet uses real ZEC: get explicit approval before any mainnet transaction.
- **Regtest environment:** [zcashlabs/thus-spoke-zakura](https://github.com/zcashlabs/thus-spoke-zakura) (`ths`) **v0.4.1**, which runs a Zakura node, lightwalletd and funded development accounts in Docker.
- **Platform order:** mobile (iOS first) unless something below becomes a blocker, in which case desktop first. BitChat's iOS and macOS apps share one codebase, so a Mac is the fallback test target.
- **Transport:** a fork of [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat). Logos nodes come later.

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
- **Where:** observed with v0.4.1 on 2026-10-25; the delta is set in the Zakura wallet stack the `ths` server uses, not yet traced to a file.
- **Workaround:** the offline builder sets its own expiry from the fleet policy.
- **Upstream feature:** a configurable expiry delta on `ths wallet send` and the payment API, so long-expiry transactions can be tested without a separate builder.

## Running ths in an orb

- Start Docker: `amp orb service start dockerd --command 'sudo dockerd'`, then `sudo chmod 666 /var/run/docker.sock`.
- Install the v0.4.1 launcher (`THS_VERSION=v0.4.1` with `install.sh`, or the `ths-x86_64-unknown-linux-gnu.tar.gz` release asset) to `~/.local/bin/ths`, run `ths pull`, then `amp orb service start ths --command "$HOME/.local/bin/ths start --no-open"`.
- Check: `getblockchaininfo` shows `consensus.chaintip = 77190ad9`, NU7 active at height 1, and an `ironwood` value pool.

## When to change the ths version

Stay on v0.4.1 until a needed feature exists only in a later release (for example `ths rpc`) or a fix lands for an issue above. Tell the user before switching and say which feature requires it.

## Useful ths facts

- Development accounts derive from a fixed public mnemonic. `ths` prints each account's unified spending key (hex), which an offline builder can import.
- Account 1 starts with 5 Ironwood ZEC. `ths wallet faucet` and `ths wallet send` fund and pay between accounts 1 to 5. `ths tx <txid>` and `ths block` read the explorer.
- `ths endpoints --json` prints the dashboard (`127.0.0.1:32805`), Zakura RPC (`127.0.0.1:18232`) and lightwalletd (`127.0.0.1:9067`, plaintext) addresses. Zakura reports `"chain": "test"` for this Regtest chain.
- `ths reorg --depth N` and `ths rollback --depth N` test chain reorganizations.
- Measured on v0.4.1 (2026-10-25): `ths wallet send --from 1 --to 2 --amount 1` produced a v6 transaction (header `06000080`, branch bytes `d90a1977`, i.e. `0x77190AD9`) with only Ironwood actions, 2 actions, **9,166 bytes**, a **10,000-zat fee** (5,000 per action) and an expiry about 40 blocks out. The same send on v0.3.0 under NU6.3 was also 9,166 bytes with a 10,000-zat fee.
