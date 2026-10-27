# Logos Basecamp modules

The same device as `ozm-node`, packaged for [Logos Basecamp](https://docs.logos.co/basecamp):

| Module | Type | What |
|---|---|---|
| [`zcash_offline_module`](zcash_offline_module) | core (Rust, cdylib) | Runs the `ozm-node` device in-process: mesh with LAN discovery, wallet, payer, payee, relay and gateway roles |
| [`zcash_offline_ui`](zcash_offline_ui) | ui_qml | Setup (paste invite), wallet, pay, receive and payments screens. It talks only to the core module, because Basecamp sandboxes UI apps |
| [`zcash_offline_organiser`](zcash_offline_organiser) | ui_qml | Create a session, select/combine device roles, export invites to a folder, optionally join this device |
| [`zcash_offline_gateway`](zcash_offline_gateway) | ui_qml | Join a gateway invite, check chain RPC, start/update gateway and Regtest mining, show funding address/balance and submissions |

A Basecamp device works with plain `ozm-node` devices in the same fleet, with no
wire changes. A three-laptop session can use only Basecamp for all device roles;
the operator still runs the Docker-backed `ths start` chain outside Basecamp.
Install the service once and any combination of the three apps. They all depend
on the **same module name**, so Basecamp loads one core process/identity, wallet
and mesh per user directory, not one per app. Closing an app leaves that shared
dependency loaded ([Basecamp lifecycle spec](https://github.com/logos-co/logos-basecamp/blob/f2fae630737bc56ef8c4bb33ce92ae365ba8abb9/docs/spec.md#ui-app-lifecycle)).
Installing Gateway never grants authority: its controls and the engine reject
non-gateway invites. The existing fleet/invite format is trusted organiser setup,
not organiser-signed enrollment; it is unchanged in v0.5.0.

## Module API

Every method returns a JSON string, and failures are `{"error": "..."}`. The device lives in `<instance persistence path>/ozm`.

| Method | Returns |
|---|---|
| `fleet_init(options_json)` | `{folder, fleet_path, devices:[{name, role, peer_id, address, invite_path, secret_path}], joined?}`. Options: `out` folder, `name` session, `devices` (`["Operator:gateway", "Alice:payer+payee", ...]`), optional `join_device` name. Refuses existing fleet/key files; joins but does not start |
| `check_rpc(rpc_url)` | `getblockchaininfo` result, or `error`; no role grant or startup |
| `join(invite_json)` | `{"ok":true,"message":...}` |
| `start(options_json)` | Status. Options are all optional: `listen`, `dial` (list), `rpc`, `mine`, `discover`, `ble` (Linux/BlueZ only), `discovery_port`, `api` (also serves the web UI). Saved for restart. On an already running device, explicit `rpc`/`mine` update the gateway in place; transport options remain unchanged |
| `status()` | As `GET /api/status`, plus `joined`, `running` and `home`; `rpc`, `mine` and invite `role` describe gateway configuration |
| `fleet()`, `list_payments()` | As `GET /api/fleet` and `GET /api/payments` |
| `pay(to, amount_zec)` | The payment |
| `sync_now()`, `request_funds(notes, amount_zec)` | As `POST /api/sync` and `POST /api/fund` |
| event `node_event(event, detail_json)` | Every node event (`built`, `receipt`, `result`, `synced`, `neighbor-up`, ...) |

## Build

All four packages need Nix with flakes; the Logos binary cache (`https://cache.nix.logos.co/public`) makes them much faster. Every flake pins logos-module-builder `8f56bf8`; UI flakes use a path input for the shared core. Nix only sees files that git tracks.

```bash
for m in zcash_offline_module zcash_offline_ui zcash_offline_organiser zcash_offline_gateway; do
  (cd "logos/$m" && nix build .#lgx --accept-flake-config --cores 1)
done
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

**v0.5.0, released Basecamp 0.3.2 AppImage, Ubuntu 24.04 container under Xvfb:**
all four portable packages installed through the real package manager. The
operator loaded all three apps, created the fleet in Organiser and joined as
`Operator:gateway`. Gateway checked `getblockchaininfo`, started with mining,
and served five 0.1 ZEC test notes after its address was funded with `ths faucet`.
Two other Basecamp user directories had only the node app when they joined.
Alice paid Bob 0.01 ZEC through the UI; both persisted `mined` for txid
`07f89240870ecfbab518b168a1c2267d85eac8850292e393ca7328ef902bd5d0`
at height 113. Gateway displayed the same submission and result.

Loading all three operator apps produced **one** `logos_host --name
zcash_offline_module` process and one TCP listener on 7777. Applying mining
off/on preserved that process and identity. Restarting Basecamp restored the
same peer ID, RPC, mining option and submission history. Installing Gateway
on Alice afterwards displayed the role error; clicking its disabled Start did
not change Alice's offline configuration. A bad RPC check showed an error and
recovered after restoring the URL. All four packages built as both `.#lgx` and
`.#lgx-portable` on Linux x86_64. These runs use TCP, not Bluetooth radios.

**macOS `.lgx` packages have not yet run on a real Mac.** Local Network permission
support is pending upstream ([logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527));
the fallback is **Dial address** = `<operator-ip>:7777`. Linux verification does
not establish macOS discovery, sandbox permissions or BLE support.
