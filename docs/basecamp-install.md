# Offline Zcash in Logos Basecamp

**v0.7.7 · macOS and Linux · test coins only.** Wallets pay, receive and relay
over a local mesh. On public testnet, only the gateway needs internet.

Install **Offline Zcash service** (`zcash_offline_module`) and the apps you need.
All three apps share one wallet and mesh node per Basecamp user directory.

| Task | App |
|---|---|
| Pay and receive | **Offline Zcash** (`zcash_offline_ui`) |
| Create a gateway session | **Offline Zcash Organiser** (`zcash_offline_organiser`) |
| Connect the mesh to the chain | **Offline Zcash Gateway** (`zcash_offline_gateway`) |

## Requirements

- [Basecamp 0.3.2 or newer](https://github.com/logos-co/logos-basecamp/releases/latest).
- **macOS:** Apple silicon. Open the `aarch64.dmg`, drag Basecamp to
  Applications, and right-click → **Open** if macOS blocks the first launch.
- **Linux:** x86_64 or arm64 with **glibc 2.39+** (Ubuntu 24.04 or newer; Debian 12
  and Ubuntu 22.04 are too old). Make the AppImage executable and open it. If it
  reports a missing `libOpenGL.so.0`, install `libopengl0`.
- Same Wi-Fi or LAN without client isolation; allow TCP 7777 and UDP 7778.

## Install the packages

### Install by name (recommended)

In Basecamp, open **Settings → Package Repositories**, add this URL, then
click **Refresh**:

```text copy Copy Catalogue URL
https://offlinezcash.com/basecamp
```

This redirects to [`logos-repo.json`](https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json);
Basecamp follows the redirect, or you can paste the full URL instead.

Open **Manage Packages**, search `zcash`, and install the service first, then
your apps. No GitHub account is needed. This is a custom catalogue with unsigned
packages: accept the warning only if you trust this experimental build.

### Manual LGX fallback

Download the service and app `.lgx` files from the
[latest release](https://github.com/megabyte0x/offline-zcash/releases/latest).
Choose `darwin-arm64`, `linux-amd64` or `linux-arm64`, or the merged packages
containing all three. Keep versions and platforms consistent; do not use `-dev`
packages with released Basecamp.

Use **Manage Packages → Install Local Package**, service first. For builds and
headless `logosctl` setup, see [logos/README.md](../logos/README.md).

Upgrade all devices together. Peers older than v0.7.2 cannot store-and-forward
payments to newer peers.

## Join a session

### Without an invite (default)

1. Open **Offline Zcash**. Leave invite, listen and dial fields empty, and
   Bluetooth off.
2. Click **Start without invite**. The wallet creates local keys and a receive
   address on **public testnet**. No scan settings are needed.
3. Wait for **ready**. Sync runs automatically when the wallet opens, finds a
   gateway or hears a new chain tip.

If discovery fails, set **Dial address** to `<operator-ip>:7777`. For a local
development chain, enable **Use a local Regtest chain instead of testnet** under
**Advanced** before starting.

### With an invite

Open **I have an organiser invite**, paste your invite, then click **Join with
invite and start**. Invites contain private keys: use only your own, from a
trusted organiser. The format is not organiser-signed.

One Basecamp user directory holds one device on one network. To change invite or
network, use [Leave session](#leave-session) first.

## Pay and receive

1. [Fund the wallet](#wallets-on-testnet) and let it sync and prepare funds.
   On Regtest, use **Debug → Get test funds (5 × 0.1 ZEC)** instead.
2. The payee copies **Your address** and shares it with the payer outside the app.
3. The payer pastes it under **Send**, enters an amount or clicks **Max**, then
   clicks **Send**. Max fills the spendable amount after fees; it does not send.
4. Follow the payment in **Payments**. Transaction IDs and events are under
   **Debug → history**.

Use a unified address for the same network with an Ironwood receiver. Addresses
outside the mesh work but show **no payee receipt**.

| State | Meaning |
|---|---|
| `submitted` | Forwarded for submission; inclusion not confirmed |
| `included` | Gateway reports 1–9 confirmations |
| `mined` | Gateway reports 10 confirmations |
| **Scanned · gateway-reported** | Found by scanning, without a mesh payment record or payee receipt |

**Confirmation reports are not chain proofs.** Tracking stops at `mined`, so
that saved result does not monitor later reorgs. Received funds and change need
a completed local scan before spending. Wait for `mined` and sync before going
offline.

Invite-free and unknown payers never get pending acceptance. A catching-up payee
keeps the payment for verification, including across restart; that is neither
acceptance nor a refund. If a signed payment is waiting for submission, let it
retry—do not pay again. A refused payment may still be mined until it expires.

### Online mode (testnet wallets with internet)

A testnet wallet with internet can turn on **Online mode** in the Wallet card
(module `start` option `direct`, `ozm-node run --direct`, or `POST /api/direct`
`{"direct":true}`). It then syncs from the session's testnet lightwalletd servers
and submits its own and received payments itself, tracking them to included and
mined without a mesh gateway. The header shows **online · direct**. The setting
survives restarts but not Leave session.

- It does **not** make the wallet a gateway: it never announces a gateway role,
  serves chain data or accepts peers' submissions, and cannot be pinned.
- The servers see this computer's IP address and its transactions; their reports
  are trusted like a gateway's.
- Only mined, scanned notes are spendable.
- If no server answers, the wallet shows the error, falls back to the mesh
  gateway and retries the servers every minute.
- Testnet only; Regtest wallets and session gateways cannot turn it on.

## Run a gateway

The gateway needs chain access and a **gateway invite**. Installing the Gateway
app alone does not grant that role. Other wallets need no invite.

### Create the session in Organiser

1. Open **Offline Zcash Organiser**. It defaults to public testnet; Regtest is
   under **Advanced**.
2. Keep the suggested export folder or choose an empty one.
3. Keep **This device is the gateway (join now)** ticked to make this computer the
   gateway, or untick it to export the invite for another computer.
4. Click **Create gateway session**. **Open session folder** shows `fleet.json`,
   `Gateway.invite.json` and `Gateway.secret.json`. Keep these files private.

For a separate gateway computer, paste its invite in **Offline Zcash Gateway**
and click **Join device**.

### Gateway on public testnet

Open **Offline Zcash Gateway**, click **Check chain service**, then **Start
gateway**. It uses the session's TLS lightwalletd servers; no Docker, full node
or `ths` is needed. **single source** means only one server is reachable.
The gateway wallet syncs automatically. There is no built-in testnet faucet.

### Wallets on testnet

1. Copy your `utest1` address to a public TAZ faucet such as
   [Valar](https://faucet.testnet.valargroup.dev). Availability varies.
2. Keep a gateway reachable. The wallet discovers funds and automatically splits
   them into smaller notes when economical, for separate offline payments.
3. Wait for preparation to reach `mined` and sync to finish before disconnecting.
   A note being split is temporarily locked; other spendable notes remain usable.

See **Debug → Offline readiness** for details. Manual splitting and balance
refresh are optional, not setup steps.

### Gateway on Regtest

1. Install [ths v0.4.1](https://github.com/zcashlabs/thus-spoke-zakura/releases/tag/v0.4.1)
   and Docker. Keep `ths start` running outside Basecamp.
2. Create a Regtest gateway session as above.
3. Set **Chain RPC URL** to the `http://` Zakura JSON-RPC address from
   `ths endpoints`, usually `http://127.0.0.1:18232`. Do not use an HTTPS testnet
   server or lightwalletd port `9067`.
4. Enable **Mine blocks (Regtest)**, click **Check chain service** (expect chain
   `test` and a height), then **Start gateway**.
5. Run the displayed `ths faucet <address> --amount 2` command to fund the gateway
   wallet. Refill it when its test-funds balance runs out.

**Apply gateway settings** updates RPC/mining without restarting the mesh.
Settings persist across restarts.

## Where the data lives

Back up this **persistence path** before upgrading or reinstalling:

| Platform | Path |
|---|---|
| Linux | `~/.local/share/Logos/LogosBasecamp/module_data/zcash_offline_module/<instance-id>` |
| macOS | `~/Library/Application Support/Logos/LogosBasecamp/module_data/zcash_offline_module/<instance-id>` |

Basecamp creates and reuses the 12-character `<instance-id>` folder. On Linux,
`$XDG_DATA_HOME` replaces `~/.local/share` if set. With `--user-dir <dir>`, use
`<dir>/module_data/zcash_offline_module/<instance-id>`.

- `ozm`: keys, wallet, payment history and session state.
- `sessions/session-YYYYMMDD-HHMMSS`: Organiser exports.
- `ozm-backup-YYYYMMDD-HHMMSS`: wallets saved by Leave session (UTC timestamps).

**Keys are plaintext.** Owner-only folder permissions do not protect against
software running as you or copies in system/cloud backups. Keep exports and
backups private; never delete a wallet folder that held funds.

## Leave session

**Leave session** stops the device and moves `ozm` to a timestamped backup,
shown on screen. Nothing is deleted. It waits for an in-flight payment proof,
cancels sync and preserves older Organiser exports under `sessions`.

- **Return to your wallet:** start without an invite on the same network. The
  newest invite-free backup with notes or payment history is restored, including
  queued incoming payments. With no used backup, it creates new keys. Balance
  returns after sync.
- **Start fresh:** use **Debug → Start a new wallet…**. It keeps the backup but
  creates new keys. Reinstalling the module does not reset the wallet.
- **Restore an invited wallet:** quit Basecamp, move any current `ozm` aside, then
  rename its backup to `ozm`. Invited wallets are not restored automatically.

Automatic restore does not bring back the gateway pin, mesh store, events or
start options. Regtest and testnet wallets stay separate. Use a fresh Regtest
wallet after `ths` creates a new chain.

## Open-mode trust and faucet limits

- Invite-free wallets select by fewest hops, then advertised tip, and pin the
  gateway at first use. **Check its mesh ID with the operator outside the app**:
  a fake gateway can win first contact. A lost pinned gateway causes a warning,
  not an automatic switch. Leave session to change operators.
- Invited wallets trust the named gateway; the organiser also holds their keys.
- Sync and confirmation reports trust the gateway, not independent chain proofs.
  A wallet in [online mode](#online-mode-testnet-wallets-with-internet) trusts the
  testnet servers instead, and they see its IP address.
- Address bindings prove receiving, not spending, authority. The address you
  enter determines where the money goes.
- The Regtest faucet allows one request per device per 60 seconds, at most ten
  notes and a total cap (default 1 ZEC). New identities bypass these limits.

[Full threat model](privacy-and-threats.md#trust-assumptions-at-v070).

## Problems

| Symptom | Check |
|---|---|
| AppImage: `GLIBC_* not found` | Use glibc 2.39+, e.g. Ubuntu 24.04 |
| Package refused | Match version/platform; use portable packages, not `-dev` |
| Missing app or unreachable service | Install both service and app in Manage Packages, then reopen the app |
| Service did not answer | Check the detail: “not connected/not reachable” means install/load the service; busy calls retry automatically |
| Old version in wallet title | [Upgrade the service](#upgrade-did-not-take-effect), not just the app |
| No neighbours | Check Wi-Fi isolation and ports; set **Dial address** to `<operator-ip>:7777` |
| macOS discovery fails | [Basecamp Local Network permission is pending](https://github.com/logos-co/logos-basecamp/issues/527); use Dial address |
| Gateway Start disabled | Join the gateway invite, not an ordinary wallet invite |
| Regtest RPC check fails | Keep `ths start` running; use its `http://` JSON-RPC address, not lightwalletd `:9067` |
| Wallet not syncing | Check the gateway's chain status; retry with **Debug → Refresh balance (optional)** |
| Payments stuck or sends fail after a long session | Upgrade every service to v0.7.7; keep the original transaction for retry, do not pay again or delete wallet data |
| Regtest test funds fail | Enable mining, fund the gateway, wait 60 seconds between requests |
| Wrong network or invite | [Leave session](#leave-session) and start again; old sessions retain Regtest |
| Several instances on one Linux host | Use separate user directories and mesh listen ports |

### Upgrade did not take effect

The wallet title shows the **running service** version, not the app version.

1. Update Basecamp to 0.3.2 or newer.
2. In **Manage Packages**, click **Reload**, then upgrade **Offline Zcash
   service** first and each app after it. Alternatively, install the new local
   `.lgx` files in that order.
3. Quit Basecamp completely (**Cmd+Q** on macOS), reopen it and open Offline
   Zcash. Check the version in its title.
4. If it is still old, uninstall the apps and service, quit and reopen Basecamp,
   then reinstall the service and apps.

Uninstalling packages preserves `module_data`. **Do not delete wallet folders**;
upgrades migrate their data automatically.

### Bluetooth on Linux

Set **Bluetooth LE (Linux only, experimental)** before starting; later changes
require restarting Basecamp. It needs `bluetoothd`, system D-Bus, a powered
adapter that scans and advertises simultaneously, and D-Bus/polkit permission
for BlueZ GATT, advertising and discovery. Do not run Basecamp as root or share
the adapter with the CLI. If Start fails, turn Bluetooth off to use LAN.

macOS Bluetooth is not implemented. **Physical radios remain untested**;
see [BLE status](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).
