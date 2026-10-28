# Logos Basecamp modules

The same device as `ozm-node`, packaged for [Logos Basecamp](https://docs.logos.co/basecamp):

| Module | Type | What |
|---|---|---|
| [`zcash_offline_module`](zcash_offline_module) | core (Rust, cdylib) | Runs the `ozm-node` device in-process: mesh with LAN discovery, wallet, paying, receiving, relaying and the gateway |
| [`zcash_offline_ui`](zcash_offline_ui) | ui_qml | Open start or optional invite, wallet, address payments, Max, receive and payments screens, Leave session. It talks only to the core module, because Basecamp sandboxes UI apps |
| [`zcash_offline_organiser`](zcash_offline_organiser) | ui_qml | Create a session, mark which devices are gateways (at least one), export invites to a folder, optionally join this device, Leave session |
| [`zcash_offline_gateway`](zcash_offline_gateway) | ui_qml | Join a gateway invite, check chain RPC (Regtest) or show the fleet's lightwalletd servers (testnet), start/update gateway and Regtest mining, show funding address/balance and submissions, Leave session |

A Basecamp device works with plain `ozm-node` devices in open or invited mode.
New signed announcements/authentication append message kinds; v0.5.x nodes
drop unsupported messages without crashing. Upgrade payment endpoints together:
v0.6.0 rejects unsigned legacy envelopes and acks, including invited peers.
Since v0.7.3 every device beacons a zero fleet tag and connects to any Offline
Zcash device; v0.7.2 peers accept it.
A three-laptop session can use only Basecamp for every device;
the operator still runs the Docker-backed `ths start` chain outside Basecamp.
Install the service once and any combination of the three apps. They all depend
on the **same module name**, so Basecamp loads one core process/identity, wallet
and mesh per user directory, not one per app. Closing an app leaves that shared
dependency loaded ([Basecamp lifecycle spec](https://github.com/logos-co/logos-basecamp/blob/f2fae630737bc56ef8c4bb33ce92ae365ba8abb9/docs/spec.md#ui-app-lifecycle)).
Every device can pay, receive and relay; `gateway` is the only role. Installing
Gateway never grants it: its controls and the engine refuse to run a gateway
unless the device's invite names it the fleet's gateway. The existing fleet/invite format is trusted organiser setup,
not organiser-signed enrollment. Open wallets generate keys locally; only gateway
operators still require a gateway invite. Invited devices trust their fleet's
named gateway; open devices pin the first gateway they use, and changes are
blocked and warned. Address bindings prove receiving authority
(without revealing a viewing key) and pin the mesh key at first receipt routing.
Open/unknown payers wait for gateway-reported inclusion, not pending acceptance.
These are not chain proofs or spending-authority proofs.
[Selection and trust model](../docs/basecamp-install.md#open-mode-trust-and-faucet-limits).

## Install by name

Released Basecamp 0.3.2 supports custom catalogues under **Settings → Package
Repositories**. Add this public URL, Refresh, then search `zcash` in **Manage
Packages** and install the service and your chosen apps:

```text copy Copy Catalogue URL
https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
```

For released `logosctl` (its own session, separate from Basecamp):

```sh copy Copy Catalogue Install
logosctl daemon start --detach
logosctl catalog add https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
logosctl catalog refresh
logosctl package search zcash
logosctl package install zcash_offline_module --yes
logosctl package install zcash_offline_ui --yes
logosctl package install zcash_offline_organiser --yes
logosctl package install zcash_offline_gateway --yes
logosctl module load zcash_offline_module
logosctl call zcash_offline_module status
```

The catalogue selects the newest release; add `--version <version>` (for example
`0.7.3`) to pin a CLI install. No GitHub authentication is required. Content roots
bind the index to the downloaded packages but do not authenticate the publisher
(`trustedSigners` is empty). Packages are unsigned experimental testnet/Regtest
builds: expect a warning, not a trusted Logos publisher signature. The catalogue
uses the upstream schema-v2 index generator, full manifest and Merkle root
bindings; it merges the three portable variants into one package per name.
The stable identity card points to the latest release's `index.json`. Each
release also includes four merged LGXs, twelve platform-specific LGXs, icon
sidecars and `catalogue.sha256`. From v0.7.4 these Basecamp packages are the only release assets (macOS and Linux); CLI tarballs and the APK are not published for now.

Manual `.lgx` installation remains a [fallback](../docs/basecamp-install.md#manual-lgx-fallback).
The official-catalogue submission is [prepared separately](../docs/logos-catalogue-submission.md).

## Module API

Every method returns a JSON string, and failures are `{"error": "..."}`. The device lives in `<instance persistence path>/ozm`; Basecamp sets that path to `<AppDataLocation>/module_data/zcash_offline_module/<instance-id>` ([per-platform paths](../docs/basecamp-install.md#where-the-data-lives)). `status()` also returns `sessions`, the default Organiser export parent.

The module declares `concurrency: "multi"`, so calls run on a worker pool: `status` answers while a proof, a chain check or Leave runs. `start`, `join`, `leave` and a joining `fleet_init` are serialized with each other.

| Method | Returns |
|---|---|
| `fleet_init(options_json)` | `{folder, fleet_path, devices:[{name, role, peer_id, address, invite_path, secret_path}], joined?}`. Options: `out` folder (Organiser suggests `<instance persistence path>/sessions/session-YYYYMMDD-HHMMSS`), `name` session, `devices` (`["Operator:gateway", "Alice", "Bob"]`: `NAME` or `NAME:gateway`, at least one gateway; legacy roles such as `payer+payee` are accepted and ignored), optional `network`, `join_device`. Refuses existing fleet/key files; joins but does not start |
| `check_rpc(rpc_url)` | Testnet TLS lightwalletd status for the fleet's network, or Regtest `getblockchaininfo`; endpoint may be a comma-separated TLS server list. On a Regtest session only a plain `http://` Zakura JSON-RPC URL is accepted: `https://` is refused (switching to testnet means `leave()` and a testnet invite) and lightwalletd `:9067` is refused as not JSON-RPC. No role grant or startup |
| `open_fleet_folder(folder)` | `{ok, folder, message}` after requesting the desktop folder opener; validates `fleet.json`. Runs in the core because QML's sandbox blocks local file URLs. Copy the displayed path if no file manager is available |
| `join(invite_json)` | `{"ok":true,"message":...}` |
| `start(options_json)` | Status. Without an installed invite, creates persistent local keys and an open wallet with an automatic per-device scan start. Options are all optional: `network` (default testnet), `listen`, `dial` (list), `rpc`, `mine` (Regtest only), `discover`, `ble` (Linux/BlueZ only), `discovery_port`, `api` (also serves the web UI). Saved for restart. Existing fleets retain their network and wallets their stored scan start. On an already running device, explicit `rpc`/`mine` update the gateway in place; transport options remain unchanged |
| `status()` | As `GET /api/status`, plus `joined`, `running`, `home`, `sessions`, `syncing` and `sync_error`; `rpc` and `mine` describe gateway configuration, `chain_tip` is the gateway's last chain tip, and `role` is `"gateway"` or `""` |
| `leave()` | Stops the node, moves the whole `<instance persistence path>/ozm` folder to `<instance persistence path>/ozm-backup-YYYYMMDD-HHMMSS` (UTC), moves any older `ozm/sessions` exports to `<instance persistence path>/sessions`, and returns the device to not joined. It cancels a running wallet sync and waits for an in-flight payment, then stops every writer before moving the folder. Nothing is deleted; to restore, quit Basecamp and move the backup back to `ozm`. Basecamp-only: not on Android or in the `ozm-node` CLI (use another `--home`). Platform paths: [Where the data lives](../docs/basecamp-install.md#where-the-data-lives) |
| `fleet()`, `list_payments()` | As `GET /api/fleet` and `GET /api/payments` |
| `pay(to, amount_zec)` | The payment; `to` is a unified address for the fleet network with an Ironwood receiver. Legacy fleet names remain accepted for scripts. An unknown mesh address submits without a payee receipt |
| `max_payment()` | `{amount, amount_zat, fee_zat}` at the same ZIP 317 padded action count as the builder; excludes locked/unmined notes and uneconomic inputs |
| `prepare_offline(notes, amount_zec)` | The self-split payment, using one mined note and the action-count fee. Wait for gateway-reported mined and wallet sync before going offline. Since v0.7.5, `pay`, Max and `prepare_offline` remain available during sync, using only locally scanned spendable notes |
| `sync_now()` | Optional refresh: `{"syncing":true}` at once; automatic sync already runs on opening, discovery and new tips. `status().syncing`, `ready`, `sync_error` and the `synced` / `sync-error` events report it. Module calls never wait for it |
| `request_funds(notes, amount_zec)` | As `POST /api/fund` |
| event `node_event(event, detail_json)` | Every node event (`built`, `receipt`, `result`, `synced`, `neighbor-up`, ...) |

## Build

All four packages need Nix with flakes; the Logos binary cache (`https://cache.nix.logos.co/public`) makes them much faster. Every flake pins logos-module-builder `8f56bf8`; UI flakes use a path input for the shared core. Nix only sees files that git tracks.

```bash
for m in zcash_offline_module zcash_offline_ui zcash_offline_organiser zcash_offline_gateway; do
  (cd "logos/$m" && nix build .#lgx --accept-flake-config --cores 1)
done
```

The module flake copies `crates/{zcash-offline-core,ozm-mesh,ozm-ble,ozm-node}` into the crate before building, because the builder copies only `rust-lib/`. For plain `cargo` in `rust-lib/`, run `dev-stage.sh` first. Do not run clippy on `rust-lib/`: the generated scaffold has lints. Linux BLE statically bundles libdbus using Cargo's vendored feature; BlueZ and system D-Bus remain host services. [Linux Bluetooth setup and hardware limits](../docs/basecamp-install.md#bluetooth-on-linux).

To build packages for released Basecamp from a fresh clone (about 10 GB free; the first build can take a while):

```sh copy Copy Build Commands
git clone https://github.com/megabyte0x/offline-zcash
cd offline-zcash
for m in zcash_offline_module zcash_offline_ui zcash_offline_organiser zcash_offline_gateway; do
  (cd "logos/$m" && nix build .#lgx-portable --accept-flake-config --cores 1 -o "../../out-$m")
done
ls out-*/*.lgx
```

`.#lgx` builds the `-dev` variant, which dev builds of Basecamp and `logosctl` accept. `.#lgx-portable` builds the variant that released Basecamp builds accept. Both already contain content hashes with the pinned builder; no bundler replacement is needed. Release packages remain portable-only: a dual package would still require the development machine's matching Nix closure, so build `.#lgx` locally for dev runtimes.

## Run

Under `logosctl`, one daemon per device:

```bash
logosctl --config-dir ./a daemon start --detach
logosctl --config-dir ./a package install --file logos-zcash_offline_module-module-lib.lgx -y
logosctl --config-dir ./a module load zcash_offline_module
logosctl --config-dir ./a call zcash_offline_module start 'str:{"listen":"0.0.0.0:7801"}'
logosctl --config-dir ./a call zcash_offline_module request_funds 3 str:0.1
logosctl --config-dir ./a call zcash_offline_module pay 'str:<recipient-regtest-address>' str:0.05
logosctl --config-dir ./a call zcash_offline_module max_payment
logosctl --config-dir ./a watch zcash_offline_module --json
```

Strings need the `str:` prefix on the command line, because otherwise `0.05` is passed as a number.

In Basecamp, follow [docs/basecamp-install.md](../docs/basecamp-install.md). For a dev build of Basecamp, install the `-dev` packages with `lgpm --modules-dir <basecamp dir>/modules --ui-plugins-dir <basecamp dir>/plugins install --file ...`.

## Funded verification (28 October 2026, Linux, Basecamp 0.3.2)

Owner-funded public-testnet wallets exercised ordinary gateway sends, catch-up
refusal/recovery, an automatic eight-note split, an offline payment using a
mined split note, Max, restart and gateway reconnect. That offline payment reached
ten gateway-reported confirmations on both wallets. Simple/Debug views and an
800px stacked layout were rendered and inspected. See [the recorded cases,
fixes and limitations](../docs/open-work.md#built) and the reusable
[`testing-basecamp-testnet` skill](../.agents/skills/testing-basecamp-testnet/SKILL.md).
This is shared-orb TCP, not physical Wi-Fi, macOS, Bluetooth or independent chain proof.

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

The final core/Organiser packages also opened the exported folder in PCManFM,
including a `#` in the path; opening runs through the core, not a sandboxed
QML file URL. A second UI payment of 0.0137 ZEC mined at height 135 on both
wallets after reinstalling those packages, with the same gateway identity.

The owner tested **v0.5.1 successfully on three real Macs**. Later versions,
including this v0.7.5 candidate, need retesting on those machines. Local Network permission
support is pending upstream ([logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527));
the fallback is **Dial address** = `<operator-ip>:7777`. Linux verification does
not establish macOS discovery, sandbox permissions or BLE support.

**v0.6.0, released Basecamp 0.3.2, Ubuntu 24.04 container under Xvfb:** all four
packages built sequentially as both `.#lgx-portable` and `.#lgx`. The final portable
core was loaded in all three installed user directories (matching library SHA-256);
the two wallet directories contained only the Node app. Both started without an
invite or dial, generated their own identities, selected/pinned Operator and synced
the existing chain. A requested five 0.1 ZEC notes through the UI, then paid B's
address 0.0137 ZEC. B used Max to return 0.0136 ZEC, with the builder's 0.0001 ZEC
fee. Both wallets and independent RPC lookups showed the two transactions mined
at heights 286 and 287. B ended with zero spendable/locked balance; A retained
0.4998 ZEC. Gateway displayed both submissions; each user had one core process.
Restarting B preserved its secret, gateway/address pins and mined payments.
Screenshots and a moving-pointer Max recording were inspected. This is packaged
Linux/TCP verification, not a macOS, Android or radio run.
