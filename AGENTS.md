# AGENTS.md

This repository holds research and, next, a proof of concept for offline Zcash payments. A signed, proven Ironwood transaction moves between devices over a mesh with no internet, and a device with internet broadcasts it. See `reports/Offline Zcash over Logos mesh.md` for the design and `research_notes/` for sources.

## Current target

- **Network:** local Regtest, pre-NU7. NU6.3 (Ironwood, consensus branch ID `0x37A5165B`, v6 transactions) is active and NU7 is never activated. Public testnet is past NU7 (height 4,465,026), so it is not the target.
- **Regtest environment:** [zcashlabs/thus-spoke-zakura](https://github.com/zcashlabs/thus-spoke-zakura) (`ths`), which runs Zakura, lightwalletd and funded development accounts in Docker.
- **Platform order:** mobile (iOS first) unless something below becomes a blocker, in which case desktop first. BitChat's iOS and macOS apps share one codebase, so a Mac is the fallback test target.
- **Transport:** a fork of [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat). Logos nodes come later.

## thus-spoke-zakura issue log

Record every `ths` bug, missing feature or workaround here as you hit it, so it can be fixed or implemented in `ths` separately. Use one entry per issue with: what happened, where in `ths` (file and line, with the commit or release), the workaround used here, and the upstream fix or feature that would remove the workaround. Mark an entry **Resolved** with the `ths` release that fixed it instead of deleting it.

### 1. No way to create a pre-NU7 chain (missing feature)

- **What:** fresh instances on v0.4.0 and later activate every upgrade through NU7 at height 1. The heights are hard-coded, with no CLI flag or config option.
- **Where:** `crates/ths-server/src/network.rs` L3–21 (`nu7: one`) and `crates/ths-server/src/bootstrap/config.rs` (`"NU7" = 1` in the generated Zakura config), at `233f9c3` (main, 2026-10-22). The unit test `selects_nu7_from_the_first_regtest_block` in `tests/unit/bootstrap/config/tests.rs` asserts it.
- **Workaround:** use release v0.3.0, which activates NU6.1, NU6.2 and NU6.3 at height 1 and not NU7. It uses Zakura 1.6.0, and the zakura wallet crates `2.2.0`. The alternative is a local patch of main that sets `nu7: None` and drops the `"NU7" = 1` line, then rebuilds images with `cargo run -p thus-spoke-zakura -- build --dev`.
- **Upstream fix:** a `ths start` option to choose the last activated upgrade or per-upgrade heights, for example `--network-upgrades nu6.3`. The two height definitions should also come from one source, since the code comments say they must be kept in sync by hand.

### 2. Prebuilt Linux launcher needs glibc 2.39 (bug)

- **What:** the v0.3.0 Linux launcher from `install.sh` fails on Debian 12 (glibc 2.36): ``version `GLIBC_2.39' not found``. Amp orbs run Debian 12.
- **Where:** release binaries for v0.3.0 installed by `install.sh`.
- **Workaround:** build the launcher from source at the release tag: `cargo build --release -p thus-spoke-zakura --bin ths` (toolchain 1.98.0 from `rust-toolchain.toml`).
- **Upstream fix:** build Linux release binaries against an older glibc (for example on Debian 12 or with a musl target) or document the minimum glibc.

### 3. Host services bind to 127.0.0.1 only (deliberate restriction, blocks phones)

- **What:** `ths` publishes the dashboard, Zakura RPC and lightwalletd on `127.0.0.1` only. Its own `AGENTS.md` requires this ("Bind every host-published service to `127.0.0.1`"). Phones on the same Wi-Fi therefore cannot reach lightwalletd to sync.
- **Workaround:** forward the port from the laptop, for example `socat TCP-LISTEN:9067,fork,bind=<lan-ip> TCP:127.0.0.1:9067`, `adb reverse tcp:9067 tcp:9067` for Android over USB, or an SSH tunnel. Keep it on a trusted network, because the development keys are public.
- **Upstream feature:** an explicit opt-in such as `ths start --bind <lan-ip>` with a warning, so mobile wallets can be tested against a local chain.

### 4. Wallet crate versions differ from Vizor's (compatibility note)

- **What:** `ths` v0.3.0 uses the zakura `2.2.0` crates, `zakura-client-*` `0.1.0-rc7` and `zakura-pczt` `0.1.0-rc4`. The reference wallet code in [chainapsis/vizor-wallet](https://github.com/chainapsis/vizor-wallet) at `3e466fa` uses `3.0.0`, `0.1.0-rc9` and `0.1.0-rc6`.
- **Impact:** the offline builder must agree with the chain on consensus rules, not on crate versions, so this is not a bug. Code copied from Vizor may need API changes to compile against the versions `ths` uses, or the builder can use Vizor's versions as long as they produce valid NU6.3 transactions.
- **Workaround:** none yet; record API differences here as they are hit.

### 5. v0.3.0 lacks `ths rpc`, `reorg` and `rollback` (missing in the pinned release)

- **What:** these commands exist on main but not in v0.3.0, which is the release we pin for a pre-NU7 chain (issue 1).
- **Workaround:** call Zakura JSON-RPC directly, for example `curl -s -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"sendrawtransaction","params":["<hex>"]}' http://127.0.0.1:18232`. There is no workaround yet for reorg testing on v0.3.0.
- **Upstream fix:** resolved by issue 1. Once main can create a pre-NU7 chain, we can move off v0.3.0 and get these commands back.

## Running ths in an orb

- Start Docker: `amp orb service start dockerd --command 'sudo dockerd'`, then `sudo chmod 666 /var/run/docker.sock`.
- Build the launcher at v0.3.0 (issue 2), install it to `~/.local/bin/ths`, run `ths pull`, then `amp orb service start ths --command "$HOME/.local/bin/ths start --no-open"`.
- Check: `getblockchaininfo` shows `consensus.chaintip = 37a5165b`, upgrades end at NU6.3, and an `ironwood` value pool exists.

## Useful ths facts

- Development accounts derive from a fixed public mnemonic. `ths` prints each account's unified spending key (hex), which an offline builder can import.
- Account 1 starts with 5 Ironwood ZEC. `ths wallet faucet` and `ths wallet send` fund and pay between accounts 1 to 5.
- `ths endpoints --json` prints the dashboard (`127.0.0.1:32805`), Zakura RPC (`127.0.0.1:18232`) and lightwalletd (`127.0.0.1:9067`, plaintext) addresses. Zakura reports `"chain": "test"` for this Regtest chain.
- Measured on v0.3.0 (2026-10-25): `ths wallet send --from 1 --to 2 --amount 1` produced a v6 transaction (header `06000080`, branch `37a5165b`) with only Ironwood actions, 2 actions, **9,166 bytes**, and a **10,000-zat fee** (5,000 per action). The offline builder should match that fee so its transactions do not stand out.
