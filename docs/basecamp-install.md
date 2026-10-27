# Offline Zcash in Logos Basecamp

Three apps share **one Offline Zcash service per device**. Install the apps for
the jobs you do; installing several does not create extra identities, wallets,
mesh nodes or listening ports. This is a local NU7 Regtest build with **test
coins only**, not real ZEC.

| Your job | Install these packages |
|---|---|
| Join, relay, hold a wallet, pay and receive | Service + **Offline Zcash** (`zcash_offline_ui`) |
| Create sessions and distribute invites | Service + **Offline Zcash Organiser** (`zcash_offline_organiser`) |
| Connect the mesh to the chain | Service + **Offline Zcash Gateway** (`zcash_offline_gateway`) |
| Organise, run the gateway and use a wallet | Service + all three apps |

An app exposes controls; it does **not** grant a fleet role. The gateway requires
an invite containing `gateway`. Roles can be combined (`payer+payee+gateway`).
The existing invite/fleet format is trusted setup from the organiser, **not an
organiser-signed enrollment format**. Do not edit or accept invites from strangers.
Each invite contains private test keys: give it only to its named owner.

The owner ran v0.5.1 successfully on three real Macs using this guide. That does
not verify v0.6.0 or Bluetooth on macOS. Local Network permission
support is pending upstream in
[logos-co/logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527).
If discovery fails, use **Dial address** = `<operator-ip>:7777`. All three Macs
must use the same non-isolating Wi-Fi; it does not need internet.

## Install Basecamp and the packages

Use [Basecamp 0.3.2 or newer](https://github.com/logos-co/logos-basecamp/releases/latest):

- Apple silicon Mac: open the `aarch64.dmg`, drag Basecamp to Applications,
  and right-click → **Open** if macOS blocks it.
- Linux x86_64 or arm64: make the AppImage executable and open it. It needs
  **glibc 2.39+** (Ubuntu 24.04+, not Debian 12/Ubuntu 22.04). If
  `libOpenGL.so.0` is missing, install `libopengl0`.

### Install by name (recommended)

In Basecamp 0.3.2, open **Settings → Package Repositories**, paste this URL and
click **Add**, then **Refresh**:

```text copy Copy Catalogue URL
https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
```

Open **Manage Packages**, search `zcash`, and install **Offline Zcash service**
and the apps from the table above. Basecamp resolves the shared dependency.
Accept the unsigned-package warning only if you trust this experimental build.
The public catalogue and downloads need no GitHub account or token. This is
our catalogue, not the official Logos catalogue.

For the headless runtime, install
[released logosctl 0.3.2](https://docs.logos.co/core/build-modules/build-and-run-a-logos-core-module)
(Linux needs glibc 2.39+, for example Ubuntu 24.04), then:

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

`logosctl` installs into its own session, **not Basecamp's data directory**.
Use Basecamp's Package Repositories screen for Basecamp installs. The catalogue
selects the newest release; add `--version 0.6.0` to pin a CLI install.
Its identity card points to `releases/latest/download/index.json`; every release
also attaches its own index, merged packages and `catalogue.sha256`. Content
roots bind the index to the downloaded packages, but do not authenticate the
publisher. Packages are unsigned (`trustedSigners` is empty).

### Manual LGX fallback

From [Offline Zcash v0.6.0](https://github.com/megabyte0x/offline-zcash/releases/tag/v0.6.0),
download the `.lgx` files for `darwin-arm64`, `linux-amd64` or `linux-arm64`
(or the merged `<name>-0.6.0.lgx` files, which contain all three):

1. `zcash_offline_module-0.6.0-<platform>.lgx` — **Offline Zcash service**, always needed.
2. The apps from the table above: `zcash_offline_ui`,
   `zcash_offline_organiser`, `zcash_offline_gateway`, with the same version/platform suffix.

In Basecamp, open **Manage Packages → Install Local Package**. Install the service
first, then each chosen app. Accept the unsigned-package warning for these test
packages. The apps appear in the sidebar; hover for their names. All use the
same service dependency, which Basecamp starts automatically.

To build instead (Nix with flakes, about 10 GB free; first build can take longer):

```sh copy Copy Build Commands
git clone https://github.com/megabyte0x/offline-zcash
cd offline-zcash
for m in zcash_offline_module zcash_offline_ui zcash_offline_organiser zcash_offline_gateway; do
  (cd "logos/$m" && nix build .#lgx-portable --accept-flake-config --cores 1 -o "../../out-$m")
done
ls out-*/*.lgx
```

Use portable packages with released Basecamp; `.#lgx` is for dev builds only.
We retain the already-hashed portable builder outputs rather than switching
bundlers. Released catalogue packages do not include `-dev` variants; build
`.#lgx` on your development machine when using a Nix/dev Basecamp runtime.

## A three-Mac session, without an ozm-node terminal

The operator installs all three apps; Alice and Bob install only **Offline
Zcash**, plus the service. The apps handle fleet creation, gateway startup and
payments. Docker and the `ths` chain stay outside Basecamp.

### 1. Operator: start the local chain

Install [ths v0.4.1](https://github.com/zcashlabs/thus-spoke-zakura/releases/tag/v0.4.1)
and Docker. In a terminal run:

```sh copy Copy Chain Command
ths start
```

Keep it running. You do not run an `ozm-node` organiser or gateway process.

### 2. Operator: create the fleet in Organiser

1. Open **Offline Zcash Organiser**.
2. Enter a session name and at least `Operator` (`gateway`). The default Alice
   and Bob entries can remain for optional invited devices; open devices do not need them.
3. Choose a new session folder, or keep the suggested one. Existing fleet/key
   files are never overwritten.
4. Set **Join this device as** to `Operator` (leave it empty to export only).
5. Click **Create fleet and invites**. It writes `fleet.json` and each device's
   `.invite.json` and `.secret.json`, shows the folder and invite paths, and joins
   the operator without starting a second engine.
6. Keep the exported keys private. Only distribute named invites if you want
   invited devices instead of the open join below. **Open session folder** opens
   the export; if no file manager opens, paste its path into Finder → Go → Go to Folder.

An organiser can export later sessions without replacing its installed identity.
One Basecamp user directory holds one device; replacing it requires a separate
Basecamp user directory, not pasting another person's invite over a wallet.

### 3. Operator: start the gateway in Gateway

1. Open **Offline Zcash Gateway**. It already sees `Operator` from Organiser.
   A separate gateway laptop can instead paste its gateway invite here and **Join device**.
2. Leave **Chain RPC URL** at `http://127.0.0.1:18232` and enable
   **Mine blocks (Regtest)**.
3. Click **Check RPC**: it must report `getblockchaininfo`, chain `test` and a height.
4. Click **Start gateway**. The same device starts its mesh, syncs its wallet and
   serves sync/funding requests. If Offline Zcash already started the device,
   this enables the gateway on that existing engine. **Apply gateway settings**
   updates RPC/mining without replacing the mesh. Saved settings resume after restart.
5. To seed the test-fund wallet, copy the displayed `ths faucet <address> --amount 2`
   command and run it once in the operator's `ths` terminal. Click **Refresh gateway balance**.
   This is chain funding, not a separate gateway program. Refill if it runs out.

Installing Gateway with an Alice/Bob invite shows **This invite does not grant
the gateway role** and disables Start. Neither installing the app nor setting an
RPC URL changes the invite's role.

### 4. Alice and Bob: start without invites in Offline Zcash

1. Leave **Invite JSON** empty. Each device generates and stores its own private
   keys and Regtest unified address; the organiser never receives those keys.
2. Leave listen/dial empty for normal LAN discovery. If it fails, set **Dial
   address** to `<operator-ip>:7777` (for example `192.168.1.20:7777`).
3. Leave experimental Bluetooth off. Click **Start without invite**.
4. Wait for the selected gateway and synced wallet. Wallet shows its name, mesh
   ID, hop count and advertised tip. Click **Sync now** if needed.

Optional: before first start, paste your own organiser invite and click **Join
and start** instead. Invited devices retain their configured gateway authority;
an open device accepts a self-signed gateway on the reachable mesh. One installed
identity cannot be replaced by another invite: preserve the wallet and keys.

Do not install organiser/gateway apps on these two Macs unless they need those
controls. Gateway authority still requires a gateway invite.

### 5. Alice: get test coins and pay Bob

1. Click **Get test funds (5 × 0.1 ZEC)**; wait for a 0.5 ZEC balance.
2. Bob copies **Your address** and shares it with Alice outside the app. Under
   **Pay offline**, Alice pastes Bob's Regtest unified address, enters `0.01`, and clicks
   **Build, prove and send over the mesh**.
3. Open devices wait for gateway-reported inclusion: `sent` → `awaiting-inclusion` → `mined`.
   Pending acceptance remains available only between devices in the same invited fleet.
   Fast LAN payments may jump straight to `mined`; expand **history** for each step.
4. The operator's Gateway app shows the txid, submission count and mined result.

Bob can spend the received coins after `mined` and a balance refresh. `mined`
means **gateway-reported**, not an independently verified chain proof. The wider
chain-proof protocol remains unbuilt.

**Max** fills the largest sendable amount using the same padded Ironwood action
count and ZIP 317 fee rule as the builder (and the configured fee floor). It
excludes locked notes and unmined change. Uneconomic dust stays behind if adding
it would reduce the sendable amount. The quote can change after another payment
or sync; Max does not send anything itself.

Any valid Regtest unified address with an Ironwood receiver works, including an
address not on the mesh. Such a payment goes to the gateway and displays
**submitted, no payee receipt**, then **mined · no payee receipt**. No device
directory or recipient list is needed. Other networks and transparent-only
addresses are rejected rather than silently using another receiver.

### Open-mode trust and faucet limits

All inner envelopes (including sync, faucet and submission requests/replies) are
signed by their sender and bound to the intended recipient. Mesh delivery acks
are signed by the original destination. Sealing alone is not authentication.
Gateway announcements and address bindings are individually sealed; only mesh
IDs and public routing keys are broadcast. A fresh announcement lasts 35 seconds.
Before first use, devices choose fewest hops, then highest advertised tip, then
mesh ID. **First use pins the gateway's key** in `data/trust.json`, across restarts.
A nearer/newer gateway cannot silently replace it. An unavailable pin produces a
warning and blocks automatic switching. To deliberately change operators, stop
the device, verify the new operator's mesh ID outside the app, remove only
`data/trust.json`, and restart. Never delete `secret.json` or wallet data.

**First contact is still trust-on-first-use, not operator authentication.** A Sybil
gateway can win the initial choice. Hop counts/tips are not independently proved;
**sync, inclusion and mined results are gateway-reported**. Use a trusted local
operator and test coins only. Gateway signatures do not prove chain facts.

Address bindings include a domain-separated Schnorr proof of knowledge of the
Ironwood receiver's incoming-viewing scalar, bound to the exact address and mesh
identity. No viewing/spending key is shared. This proves **receiving authority,
not spending authority**; a holder of a copied incoming viewing key can also prove
it. First receipt routing pins address → mesh key; changed/ambiguous bindings
disable receipt routing, never redirect funds. The typed address always determines
the transaction's destination. This experimental proof implementation has not
had an independent cryptographic review.

Open-mode and unknown payers **never receive pending acceptance**. A payee waits
for its pinned gateway to report inclusion before issuing an `Included` receipt.
That receipt is not an independently verified inclusion or finality proof.

The loopback web API requires a per-run random token and a loopback Host header;
the UI and CLI obtain the token from the local page. Remote strings are rendered
as text. Keep the API bound to `127.0.0.1`; `--api 0.0.0.0` is for isolated tests,
not a secure network API. There is no protection from programs running as your user.

The Regtest faucet allows one request per device per 60 seconds (persisted over
gateway restarts), at most 10 notes and the configured total cap (default 1 ZEC).
New identities can evade the limit: it is not Sybil protection. Keep the gateway
wallet funded. v0.5.x nodes ignore unsupported new messages without crashing;
upgrade all payment/sync endpoints to v0.6.0: unsigned legacy envelopes/acks are
no longer accepted. Invite creation and joining remain supported in v0.6.0.

## Problems

| Symptom | Check |
|---|---|
| AppImage does not start (`GLIBC_* not found`) | Use glibc 2.39+, e.g. Ubuntu 24.04 |
| Package refused | Match platform/version and use portable, not `-dev`, with released Basecamp |
| Missing app icon / module not reachable | Manage Packages must list the service plus the chosen app; reopen the app |
| RPC check fails | Keep `ths start` running on the gateway machine; check the RPC URL |
| No mesh neighbours | Same Wi-Fi, no guest-network isolation; allow TCP 7777 and UDP 7778; dial `<operator-ip>:7777` |
| macOS discovery fails | Local Network support is pending upstream (#527); use the dial fallback above, without assuming a permission prompt exists |
| Wallet has never synced | Wait for the gateway, then Sync now; check Gateway's RPC result |
| Test funds fail | Enable Regtest mining, fund the gateway address, and wait 60 seconds between requests from one device |
| Payment refused | Read its history; pending is not final and testnet/mainnet are not supported here |
| Multiple Basecamp instances on one Linux host | Use separate user directories and unique mesh listen ports; ordinary laptops all use 7777 |

### Bluetooth on Linux

**Offline Zcash** Setup has a **Bluetooth LE (Linux only, experimental)** toggle.
Enable it before first start; changing transport on an already running engine
requires restarting Basecamp. RPC/mining settings are the exception and update live.

You need `bluetoothd`, system D-Bus and a powered LE adapter with simultaneous
scanning/advertising. The user's D-Bus/polkit policy and Basecamp sandbox must
permit BlueZ GATT, advertising and discovery. Do not run Basecamp as root.
libdbus is bundled; BlueZ is a host requirement. Do not run the CLI and Basecamp
on the same adapter at once. Unavailable Bluetooth makes Start fail; disable it
to use LAN. Neighbours show BLE/TCP and Mesh shows Bluetooth status.

**No real-radio testing yet (D1/D2/D5 open).** macOS BLE is not implemented and
also needs upstream Bluetooth usage-description support in #527. See
[desktop BLE milestones](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).
