# Install Offline Zcash in Logos Basecamp

This guide adds the **Offline Zcash** app to [Logos Basecamp](https://docs.logos.co/basecamp). You can then pay other people on the same Wi-Fi with no internet. A gateway laptop with internet puts the payments on chain.

It is a test build: it runs on a local test chain with test coins, never real ZEC.

You need:
- Basecamp **0.3.2 or newer**, on Linux x86_64 or arm64, or macOS on Apple silicon.
- An **invite file** (`<your-name>.invite.json`) from whoever runs the session. It contains your private test keys, so don't share it.
- The same Wi-Fi as the session's gateway. That Wi-Fi does not need internet.

## 1. Install Basecamp

Download Basecamp from [its releases page](https://github.com/logos-co/logos-basecamp/releases/latest):
- **Linux:** `LogosBasecamp-Desktop-*-x86_64.AppImage` (or `aarch64`). Run `chmod +x` on it, then open it.
- **macOS:** `LogosBasecamp-Desktop-*-aarch64.dmg`. Drag Basecamp to Applications and open it. If macOS blocks it, right-click Basecamp and choose **Open**.

Open Basecamp once, then close it.

## 2. Get the two packages

Each platform has two files:
- `zcash_offline_module-<version>-<platform>.lgx`: the background service.
- `zcash_offline_ui-<version>-<platform>.lgx`: the app window.

`<platform>` is `linux-amd64`, `linux-arm64` or `darwin-arm64`.

**Option A: download** both files for your platform from the [Offline Zcash releases](https://github.com/megabyte0x/offline-zcash/releases/latest).

**Option B: build them** if there is no release for your platform yet. This needs [Nix](https://nixos.org/download) with flakes and about 10 GB of free disk. The first build takes 10–30 minutes.

```sh
git clone https://github.com/megabyte0x/offline-zcash && cd offline-zcash
(cd logos/zcash_offline_module && nix build .#lgx-portable --accept-flake-config -o ../../module-lgx)
(cd logos/zcash_offline_ui && nix build .#lgx-portable --accept-flake-config -o ../../ui-lgx)
ls module-lgx/*.lgx ui-lgx/*.lgx
```

## 3. Install both packages

1. Open Basecamp and go to the **Modules** view.
2. Click **Install LGX Package** and pick the `zcash_offline_module` file.
3. Click **Install LGX Package** again and pick the `zcash_offline_ui` file.
4. Both now appear in the lists, Offline Zcash under **UI Apps** and the module under **Logos Modules**.

Install the module before the app, because the app needs it.

## 4. Join the session

1. Click **Offline Zcash** in the sidebar. The **Setup** screen opens.
2. Open your invite file in a text editor, copy all of it, and paste it into the box.
3. Leave the other fields empty, then click **Join and start**.

Within a few seconds the header shows your name, and **mesh neighbours** lists the other devices on the Wi-Fi. If it stays at "none", see [Problems](#problems).

## 5. Get test coins and pay

1. Click **Get test funds (5 × 0.1 ZEC)**. About a minute later your balance shows about 0.5 ZEC.
2. Under **Pay offline**, choose a person in **To**, enter an amount (for example `0.01`), then click **Build, prove and send over the mesh**.
3. Follow the payment in **Payments**:
   - `sent` means it has left your device;
   - `accepted-pending` means the other person checked it and sent back a signed receipt;
   - `mined` means the gateway put it on chain.

Money you receive can be spent once it shows `mined` and your balance has updated.

## Problems

| What you see | What to do |
|---|---|
| Basecamp refuses the package | Check it is for your platform and is the portable file (not one ending `-dev`), and that Basecamp is 0.3.2 or newer |
| Offline Zcash is not in the sidebar | In Modules, click **Load** next to `zcash_offline_module`, then next to Offline Zcash |
| "module not reachable" | The module is not loaded. Load it in Modules and wait a few seconds |
| No mesh neighbours | You must be on the same Wi-Fi as the others, and it must not isolate devices from each other (guest networks often do). On macOS, allow **Local Network** for Basecamp when asked, or under System Settings → Privacy & Security → Local Network. Otherwise ask the organiser for the gateway's address, then enter `<address>:7777` in **Dial address** on the Setup screen |
| "wallet has never synced" | The gateway is not reachable yet. Wait for a neighbour, then click **Sync now** |
| A payment says `refused` | The reason is shown in its history. The money does not move unless the payment also shows `mined` |

## For the organiser

Running the chain and the gateway, and creating invite files, is described in [mvp-v0.2.0.md](mvp-v0.2.0.md#running-a-session). Plain `ozm-node` devices and Basecamp devices can be in the same session.
