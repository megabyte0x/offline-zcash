# Logos Zcash modules (logos-co) and where they fit the offline design

Scope: the eight Zcash repositories in the [logos-co](https://github.com/logos-co?q=zcash&type=all) organization, read in October 2026, and whether they can serve as the offline builder, the internet gateway, or the transport for offline Ironwood payments. Nothing was built or run; "verified" means read in code at the commits below, "inferred" means my reasoning.

---

## 1. Takeaway

These repos are a desktop Zcash wallet and an in-process Zebra node for Logos Basecamp. They are a good starting point for the **basecamp gateway in the Logos phase**: `logos-zebrad-module` already serves lightwalletd's `SendTransaction` over Logos IPC. They do not fit the **iOS/BitChat MVP**: no mobile targets, no raw-transaction or PCZT export, no mesh or Logos Delivery code, and they use upstream librustzcash rather than the Zakura forks that thus-spoke-zakura uses.

---

## 2. Repositories (verified)

All eight were created 7–8 October 2026 and have 4–39 commits. No repository has a LICENSE file; the Rust crates declare `MIT OR Apache-2.0`.

| Repo | Commit read | Role | Language |
|---|---|---|---|
| [logos-zcash-wallet-core-module](https://github.com/logos-co/logos-zcash-wallet-core-module/tree/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2) | `21b9f06`, 22 Oct | Wallet engine, holds the keys; answers only `zcash_wallet_backend` | Rust |
| [logos-zcash-wallet-backend](https://github.com/logos-co/logos-zcash-wallet-backend/tree/fe8d6a1c) | `fe8d6a1`, 9 Oct | Role broker (custodian, approver); holds no keys | Rust |
| [logos-zcash-wallet-cli](https://github.com/logos-co/logos-zcash-wallet-cli/tree/e4c311c8) | `e4c311c`, 8 Oct | `logosctl call zcash_wallet_cli …` relay to the backend | Rust |
| [logos-zcash-wallet-ui](https://github.com/logos-co/logos-zcash-wallet-ui/tree/dd72f454) | `dd72f45`, 10 Oct | Basecamp wallet app | QML/C++ |
| [logos-zcash-node-module](https://github.com/logos-co/logos-zcash-node-module/tree/952f78b661a5f9ab29c73b28d2a4ef69b327b744) | `952f78b`, 22 Oct | Registry of lightwalletd servers, Tor proxy, route table per call class; does not broadcast | Rust |
| [logos-zebrad-module](https://github.com/logos-co/logos-zebrad-module/tree/a68194507241dd03c633752480f9a8e39cbe1b79) | `a681945`, 22 Oct | Zebra running in-process; `grpc(path, body)` over Logos IPC | C++ |
| [logos-zebra-nix](https://github.com/logos-co/logos-zebra-nix/tree/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba) | `0b5e2ac`, 22 Oct | Patched Zebra built in Nix as the C library `libzebrad_c` | Nix/Rust |
| [logos-zebrad-ui](https://github.com/logos-co/logos-zebrad-ui/tree/1913534d) | `1913534`, 8 Oct | Basecamp app to manage the node | QML/C++ |

Call path: UI or CLI → backend → wallet core; the core asks the node module for routes and reaches the in-process node at `logos://zebrad_module` ([net/ipc.rs L1-14](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/net/ipc.rs#L1-L14)). Callers are gated with `logos::currentCaller()`.

---

## 3. Zcash stack (verified)

- **Upstream librustzcash from crates.io, not Zakura forks:** `zcash_client_backend =0.25.0`, `zcash_client_sqlite =0.23.0`, `zcash_keys =0.17.0`, `zcash_protocol =0.11.0`, `zcash_primitives` and `zcash_proofs =0.31.0`, `zcash_pool_migration =0.2.0`, `pczt =0.10.0`, `orchard 0.16.0`, `sapling-crypto 0.9.0` ([Cargo.toml L16-37](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/Cargo.toml#L16-L37)).
- `zcash_protocol` 0.11.0 defines `Nu6_3` ("Ironwood / NU6.3") as `0x37a5_165b` and NU7 as `0x77190AD9`.
- **Node:** upstream ZcashFoundation Zebra **v7.0.0** by tag, plus lightwalletd v0.5.5 ([zebra-src.nix L6-13](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/nix/zebra-src.nix#L6-L13)). Four patches, none touching consensus, chain, state or network code, enforced by `check-patches.sh` ([README L27-43](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/README.md#L27-L43)): a `run_node(config)` entry point, end-of-support returns an error instead of panicking, batch verifiers created on demand so the node can restart in-process, and the lightwalletd gRPC service handed to the embedding process in memory.

---

## 4. Networks (verified)

- Mainnet, testnet and regtest. Regtest is for test harnesses and turns on when `regtest.json` (with nullable `nu6_3` and `nu7` heights) is in each module's persistence directory ([network.rs L17-56](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/network.rs#L17-L56)). `nu7: null` is supported, so a pre-NU7 chain is representable.
- Tests assert testnet NU7 at 4,465,026 with branch `77190ad9` ([network.rs L133-149](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/network.rs#L133-L149)).
- A plaintext local lightwalletd such as `http://127.0.0.1:9067` works without Tor: loopback, private and `.lan` addresses are dialled directly ([node README L21-40](https://github.com/logos-co/logos-zcash-node-module/blob/952f78b661a5f9ab29c73b28d2a4ef69b327b744/README.md#L21-L40); [engine.rs L56-86](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/engine.rs#L56-L86)). Tor is the default and routes fail closed unless every server is LAN or marked `direct`.
- A server is accepted only if it reports chain `regtest` or `"test"` and its branch matches the tip or tip+1 ([driver.rs L462-472](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/sync/driver.rs#L462-L472)). Zakura on thus-spoke-zakura v0.3.0 reports `"chain": "test"`, which should pass; untested.

---

## 5. Transaction flow (verified)

- `propose` builds an unsigned preview without keys, valid for 120 s (`PREVIEW_TTL_SECS`).
- `approve_send(requestId, password)` runs one `sign_and_send` job: unseal the phrase, prove and sign locally with `LocalTxProver` ([send.rs L326-382](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/send.rs#L326-L382)), then call `SendTransaction` on each broadcast server in turn ([engine.rs L495-545](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/engine.rs#L495-L545)). "Sends signed at approval" refers to this.
- Signing and broadcast are not separable at the API. `sign()` returns the raw bytes internally, but the job result is only `{txid, accepted, server, error}`. No method exports a raw transaction or PCZT (the `pczt` feature is compiled in but unused), and no method accepts a raw transaction to broadcast.
- Expiry before NU7 is target height + 40 blocks, far too short for mesh carriage.
- No Waku, Logos Delivery, mesh or store-and-forward code. Transport is gRPC over Tor or TCP, or Logos IPC.

---

## 6. Platforms (verified)

Linux, macOS and Windows; verified builds are aarch64-darwin, x86_64-linux and Windows 11 ([zebra-nix README L151-159](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/README.md#L151-L159)). No iOS or Android targets. The wallet core also builds as a plain Rust library with `--no-default-features`, and its regtest test drives `Engine::new` directly ([tests/regtest.rs L17-19](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/tests/regtest.rs#L17-L19)), so Basecamp is not required.

---

## 7. Fit for this design

| Role | Fit | What it would need |
|---|---|---|
| Phone MVP (BitChat, iOS) | No | Mobile targets; uses a different crate stack from the MVP |
| Offline builder | Not as-is; the propose→sign path works on a previously synced DB | A sign-only job that returns raw bytes or a PCZT; a configurable long expiry; previews that outlive 120 s |
| Basecamp gateway (Logos phase) | Yes, with small additions. `grpc()` forwards every `CompactTxStreamer` path except `GetMempoolStream`, so `SendTransaction` passes ([grpc.rs L64-111](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/zebrad-c/src/grpc.rs#L64-L111)) | A relay module admitted in `zebrad_module`'s `callers.json` (only the wallet core is allowed by default, [README L87-92](https://github.com/logos-co/logos-zebrad-module/blob/a68194507241dd03c633752480f9a8e39cbe1b79/README.md#L87-L92)); a Logos Delivery transport module; a full Zebra node on the gateway |
| Node module as gateway | No | It routes calls; it does not broadcast |

Also reusable as a pattern: the core's LAN/Tor route table and its two-operator cross-check (`confirm_elsewhere`), which match the report's rule that camps cross-check chain data from two gateways.

---

## 8. Risks and open questions

- **Fork divergence (inferred, the biggest unknown):** these repos use upstream librustzcash and Zebra 7; the MVP uses Zakura crates and a Zakura node. Whether an Ironwood v6 transaction built with one stack is accepted by the other is unverified. Test it before mixing stacks.
- The repos are about two weeks old, change quickly, and have no LICENSE files.
- Caller allowlists in every module mean a relay module needs configuration or upstream changes.
- Logos's own regtest fixture is upstream Zebra 7.0.0 with NU7 at height 200, not Zakura; `regtest.json` heights must match whichever node is used.
