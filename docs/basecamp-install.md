# Offline Zcash in Logos Basecamp

Three apps share **one Offline Zcash service per device**. Install the apps for
the jobs you do; installing several does not create extra identities, wallets,
mesh nodes or listening ports. v0.7.4 targets Basecamp on **macOS (Apple silicon)
and Linux (x86_64, arm64)**, on NU7 public testnet (TAZ) or the local Regtest
chain, with **test coins only**, never real ZEC.

| Your job | Install these packages |
|---|---|
| Hold a wallet, pay, receive and relay | Service + **Offline Zcash** (`zcash_offline_ui`) |
| Create sessions and export invites | Service + **Offline Zcash Organiser** (`zcash_offline_organiser`) |
| Connect the mesh to the chain | Service + **Offline Zcash Gateway** (`zcash_offline_gateway`) |

Every device can pay and receive, and every device relays. The only role is
**gateway**: the organiser marks which devices are gateways, and only a device
whose invite makes it a gateway can run one. Installing the Gateway app does not.
Invites contain private test keys: give each one only to its named owner, and
never accept invites from strangers. The invite format is trusted setup from the
organiser, not organiser-signed enrollment.

## Requirements

- [Basecamp 0.3.2 or newer](https://github.com/logos-co/logos-basecamp/releases/latest).
- **macOS:** Apple silicon. Open the `aarch64.dmg`, drag Basecamp to
  Applications, and right-click → **Open** the first time if macOS blocks it.
  Basecamp does not yet ask for the Local Network permission
  ([logos-co/logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527)),
  so LAN discovery may fail; then set **Dial address** to `<operator-ip>:7777`.
- **Linux:** x86_64 or arm64 with **glibc 2.39+** (Ubuntu 24.04 or newer; Debian 12
  and Ubuntu 22.04 are too old). Make the AppImage executable and open it. If
  `libOpenGL.so.0` is missing, install `libopengl0`.
- All devices on the same Wi-Fi or LAN without client isolation, with TCP 7777 and
  UDP 7778 allowed. The LAN does not need internet.

## Install Basecamp and the packages

### Install by name (recommended)

Open **Settings → Package Repositories**, paste this URL, click **Add**, then
**Refresh**:

```text copy Copy Catalogue URL
https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
```

Open **Manage Packages**, search `zcash`, and install **Offline Zcash service**
and the apps from the table above. No GitHub account or token is needed. This is
our own catalogue, not the official Logos one. Packages are unsigned: accept the
warning only if you trust this experimental build. For `logosctl` (headless,
separate from Basecamp's data), see [logos/README.md](../logos/README.md#install-by-name).

### Manual LGX fallback

From the [latest release](https://github.com/megabyte0x/offline-zcash/releases/latest),
download the `.lgx` files for `darwin-arm64`, `linux-amd64` or `linux-arm64`,
or the merged `<name>-<version>.lgx` files, which contain all three:

1. `zcash_offline_module-<version>-<platform>.lgx` — **Offline Zcash service**, always needed.
2. The apps you chose: `zcash_offline_ui`, `zcash_offline_organiser`,
   `zcash_offline_gateway`, with the same version and platform.

In Basecamp, open **Manage Packages → Install Local Package**. Install the service
first, then each app. The apps appear in the sidebar; hover for their names.
To build the packages yourself, see [logos/README.md](../logos/README.md#build).

Upgrade all devices in a session together. Peers older than v0.7.2 can only
live-forward to newer ones, not store-and-forward.

## Where the data lives

Basecamp gives the Offline Zcash service one data folder per Basecamp user
directory. Its **persistence path** is:

| Platform | Persistence path |
|---|---|
| Linux | `~/.local/share/Logos/LogosBasecamp/module_data/zcash_offline_module/<instance-id>` |
| macOS | `~/Library/Application Support/Logos/LogosBasecamp/module_data/zcash_offline_module/<instance-id>` |

`<instance-id>` is a 12-character hex folder that Basecamp creates once and then
reuses. On Linux, `~/.local/share` is `$XDG_DATA_HOME` if you set it. If you start
Basecamp with `--user-dir <dir>`, the path is
`<dir>/module_data/zcash_offline_module/<instance-id>` instead.

Inside it:

| Folder | Contents |
|---|---|
| `ozm` | This device: keys (`secret.json`), fleet, wallet, payment history, gateway pin |
| `sessions/session-YYYYMMDD-HHMMSS` | Organiser exports: `fleet.json`, invites and secrets |
| `ozm-backup-YYYYMMDD-HHMMSS` | A device folder moved aside by [Leave session](#leave-session) (UTC time) |

You do not need to find these by hand: the Organiser shows the export folder before
you create a session, and Leave session shows the backup folder it created. Back up
the persistence path before upgrading or reinstalling Basecamp; never delete `ozm`
or a backup that held funds.

## Join a session

One Basecamp user directory holds one device, on one network (Regtest or testnet).
To switch network or invite, use [Leave session](#leave-session).

### Without an invite (default)

1. Open **Offline Zcash** and leave **I have an organiser invite** closed. The device creates and
   stores its own keys and address; nobody else receives them.
2. New devices use **public testnet**; there is no network to choose. Only for
   development with a local `ths` chain, open **Advanced (local Regtest for
   developers)** and turn on **Use a local Regtest chain instead of testnet**.
   No block height or scan settings are needed. Fresh keys have no history to restore.
3. Leave listen/dial empty for LAN discovery. If no neighbours appear, set **Dial
   address** to `<operator-ip>:7777`. Leave experimental Bluetooth off unless you
   are on [Linux with BLE](#bluetooth-on-linux).
4. Click **Start without invite**. Your receive address appears straight away;
   share it even while offline. The wallet syncs automatically when it opens,
   finds a gateway, or hears a new chain tip. The header says **ready** when caught
   up; the chain view shows the gateway's name, mesh ID, hops and tip.
   **Refresh balance (optional)** is only a manual retry, never a setup step.

### With an invite

Before the first start, open **I have an organiser invite**, paste your own invite
and click **Join with invite and start**.
An invited device trusts its fleet's named gateway instead of pinning the first
one it hears. An installed identity is never replaced by pasting another invite;
use [Leave session](#leave-session) first.

## Pay and receive

1. On Regtest, click **Get test funds (5 × 0.1 ZEC)** and wait for the balance.
   On testnet, see [Wallets on testnet](#wallets-on-testnet).
2. The payee copies **Your address** and sends it to the payer outside the app.
3. The payer pastes it under **Pay offline**, enters an amount or clicks **Max**,
   and clicks **Build, prove and send over the mesh**.
4. Expand **history** to follow each state. Invite-free and unknown payers are
   never accepted pending: the payee waits for the gateway to report inclusion.
   Fast LAN payments may jump straight to `mined`.

`submitted` is not inclusion. `included` is 1 confirmation and `mined` is 10.
**Both are reported by the gateway, not proved**, and a reorg can downgrade them.
Received coins are spendable after `mined` and a sync.

**Max** fills the largest amount the builder can send after its ZIP 317 fee. It
skips locked notes, unmined change and dust that would cost more than it adds.
It does not send anything itself.

Any unified address for the session's network with an Ironwood receiver works,
including one not on the mesh. That payment still goes to the gateway and shows
**no payee receipt**. Addresses for another network and transparent-only
addresses are refused.

## Run a gateway

The gateway is the device with chain access. It needs an invite that makes it the
gateway; the Gateway app and the service refuse to start one otherwise. Docker and
the `ths` chain (Regtest) stay outside Basecamp.

### Create the session in Organiser

1. Open **Offline Zcash Organiser**. New sessions are **public testnet**. Only for
   development with a local `ths` chain, open **Advanced (local Regtest for
   developers)** and turn on **Create a local Regtest session instead of testnet**.
   Wallet scan settings are automatic, including for invites created offline.
2. Add a device for each invite you need and tick **This device is the gateway**
   for at least one of them, for example `Operator`. Wallet devices can join
   without invites, so you may only need the gateway entry.
3. Keep the suggested session folder, `<persistence path>/sessions/session-YYYYMMDD-HHMMSS`
   ([paths](#where-the-data-lives)), or choose another. Existing fleet or key files
   are never overwritten.
4. To use this device as the gateway, set **Join this device as** to `Operator`.
   Leave it empty to export only.
5. Click **Create session and invites**. It writes `fleet.json` and each device's
   `.invite.json` and `.secret.json`. **Open session folder** opens the export; if
   no file manager opens, paste the shown path into Finder → Go → Go to Folder.

A separate gateway laptop pastes its gateway invite in the Gateway app and clicks
**Join device**. A device whose invite is not the gateway sees Start disabled.

### Gateway on Regtest

1. Install [ths v0.4.1](https://github.com/zcashlabs/thus-spoke-zakura/releases/tag/v0.4.1)
   and Docker, run `ths start`, and keep it running.
2. In **Offline Zcash Gateway**, set **Chain RPC URL** to the plain `http://`
   Zakura JSON-RPC address from `ths endpoints`, usually `http://127.0.0.1:18232`.
   `https://` URLs such as testnet.zec.rocks are refused, because the device is on
   a Regtest session; to use testnet, [leave the session](#leave-session) and join
   a testnet invite. lightwalletd (`:9067`) is refused because it is not JSON-RPC.
3. Enable **Mine blocks (Regtest)**: ths does not mine submitted transactions on
   its own.
4. Click **Check chain service**. It must report chain `test` and a height.
5. Click **Start gateway**. If the device already runs, this enables the gateway
   on the same service. **Apply gateway settings** changes RPC and mining without
   restarting the mesh. Settings are restored after a restart.
6. Copy the displayed `ths faucet <address> --amount 2` command, run it once in a
   terminal, and wait for the balance to update automatically. This wallet serves
   test funds; refill it when it runs out. **Refresh gateway balance** is optional.

### Gateway on public testnet

The testnet gateway is a light client of TLS lightwalletd servers; no full node,
VPS or `ths` is needed. Wallet devices do not need internet.

1. Create a `testnet` session in Organiser with the internet device as the gateway,
   and join it on that device.
2. **Offline Zcash Gateway** hides the RPC field, the mining toggle and the faucet
   command on a testnet session. It shows the fleet's lightwalletd servers from the
   fleet file, by default `https://testnet.zec.rocks:443` and
   `https://zaino.testnet.unsafe.zec.rocks:443`.
3. Click **Check chain service**, then **Start gateway**. With only one server
   reachable the app shows **single source**; that is a warning, not a failure.
4. The gateway wallet syncs automatically in the background; the badge shows
   `syncing · height X of Y`. Leave stops the sync. There is no scan-height setup.

There is no gateway faucet on testnet.

### Wallets on testnet

1. Start without an invite on `testnet` (or join a testnet invite), then copy the
   `utest1` address to a public TAZ faucet such as
   [Valar](https://faucet.testnet.valargroup.dev). Faucet claims have failed in our
   tests ([details](components/offline-builder.md#v070-testnet-note-plan)).
2. Funds appear automatically. While a gateway is reachable the wallet prepares
   smaller notes itself; **Offline readiness** shows progress. No Prepare click
   or refresh is needed. Small balances avoid uneconomic split fees.
3. Wait for `mined` (ten gateway-reported confirmations on testnet) and the
   automatic scan before disconnecting. A single-note wallet temporarily locks
   that note while the split mines; other spendable notes remain usable. Unmined
   change is never spendable. **Advanced manual split** is optional.

The gateway retries the same transaction every three blocks until expiry. A
refused payment may still be mined until it expires.

## Leave session

**Leave session**, in all three apps, takes the device out of its current session.
After you confirm, it:

1. stops the node, cancelling a running wallet sync and letting a payment that is
   being proved finish first;
2. moves the whole `<persistence path>/ozm` folder to
   `<persistence path>/ozm-backup-YYYYMMDD-HHMMSS` (UTC time), shown on screen.
   Nothing is deleted: keys and funds stay in the backup;
3. moves any older Organiser exports from `ozm/sessions` to
   `<persistence path>/sessions`, so exported invites stay reachable;
4. returns the device to the not-joined state.

The device can then start without an invite or join a new invite, for example to
switch from Regtest to testnet.

**Your own wallet comes back.** Since v0.7.4, when the device next starts
**without an invite on the same network**, it restores the keys, address, wallet
(notes and sync height) and payment history from the newest backup that holds an
invite-free wallet on that network. The balance shows again after the next sync.
The session stays behind: the pinned gateway, the mesh store, events and start
options are not restored, so the device picks a gateway again. Regtest and testnet
keep separate wallets. An invited session uses the organiser's keys from its
invite; that wallet stays in its own backup and does not replace your invite-free
wallet, which returns after you leave the invited session. A Regtest wallet only
matches the chain it was synced on; after `ths` creates a new chain, leave and
move its backup aside to start with a fresh wallet.

To restore an invited wallet, quit Basecamp, move or rename any new `ozm` folder
out of the way, and rename that backup folder back to `ozm`
([paths](#where-the-data-lives)). Leave exists only in these Basecamp apps, not in the `ozm-node` CLI;
for the CLI, use a different `--home`.

## Open-mode trust and faucet limits

- A device without an invite picks a gateway by fewest hops, then highest
  advertised tip, and **pins that gateway's key at first use**. A different
  gateway cannot silently replace it; if the pinned one disappears the app warns
  and does not switch. Invited devices trust their fleet's named gateway instead.
- First contact is trust-on-first-use. A fake gateway can win it: check the
  gateway's mesh ID with the operator outside the app. To change operator on
  purpose, use [Leave session](#leave-session).
- **Sync, inclusion and mined results are gateway-reported**, not chain proofs.
- Address bindings prove the address holder can receive, not spend, and pin the
  address to a mesh key at first receipt routing. The typed address always decides
  where the money goes.
- Invite-free and unknown payers never get pending acceptance.
- The Regtest faucet allows one request per device per 60 seconds, at most 10
  notes and the gateway's total cap (default 1 ZEC). New identities can get
  around it; it is not Sybil protection.

Use a trusted local operator and test coins only. Full details:
[privacy-and-threats.md](privacy-and-threats.md#open-mode-controls-v060).

## Problems

| Symptom | Check |
|---|---|
| AppImage does not start (`GLIBC_* not found`) | Use glibc 2.39+, for example Ubuntu 24.04 |
| Package refused | Match platform and version; released Basecamp needs the portable packages, not `-dev` |
| Missing app or module not reachable | Manage Packages must list the service and the app; reopen the app |
| Title still shows the old version (for example "Offline Zcash v0.7.2") after installing a new one | The service is still the old one. See [Upgrade did not take effect](#upgrade-did-not-take-effect) |
| "The Offline Zcash service did not answer" | The text after it is Basecamp's detail. "not connected/not reachable": install or load the service. Otherwise the service is busy; the page retries every 3 seconds |
| No mesh neighbours | Same Wi-Fi without guest/client isolation; allow TCP 7777 and UDP 7778; set **Dial address** to `<operator-ip>:7777` |
| macOS discovery fails | Basecamp's Local Network permission support is pending ([logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527)); use **Dial address** = `<operator-ip>:7777` |
| Gateway Start disabled | This device's invite is not the gateway, or it joined without an invite; use the gateway invite |
| RPC URL refused on Regtest | Use the `http://` JSON-RPC URL from `ths endpoints`, not `https://` or lightwalletd `:9067`; for testnet, Leave session and join a testnet invite |
| RPC check fails | Keep `ths start` running on the gateway machine |
| Wallet never synced | Check the Gateway app's chain status; discovery starts sync automatically. **Refresh balance (optional)** retries it |
| Test funds fail | Regtest only: enable mining, fund the gateway address, wait 60 seconds between requests |
| Wrong network or wrong invite installed | [Leave session](#leave-session), then start again |
| Header says "Regtest session (local ths chain)" but you wanted testnet | Before v0.7.4 the Organiser and the wallet started on Regtest unless you changed the network, so a session or invite made then (for example the default "Three laptops") is Regtest. [Leave session](#leave-session) on every device, create a new session in Organiser v0.7.4 (testnet by default) and join with its new invites |
| Need to find keys, wallet or exported invites | See [Where the data lives](#where-the-data-lives) |
| Several Basecamp instances on one Linux host | Use separate user directories and different mesh listen ports |

### Upgrade did not take effect

The wallet title shows the version of the **running service**
(`Offline Zcash v<version>`). Basecamp 0.3.2 replaces installed packages in
place, stops the old service before the swap and starts the new one when an
app is opened again. Older Basecamp releases could keep the old service
running after an install ([logos-basecamp#423](https://github.com/logos-co/logos-basecamp/issues/423),
fixed in 0.3.1). Your wallet is in `module_data` and is not touched by any of
these steps.

1. Check Basecamp's version, shown at the bottom of its left sidebar (for example
   `0.3.2 · Portable`), and update to
   [0.3.2 or newer](https://github.com/logos-co/logos-basecamp/releases/latest) if it is older.
2. Check which versions are on disk. On macOS:

   ```sh copy Copy Version Check
   cd ~/Library/Application\ Support/Logos/LogosBasecamp
   grep -H '"version"' modules/zcash_offline_module/manifest.json plugins/zcash_offline_*/manifest.json
   ```

   On Linux, use `~/.local/share/Logos/LogosBasecamp` instead.
3. If they show the new version, quit Basecamp completely (**Cmd+Q** on macOS,
   not just closing the window), check that no old service is left
   (`pgrep -fl logos_host`; quit it if one is), and open Basecamp again. Open
   **Offline Zcash** to start the service; the title shows the new version.
4. If they show the old version, open **Manage Packages**, click **Reload**, and
   click **Upgrade** for **Offline Zcash service** and then each app, accepting
   the unsigned-package prompt each time. Or use **Install Local Package** with
   the new `darwin-arm64` (or Linux) `.lgx` files, service first. Then repeat step 3.
5. If an upgrade still keeps the old version, uninstall the three apps and then
   **Offline Zcash service** in Manage Packages, quit and reopen Basecamp, and
   install the new service and apps. Uninstalling packages does not delete
   `module_data`; never delete `module_data/zcash_offline_module` or its `ozm`
   and `ozm-backup-*` folders.

Upgrade every device in a session to the same version.

### Bluetooth on Linux

The **Bluetooth LE (Linux only, experimental)** toggle in Offline Zcash must be set
before the first start; changing it later requires restarting Basecamp. You need
`bluetoothd`, system D-Bus and a powered LE adapter that can scan and advertise at
once, and D-Bus/polkit must allow BlueZ GATT, advertising and discovery. Do not run
Basecamp as root, and do not run the CLI and Basecamp on the same adapter. If
Bluetooth is unavailable, Start fails; turn it off to use LAN. macOS Bluetooth is
not implemented. **No real-radio test has been done yet**
([BLE milestones](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open)).
