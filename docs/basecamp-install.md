# Install Offline Zcash in Logos Basecamp

This guide adds the **Offline Zcash** app to [Logos Basecamp](https://docs.logos.co/basecamp). You can then pay other people on the same Wi-Fi with no internet. A gateway laptop with internet puts the payments on chain.

It is a test build: it runs on a local test chain with test coins, never real ZEC.

You need:
- Basecamp **0.3.2 or newer**, on macOS with Apple silicon, or on Linux x86_64 or arm64 with **glibc 2.39 or newer** (for example Ubuntu 24.04 or Fedora 40 and later). The Basecamp AppImage does not start on Debian 12 or Ubuntu 22.04.
- An **invite file** (`<your-name>.invite.json`) from whoever runs the session. It contains your private test keys, so don't share it.
- The same Wi-Fi as the session's gateway. That Wi-Fi does not need internet.

## 1. Install Basecamp

Download Basecamp from [its releases page](https://github.com/logos-co/logos-basecamp/releases/latest):
- **Linux:** `LogosBasecamp-Desktop-*-x86_64.AppImage` (or `aarch64`). Run `chmod +x` on it, then open it. If it says `GLIBC_2.38' not found`, your Linux is too old (see above). If it says `libOpenGL.so.0` is missing, install it (`sudo apt install libopengl0` on Ubuntu).
- **macOS:** `LogosBasecamp-Desktop-*-aarch64.dmg`. Drag Basecamp to Applications and open it. If macOS blocks it, right-click Basecamp and choose **Open**.

Open Basecamp once (it shows "0.3.2 · Portable" or newer), then close it.

## 2. Get the two packages

You need two files for your platform (`linux-amd64`, `linux-arm64` or `darwin-arm64`):
- the **service** (`zcash_offline_module`), which runs in the background;
- the **app** (`zcash_offline_ui`), which is the window you use.

**Option A: download** `zcash_offline_module-<version>-<platform>.lgx` and `zcash_offline_ui-<version>-<platform>.lgx` from the [Offline Zcash releases](https://github.com/megabyte0x/offline-zcash/releases/latest).

**Option B: build them** if there is no release for your platform yet. This needs [Nix](https://nixos.org/download) with flakes and about 10 GB of free disk. The first build takes 10–30 minutes.

```sh
git clone https://github.com/megabyte0x/offline-zcash && cd offline-zcash
(cd logos/zcash_offline_module && nix build .#lgx-portable --accept-flake-config -o ../../module-lgx)
(cd logos/zcash_offline_ui && nix build .#lgx-portable --accept-flake-config -o ../../ui-lgx)
ls module-lgx/*.lgx ui-lgx/*.lgx
```

The service is `module-lgx/logos-zcash_offline_module-module-lib.lgx` and the app is `ui-lgx/logos-zcash_offline_ui-module.lgx`.

## 3. Install both packages

1. Open Basecamp and click **Manage Packages** (on the home screen, or the cube icon in the sidebar).
2. Click **Install Local Package** and pick the **service** file. In the file picker you can press Ctrl+L (Cmd+Shift+G on macOS) to type a path.
3. Basecamp warns that the package is not part of Logos and is unsigned. That is expected for this test build: click **Install**.
4. Do the same for the **app** file.
5. The Packages table now lists **Offline Zcash service** (type core) and **Offline Zcash** (type ui_qml), and a **Z** icon appears in the sidebar.

Install the service before the app, because the app needs it.

## 4. Join the session

1. Click the **Z** icon in the sidebar. Offline Zcash opens on the **Join a fleet** screen. Basecamp starts the service by itself.
2. Open your invite file in a text editor, copy all of it, and paste it into the box.
3. Leave the other fields empty, then click **Join and start**.

Within a few seconds the header shows your name, and **mesh neighbours** lists the other devices on the Wi-Fi. If it stays at "none", see [Problems](#problems).

## 5. Get test coins and pay

1. Click **Get test funds (5 × 0.1 ZEC)**. Within a minute your balance shows 0.5 ZEC.
2. Under **Pay offline**, choose a person in **To**, enter an amount (for example `0.01`), then click **Build, prove and send over the mesh**.
3. Follow the payment in **Payments**:
   - `sent` means it has left your device;
   - `accepted-pending` means the other person checked it and sent back a signed receipt;
   - `mined` means the gateway put it on chain.

   On a fast network it can jump straight to `mined`; click ▸ to see each step in its history.

Money you receive can be spent once it shows `mined` and your balance has updated.

## Problems

### Bluetooth on Linux

Development packages add a **Bluetooth LE (Linux only, experimental)** toggle in
Setup; leave it off for LAN only. Enable it before **Join and start** / **Start
node**, or call the service's `start` with `{"ble":true}`. The choice is saved for
Basecamp restarts. Start is idempotent; changing an already running node requires
a restart (the auto-started node otherwise keeps its saved choice).

You need `bluetoothd` running, system D-Bus, Bluetooth powered on and a default LE
adapter supporting simultaneous scanning and advertising. No pairing is needed.
Your desktop user's D-Bus/polkit policy and any Basecamp sandbox must permit
`org.bluez` access, GATT registration, advertising and discovery. Grant only the
needed permissions; do not run Basecamp as root. libdbus is statically bundled in
the portable module; BlueZ remains a host requirement. Avoid running the CLI and
Basecamp on the same adapter at once.

The header shows each neighbour's **BLE/TCP** kind and Mesh shows Bluetooth status.
Same-fleet Linux CLI and Android devices should connect; a LAN link may win
duplicate selection. Unavailable Bluetooth causes **Start failed** with a reason;
turn the toggle off to use LAN. BLE on macOS is not implemented yet and additionally
needs Basecamp's `NSBluetoothAlwaysUsageDescription` support ([upstream issue #527](https://github.com/logos-co/logos-basecamp/issues/527)).

**Untested on real hardware until D1 passes; Basecamp radio integration is D2.**
These controls are in the v0.4.1 and later release packages. See the [desktop BLE milestones](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).

### Troubleshooting

| What you see | What to do |
|---|---|
| Basecamp does not start on Linux (`GLIBC_2.38' not found`) | Your Linux is older than glibc 2.39. Use Ubuntu 24.04 or newer, or a Mac |
| Basecamp refuses the package | Check it is for your platform and is the portable file (not one ending `-dev`), and that Basecamp is 0.3.2 or newer |
| No **Z** icon in the sidebar | Open **Manage Packages** and check both packages are listed. Install whichever is missing |
| "module not reachable" | The service is not installed or has not started. Check **Manage Packages** lists Offline Zcash service, then reopen the app |
| No mesh neighbours | You must be on the same Wi-Fi as the others, and it must not isolate devices from each other (guest networks often do). On macOS, allow **Local Network** for Basecamp when asked, or under System Settings → Privacy & Security → Local Network. Otherwise ask the organiser for the gateway's address, then enter `<address>:7777` in **Dial address** on the Setup screen |
| "wallet has never synced" | The gateway is not reachable yet. Wait for a neighbour, then click **Sync now** |
| A payment says `refused` | The reason is shown in its history. The money does not move unless the payment also shows `mined` |

## For the organiser

Running the chain and the gateway, and creating invite files, is described in [mvp-v0.2.0.md](mvp-v0.2.0.md#running-a-session). Plain `ozm-node` devices and Basecamp devices can be in the same session.
