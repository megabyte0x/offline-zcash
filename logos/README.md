# Logos Basecamp modules

The same device as `ozm-node`, packaged for [Logos Basecamp](https://docs.logos.co/basecamp):

| Module | Type | What |
|---|---|---|
| [`zcash_offline_module`](zcash_offline_module) | core (Rust, cdylib) | Runs the `ozm-node` device in-process: mesh with LAN discovery, wallet, payer, payee, relay and gateway roles |
| [`zcash_offline_ui`](zcash_offline_ui) | ui_qml | Setup (paste invite), wallet, pay, receive and payments screens. It talks only to the core module, because Basecamp sandboxes UI apps |

A Basecamp device works with plain `ozm-node` devices in the same fleet, because they use the same wire format. The gateway is usually a plain `ozm-node` on the organiser's laptop ([docs/mvp-v0.2.0.md](../docs/mvp-v0.2.0.md)).

## Module API

Every method returns a JSON string, and failures are `{"error": "..."}`. The device lives in `<instance persistence path>/ozm`.

| Method | Returns |
|---|---|
| `join(invite_json)` | `{"ok":true,"message":...}` |
| `start(options_json)` | Status. Options are all optional: `listen`, `dial` (list), `rpc`, `mine`, `discover`, `ble` (Linux/BlueZ only), `discovery_port`, `api` (also serves the web UI). They are saved, and a restart resumes with them |
| `status()` | As `GET /api/status`, plus `joined`, `running` and `home` |
| `fleet()`, `list_payments()` | As `GET /api/fleet` and `GET /api/payments` |
| `pay(to, amount_zec)` | The payment |
| `sync_now()`, `request_funds(notes, amount_zec)` | As `POST /api/sync` and `POST /api/fund` |
| event `node_event(event, detail_json)` | Every node event (`built`, `receipt`, `result`, `synced`, `neighbor-up`, ...) |

## Build

Both builds need Nix with flakes; the Logos binary cache (`https://cache.nix.logos.co/public`) makes them much faster. Nix only sees files that git tracks.

```bash
(cd logos/zcash_offline_module && nix build .#lgx)   # result/logos-zcash_offline_module-module-lib.lgx
(cd logos/zcash_offline_ui && nix flake update zcash_offline_module && nix build .#lgx)
```

The module flake copies `crates/{zcash-offline-core,ozm-mesh,ozm-ble,ozm-node}` into the crate before building, because the builder copies only `rust-lib/`. For plain `cargo` in `rust-lib/`, run `dev-stage.sh` first. Do not run clippy on `rust-lib/`: the generated scaffold has lints. Linux BLE statically bundles libdbus using Cargo's vendored feature; BlueZ and system D-Bus remain host services. [Linux Bluetooth setup and hardware limits](../docs/basecamp-install.md#bluetooth-on-linux).

`.#lgx` builds the `-dev` variant, which dev builds of Basecamp and `logosctl` accept. `.#lgx-portable` builds the variant that released Basecamp builds accept.

## Run

Under `logosctl`, one daemon per device:

```bash
logosctl --config-dir ./a daemon start --detach
logosctl --config-dir ./a package install --file logos-zcash_offline_module-module-lib.lgx -y
logosctl --config-dir ./a module load zcash_offline_module
logosctl --config-dir ./a call zcash_offline_module join @A.invite.json
logosctl --config-dir ./a call zcash_offline_module start 'str:{"listen":"0.0.0.0:7801"}'
logosctl --config-dir ./a call zcash_offline_module request_funds 3 str:0.1
logosctl --config-dir ./a call zcash_offline_module pay B str:0.05
logosctl --config-dir ./a watch zcash_offline_module --json
```

Strings need the `str:` prefix on the command line, because otherwise `0.05` is passed as a number.

In Basecamp, follow [docs/basecamp-install.md](../docs/basecamp-install.md). For a dev build of Basecamp, install the `-dev` packages with `lgpm --modules-dir <basecamp dir>/modules --ui-plugins-dir <basecamp dir>/plugins install --file ...`.

## Verified (27 October 2026, Linux, dev builds)

- **Under `logosctl`:**
  - Two module instances and a plain `ozm-node` gateway found each other by LAN discovery.
  - A received test funds over the mesh and paid B 0.05 ZEC, and B saw it mined. B then paid A 0.01 ZEC from what it received.
  - `pay` takes 0.34–0.57 s including the proof. Other calls take about 25 ms.
- **In Basecamp:**
  - Device B joined from the Setup screen, synced over the mesh, and received 0.05 ZEC from A, mined at height 200.
  - B then paid A 0.01 ZEC from the Pay panel: A's receipt came back and it was mined at height 202.

- **Released Basecamp 0.3.2 (AppImage), fresh orb, following [docs/basecamp-install.md](../docs/basecamp-install.md):** both portable packages installed through **Manage Packages → Install Local Package**; the unsigned-package warning is the only prompt. Join, test funds, a payment out and a payment in all reached `mined`. The AppImage needs glibc 2.39, so it does not start on Debian 12 or Ubuntu 22.04.

Not yet run on macOS.
