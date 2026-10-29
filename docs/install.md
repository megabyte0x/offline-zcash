# Installing ozm-node

`ozm-node` runs on macOS (Apple silicon and Intel) and Linux (x86_64 and arm64, glibc 2.35 or newer, for example Debian 12 or Ubuntu 22.04).

**Current source: v0.7.7. Latest released CLI binary: v0.7.3.** Releases since
v0.7.4 publish only Basecamp packages. Use [Basecamp](basecamp-install.md) for
the current packaged app, or build the CLI below. The workflow sections describe
current source; automatic scan starts, readiness and later fixes are not in the
v0.7.3 binary. Testnet TAZ and Regtest only; no mainnet.

## Option 1: install script

Run:

```sh
curl -fsSL https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
```

The script picks the right build for your machine. It downloads v0.7.3, the newest release with CLI builds, checks its SHA-256 checksum and installs `ozm-node` to `~/.local/bin`. No GitHub account or token is needed.

You can set these options:

| Variable | Meaning | Default |
|---|---|---|
| `OZM_VERSION` | Version to install, for example `0.7.3` | `0.7.3`, the newest release with CLI builds (v0.7.4 ships only Basecamp packages) |
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

## Option 3: build current source

With Git, Rust 1.97.1 and a native C/C++ build toolchain installed:

```sh copy Copy CLI Build
git clone https://github.com/megabyte0x/offline-zcash.git
cd offline-zcash
cargo install --locked --path crates/ozm-node --force
ozm-node --version
```

Keep the repository checkout for updates. Building `main` includes changes after
the latest release; check its version and changelog before testing.

## Next: find a gateway and pay by address

With a funded Regtest gateway on your LAN, start without an invite:

```sh
ozm-node start --network regtest
```

New invite-free identities and `fleet-init` sessions default to testnet from
v0.7.4 (built from source); the released v0.7.3 CLI defaults to Regtest. Pass
`--network` explicitly to get the same result on both. Existing homes and fleet
files keep their saved network.

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
receipt**. Unified addresses must match the saved network and have an Ironwood
receiver: Regtest here, or `utest1` on public testnet.

The faucet allows one request/device/60 seconds, at most ten notes and the gateway's
configured total cap. First gateway use is trust-on-first-use, not trusted enrollment.
Address bindings prove receiving authority and pin the mesh key; they do not prove
spending authority. Open/unknown payers wait for gateway-reported inclusion, never
pending acceptance. The loopback API has a per-run random token and Host check;
the CLI obtains its token from the local UI. Keep it on `127.0.0.1`.
[Open trust model](basecamp-install.md#open-mode-trust-and-faucet-limits).

Invites still work: run `ozm-node join <invite.json>` **before** first start, then
`ozm-node start`. An invited device trusts its fleet's named gateway instead of
pinning the first one it hears. Existing fleet scripts may still pass names to `pay --to`.

Every device can pay, receive and relay; `gateway` is the only role. Create a fleet
with `NAME` or `NAME:gateway` per device and at least one gateway, for example
`ozm-node fleet-init --out invites --name my-session --network regtest --device Operator:gateway --device Alice --device Bob`.
Older invites, fleet files and inputs such as `Alice:payer+payee` still work; every
role except `gateway` is ignored.

The CLI has no Leave session. To start over or switch network, keep the old home
and use a new one with `--home`; never delete a funded wallet.
For a Regtest operator, fund the gateway address with `ths faucet`, then run its
joined home with `--rpc http://127.0.0.1:18232 --mine`. `ths start` must remain
running. See [Basecamp operator setup](basecamp-install.md#gateway-on-regtest)
or the historical [invited CLI session](mvp-v0.2.0.md#running-a-session).

## Public testnet (v0.7.0)

Use a **new home/fleet**, never relabel a funded Regtest wallet. Public testnet uses
TAZ and `utest1` addresses. Scan start is automatic for freshly generated keys,
including wallets created offline. Existing wallets keep their stored scan start;
existing fleet settings remain authoritative for the network. Sync runs in the
background without a manual step. Seed restoration is not supported.

Create a gateway invite, install it on the device with internet, and start:

```sh copy Copy Testnet Gateway
ozm-node fleet-init --out ./testnet-fleet --name testnet --network testnet --device Gateway:gateway
ozm-node join ./testnet-fleet/Gateway.invite.json --home ~/.ozm-testnet-gateway
ozm-node start --home ~/.ozm-testnet-gateway
```

The gateway defaults to TLS `testnet.zec.rocks:443`, then
`zaino.testnet.unsafe.zec.rocks:443`. Set `fleet.json`'s `servers` list or pass
`--rpc 'https://server-a:443,https://server-b:443'` to override it. This option is
a lightwalletd list on testnet, JSON-RPC on Regtest. No node/VPS is needed.
On 27 October, Zaino returned HTTP 503 and the primary was usable with a
**single-source** warning; that is dated evidence, not a live service check.
`--mine` and gateway `fund` are refused on testnet.

On each wallet laptop, create its own identity and discover the LAN gateway:

```sh copy Copy Testnet Wallet
ozm-node start --home ~/.ozm-testnet --network testnet
```

Use the UI/address from that running node. Request TAZ at
[Valar](https://faucet.testnet.valargroup.dev) (advertised 0.125 TAZ/address/day),
then sync and wait for spendable funds. Our orb claim failed with
`the wallet cannot pay this address`; zecpages timed out after 30001 ms.
[Exact funding evidence and note policy](components/offline-builder.md#v070-testnet-note-plan).
Owner-funded Linux Basecamp tests subsequently exercised an automatic split,
offline payment, reconnect and ten gateway-reported confirmations; see
[the 28 October verification](open-work.md#built). This is experimental testnet
software, not evidence of mainnet safety or independent chain verification.

```sh copy Copy Prepare And Pay
ozm-node prepare --notes 10 --amount 0.0101
ozm-node pay --to '<recipient-utest1-address>' --amount 0.01
```

Automatic economical splits already prepare sufficiently large funding; the
`prepare` command is an optional explicit split. These client commands target the default local API; use `--api` if you changed
its port. Prepare waits for gateway-reported mining, then requests sync. Confirm
the split notes are spendable **before going offline**. Larger splits pay ZIP 317
fees by padded action count; two-action payments retain the 10,000-zat floor.
Unmined change is never spendable.

The internet gateway watches blocks, resubmits unchanged bytes every three blocks
until expiry, and pushes results to payer/payee: `submitted` → `included` (1
confirmation) → `mined` (10). All chain facts are **gateway-reported**; TLS and
agreement between reachable servers are not inclusion/PoW/MMR proofs. Reorgs can
downgrade tracked results, but recurring polling stops at `mined`; a saved
ten-confirmation report is not continuous reorg monitoring. Refused payments may still mine
until expiry. Keep wallet files and signed transaction history for recovery.

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
on/off. Other Offline Zcash Android and Linux devices should appear within a few seconds;
LAN links may win the mesh's duplicate-link selection. Missing/off adapters or a
failed registration/scan produce a startup error; restart without `--ble` for LAN
only. macOS returns an unsupported-platform error for `--ble`.

**Untested on real hardware until D1 passes.** Protocol/in-memory tests do not prove
radio compatibility, throughput or durability. See [desktop BLE gates](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open).
