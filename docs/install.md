# Installing ozm-node

`ozm-node` runs on macOS (Apple silicon/Intel) and Linux (x86_64/arm64,
glibc 2.35+; Debian 12 and Ubuntu 22.04 work).

**Current source: v0.7.7. Latest released CLI binary: v0.7.3.** Releases since
v0.7.4 ship only [Basecamp](basecamp-install.md). Build current source for the
automatic sync/preparation and fixes described below. Test coins only; no mainnet.

## Option 1: install script

```sh
curl -fsSL https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
```

The script selects your platform, checks SHA-256 and installs to `~/.local/bin`.
No GitHub account is needed.

| Variable | Meaning | Default |
|---|---|---|
| `OZM_VERSION` | Version to install | `0.7.3` |
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

The binary is unsigned. If macOS blocks a browser download as an unverified
developer, remove quarantine (the install script does this):

```sh
xattr -d com.apple.quarantine ~/.local/bin/ozm-node
```

## Option 3: build current source

With Git, Rust 1.97.1 and a native C/C++ build toolchain installed:

```sh copy Copy CLI Build
git clone https://github.com/megabyte0x/offline-zcash.git
cd offline-zcash
cargo install --locked --path crates/ozm-node --force
ozm-node --version
```

Keep the checkout for updates; `main` may include unreleased changes.

## Next: find a gateway and pay by address

With a funded Regtest gateway on your LAN, start without an invite:

```sh
ozm-node start --network regtest
```

Pass `--network` explicitly: new wallets default to testnet in current source,
Regtest in v0.7.3. Existing homes retain their saved network.

Start creates persistent local keys and pins the first gateway used, selected by
fewest hops then highest tip. The UI warns if that gateway disappears; it cannot
silently switch. Chain data is **gateway-reported**, not independently proved.
Preserve `~/.ozm`, especially `secret.json`; never regenerate a funded wallet.

In a second terminal, copy the address from `ozm-node status` and share it outside
the app. Paste the other person's address into the UI or use:

```sh copy Copy Address Payment
ozm-node fund --notes 3 --amount 0.1
ozm-node status
ozm-node pay --to '<recipient-regtest-unified-address>' --amount 0.01
ozm-node max
```

Wait for a spendable balance. `max` quotes `amount` and `fee_zat`; the UI's Max
fills that amount. It excludes locked notes, unmined change and uneconomic inputs.
Use a same-network unified address with an Ironwood receiver. Off-mesh addresses
submit with **no payee receipt**. Open/unknown payers never get pending acceptance.
[Gateway trust and faucet limits](basecamp-install.md#open-mode-trust-and-faucet-limits).

Keep the HTTP API on `127.0.0.1`. It checks Host and a per-run random token,
which the CLI obtains from the local UI.

For an invite, run `ozm-node join <invite.json>` **before** first start. Invited
wallets trust their named gateway. `pay --to` still accepts fleet names.

Every device pays, receives and relays; only `gateway` is a role. Create a fleet
with at least one gateway, for example
`ozm-node fleet-init --out invites --name my-session --network regtest --device Operator:gateway --device Alice --device Bob`.
Legacy roles such as `Alice:payer+payee` are accepted but ignored.

The CLI has no Leave session. To start over or switch network, keep the old home
and use a new one with `--home`; never delete a funded wallet.
For a Regtest operator, fund the gateway address with `ths faucet`, then run its
joined home with `--rpc http://127.0.0.1:18232 --mine`. `ths start` must remain
running. See [Basecamp operator setup](basecamp-install.md#gateway-on-regtest)
or the historical [invited CLI session](mvp-v0.2.0.md#running-a-session).

## Public testnet (v0.7.0)

Use a **new home/fleet**, never relabel a funded Regtest wallet. Testnet uses TAZ
and `utest1` addresses. New keys choose a scan start automatically, even when
created offline; existing wallets retain theirs. Sync is automatic. Seed
restoration is unsupported.

Create a gateway invite, install it on the device with internet, and start:

```sh copy Copy Testnet Gateway
ozm-node fleet-init --out ./testnet-fleet --name testnet --network testnet --device Gateway:gateway
ozm-node join ./testnet-fleet/Gateway.invite.json --home ~/.ozm-testnet-gateway
ozm-node start --home ~/.ozm-testnet-gateway
```

The gateway uses TLS `testnet.zec.rocks:443` and
`zaino.testnet.unsafe.zec.rocks:443`; no full node/VPS. Override `fleet.json`'s
`servers` or pass `--rpc 'https://server-a:443,https://server-b:443'`.
`--rpc` means lightwalletd on testnet, JSON-RPC on Regtest. Unreachable secondary
servers produce a **single-source** warning ([dated checks](components/chain.md#dated-service-checks)).
Testnet refuses `--mine` and gateway `fund`.

On each wallet laptop, create its own identity and discover the LAN gateway:

```sh copy Copy Testnet Wallet
ozm-node start --home ~/.ozm-testnet --network testnet
```

Request TAZ for that wallet's address at
[Valar](https://faucet.testnet.valargroup.dev), then wait for spendable funds.
[Faucet failures and note policy](components/offline-builder.md#v070-testnet-note-plan);
[funded Linux test evidence](open-work.md#built).

```sh copy Copy Prepare And Pay
ozm-node prepare --notes 10 --amount 0.0101
ozm-node pay --to '<recipient-utest1-address>' --amount 0.01
```

Automatic splits prepare sufficiently large funding; `prepare` is optional.
Commands use the default local API; pass `--api` for a changed port. Prepare
waits for gateway-reported mining, then requests sync. Confirm split notes are
spendable **before going offline**. ZIP 317 fees follow padded action count,
with a 10,000-zat floor. Unmined change is not spendable.

The gateway retries unchanged bytes every three blocks until expiry and reports
`submitted` → `included` (1 confirmation) → `mined` (10). These reports are
trusted, not chain proofs. Tracking stops at `mined`; it does not monitor later
reorgs. Refused payments may still mine until expiry. Keep wallet files and
signed transaction history for recovery.

## Bluetooth on Linux

Start experimental BLE with `ozm-node start --ble` or `OZM_BLE=true` (v0.4.1+).
It needs `bluetoothd`, system D-Bus and a powered LE adapter supporting simultaneous
scanning/advertising. No pairing. One process should own the default adapter
(`hci0` if present).

Run as your desktop user, not root. D-Bus/polkit must permit `org.bluez`, GATT,
advertising and discovery; grant those permissions, not the whole system bus.
Sandboxed launchers need system-bus access. libdbus is bundled; BlueZ is not.

The UI labels neighbours BLE/TCP; LAN may win duplicate-link selection. Missing
adapters or failed setup stop startup; omit `--ble` for LAN only. macOS BLE is
unsupported.

**Untested on real hardware until D1 passes.** Protocol/in-memory tests do not prove
radio compatibility, throughput or durability. See [desktop BLE gates](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).
