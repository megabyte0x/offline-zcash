# Installing ozm-node

`ozm-node` runs on macOS (Apple silicon and Intel) and Linux (x86_64 and arm64, glibc 2.35 or newer, for example Debian 12 or Ubuntu 22.04).

## Option 1: install script

Run:

```sh
curl -fsSL https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
```

The script picks the right build for your machine. It downloads the latest public release, checks its SHA-256 checksum and installs `ozm-node` to `~/.local/bin`. No GitHub account or token is needed.

You can set these options:

| Variable | Meaning | Default |
|---|---|---|
| `OZM_VERSION` | Version to install, for example `0.6.0` | latest release |
| `OZM_INSTALL_DIR` | Folder for the binary | `~/.local/bin` |
| `OZM_DRY_RUN=1` | Show what would be downloaded, then stop | off |

If `~/.local/bin` is not on your `PATH`, the script tells you how to add it.

## Option 2: download by hand

1. Open the [Releases page](https://github.com/megabyte0x/offline-zcash/releases).
2. Download the file for your machine and its `.sha256` file:

   | Machine | File |
   |---|---|
   | Mac with Apple silicon (M1 and later) | `ozm-node-<version>-aarch64-apple-darwin.tar.gz` |
   | Mac with Intel | `ozm-node-<version>-x86_64-apple-darwin.tar.gz` |
   | Linux x86_64 | `ozm-node-<version>-x86_64-unknown-linux-gnu.tar.gz` |
   | Linux arm64 | `ozm-node-<version>-aarch64-unknown-linux-gnu.tar.gz` |

3. Check the download. The command must print `OK`:

   ```sh
   shasum -a 256 -c ozm-node-<version>-<target>.tar.gz.sha256   # macOS
   sha256sum -c ozm-node-<version>-<target>.tar.gz.sha256       # Linux
   ```

4. Unpack it and move the binary onto your `PATH`:

   ```sh
   tar -xzf ozm-node-<version>-<target>.tar.gz
   mkdir -p ~/.local/bin
   mv ozm-node-<version>-<target>/ozm-node ~/.local/bin/
   ```

5. Check that it runs: `ozm-node --version`.

## macOS Gatekeeper

The binary is not signed or notarized. If you download it with a browser, macOS may block it with "cannot be opened because the developer cannot be verified". Remove the quarantine flag:

```sh
xattr -d com.apple.quarantine ~/.local/bin/ozm-node
```

The install script does this for you.

## Next: find a gateway and pay by address

With a funded Regtest gateway on your LAN, start without an invite:

```sh
ozm-node start
```

This creates persistent keys locally and syncs from a signed gateway, initially
selected by fewest hops then highest advertised tip and pinned on first use.
The web UI shows its identity and warns/blocks if the pinned gateway disappears. It
marks the view **gateway-reported**. It is not an independently verified chain
proof. Preserve `~/.ozm`, especially `secret.json`; do not regenerate a funded wallet.

In a second terminal, copy the address from `ozm-node status` and share it outside
the app. Paste the other person's address into the UI or use:

```sh copy Copy Address Payment
ozm-node fund --notes 3 --amount 0.1
ozm-node status
ozm-node pay --to '<recipient-regtest-unified-address>' --amount 0.01
ozm-node max
```

Wait for funding and a spendable balance before paying. `max` quotes `amount`
and `fee_zat` using the builder's ZIP 317 action count; the web UI's **Max** fills
that amount. Locked notes and unmined change cannot be spent. Uneconomic inputs
can remain after Max. Unknown mesh addresses still submit and display **no payee
receipt**; only Regtest unified addresses with Ironwood receivers are supported.

The faucet allows one request/device/60 seconds, at most ten notes and the gateway's
configured total cap. First gateway use is trust-on-first-use, not trusted enrollment.
Address bindings prove receiving authority and pin the mesh key; they do not prove
spending authority. Open/unknown payers wait for gateway-reported inclusion, never
pending acceptance. The loopback API has a per-run random token and Host check;
the CLI obtains its token from the local UI. Keep it on `127.0.0.1`.
[Open trust model](basecamp-install.md#open-mode-trust-and-faucet-limits).

Invites still work: run `ozm-node join <invite.json>` **before** first start, then
`ozm-node start`. Existing fleet scripts may still pass names to `pay --to`.
For operator setup see [Basecamp](basecamp-install.md) or the older invited
[CLI session guide](mvp-v0.2.0.md#running-a-session).

## Bluetooth on Linux

v0.4.1 and later add experimental BLE on Linux (v0.4.0 binaries do not have it). Start with `ozm-node start --ble` (or `OZM_BLE=true`). You need BlueZ's
`bluetoothd` running, system D-Bus, Bluetooth powered on and an LE adapter that
supports simultaneous scanning and peripheral advertising. No pairing is needed.
The default adapter is used (`hci0` if present); one node process should own it.

Run as your desktop user. The distribution's D-Bus/polkit policy must allow access
to `org.bluez`, GATT registration, advertising and discovery; if denied, ask the
administrator to grant those specific permissions rather than running the wallet
as root or opening the whole system bus. Sandboxed desktop launchers also need
system-bus access. libdbus is bundled into the binary; BlueZ is not.

The web UI lists neighbours as `Name (BLE)` or `Name (TCP)` and shows Bluetooth
on/off. Open or same-fleet Android and Linux devices should appear within a few seconds;
LAN links may win the mesh's duplicate-link selection. Missing/off adapters or a
failed registration/scan produce a startup error; restart without `--ble` for LAN
only. macOS returns an unsupported-platform error for `--ble`.

**Untested on real hardware until D1 passes.** Protocol/in-memory tests do not prove
radio compatibility, throughput or durability. See [desktop BLE gates](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).
