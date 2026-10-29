# Android test app (v0.7.7 source, not released)

Requires Android 10+ (API 29), **arm64-v8a** and BLE central/peripheral support.
**Test coins only.** Host-JVM tests cover the Kotlin↔Rust link contract, not
phones, radios, screens, background survival or phone proving cost.
[Hardware gates B2–B7](components/mesh.md#milestones-and-gating-tests) remain open.

These instructions describe current source. The last released APK, v0.7.3,
needs manual sync/preparation and lacks later recovery/gateway fixes.

## Install and join

1. Download `offline-zcash-0.7.3-arm64-v8a.apk` and its checksum from the
   [release](https://github.com/megabyte0x/offline-zcash/releases). In the download
   directory, run `sha256sum -c offline-zcash-0.7.3-arm64-v8a.apk.sha256`.
   For current source, use an `android-apk` artifact from a successful
   [CI run](https://github.com/megabyte0x/offline-zcash/actions/workflows/ci.yml)
   or build below.
2. Transfer the APK, open it in Files and allow installation from that source.
   For USB installation, enable USB debugging, install
   [Platform-Tools](https://developer.android.com/tools/releases/platform-tools),
   accept the phone's debugging prompt, check `adb devices`, then run
   `adb install -r <apk-path>`.
3. Enable Bluetooth. Allow **Nearby devices** on Android 12+, **Location** plus
   the system Location switch on 10/11, and notifications on 13+. Denied
   notifications must be enabled in Android Settings. **Select testnet before
   first start** if wanted: Android still defaults to Regtest.
4. Tap **Start mesh without invite / grant permissions**. Keys are created locally
   and reused on restart. Optional: **Join with invite file** before first start.
   Invites contain private keys; use only your own, never on two devices.
   Do not clear storage or uninstall a funded wallet to change fleets.
5. Join the gateway laptop's Wi-Fi: no client isolation, TCP 7777/UDP 7778 allowed.
   The phone needs no internet, lightwalletd or RPC access. A LAN-connected phone
   bridges BLE-only phones to the laptop. Neighbours list direct links only,
   labelled BLE/TCP. Keep one phone off Wi-Fi to test BLE without a LAN shortcut.
6. Wait for sync; **Sync from gateway** retries it. On Regtest, request
   **3 × 0.1 test ZEC** and wait for spendable funds. Share your address, paste a
   same-network recipient address, enter an amount or tap **Max**, then tap
   **Build and send payment** once. Max excludes fees, locked/unmined notes and
   uneconomic dust. Incoming funds require scanning before spending.
7. Keep the mesh notification running to relay. Scanning is continuous in front,
   5 s on / 15 s off in background; vendor battery controls may stop it.
   **Stop mesh** preserves pending packets/payments. Restart with the Start
   button, not by reopening the app. Stop/start after Bluetooth or permission changes.

Sync and inclusion/mining (1/10 confirmations) are **gateway-reported, not chain
proofs**. Open/unknown payers wait for inclusion, never pending acceptance.
Off-mesh addresses submit with **no payee receipt**. Verify the first gateway's
mesh ID with the operator; it is pinned on first use.
[Trust and faucet limits](basecamp-install.md#open-mode-trust-and-faucet-limits).

Both APK variants share a BLE UUID but are **not BitChat-compatible**. They use
a **public test signing key** (`android` / `androiddebugkey`): anyone can sign an
impersonating APK. Install only from trusted repository CI/releases.
`org.zcash.offline.test` disables backups and must never hold real funds.
Report phone/Android version, link kind, timings and errors—not private invites.

## Session owner: laptop gateway

On the laptop, install `ths` **v0.4.1**, Docker, `jq` and
[current CLI source](install.md#option-3-build-current-source), or use
[Basecamp](basecamp-install.md). Keep `ths start` running in a separate terminal;
run the remaining Regtest commands from an empty session directory.
In an orb, use `amp orb services ensure` instead of `ths start`;
the orb is not the phone's LAN. Public testnet needs only a
[TLS gateway and external faucet](install.md#public-testnet-v070).

```bash copy Copy Gateway Setup
ths start --no-open
# In a second terminal, in the session directory:
ozm-node fleet-init --out invites --name android-test --network regtest --device Gateway:gateway
ozm-node join invites/Gateway.invite.json --home "$PWD/gateway-home"
address=$(jq -r '.devices[] | select(.name=="Gateway") | .address' invites/fleet.json)
ths faucet "$address" --amount 1 --json
ozm-node start --home "$PWD/gateway-home" --rpc http://127.0.0.1:18232 --mine --no-open
```

Phones need no invite. For invited devices, add names to `fleet-init` and give
each only its own invite before first start. The gateway pays test funds from
its wallet; `--mine` is required because ths does not auto-mine submissions.
For failed discovery, check isolation, firewall, permissions and mesh notification.

## Build and verify in a fresh Linux orb

From the repository root with Rust/Docker/ths installed; no emulator or Bluetooth
required. Install JDK 17, the pinned SDK/NDK and cargo-ndk:

```bash copy Copy Android Build
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends openjdk-17-jdk-headless unzip
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64
export ANDROID_HOME="$HOME/android-sdk"
mkdir -p "$ANDROID_HOME/cmdline-tools"
curl -fL https://dl.google.com/android/repository/commandlinetools-linux-13114758_latest.zip -o /tmp/android-tools.zip
unzip -q /tmp/android-tools.zip -d "$ANDROID_HOME/cmdline-tools"
mv "$ANDROID_HOME/cmdline-tools/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
yes | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --licenses
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" 'platforms;android-35' 'build-tools;35.0.0' 'ndk;27.2.12479018'
rustup target add aarch64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked
./android/build.sh
./android/gradlew -p android --no-daemon :app:lintRelease :contract:compileKotlin
amp orb services ensure
./bench/ffi-bench.sh
```

If `amp orb services ensure` fails (for example `Timed out waiting for Portal proxy`) and `ths status` shows no running chain, start the chain without the dashboard portal, wait until `curl -s -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"getblockchaininfo","params":[]}' http://127.0.0.1:18232` returns a result, then run the bench:

```bash copy Copy No-portal Chain Start
amp orb service start ths --command 'exec "$HOME/.local/bin/ths" start --no-open'
```

Accept SDK licences; with an existing SDK, export `ANDROID_HOME` and skip its
download. `android/build.sh` generates Kotlin bindings, cross-builds
`libozm_ffi.so` for arm64/API 29 (16 KiB ELF alignment), and writes
`android/app/build/outputs/apk/release/app-release.apk` (no checksum file).
Build/bench output is gitignored; Gradle verifies its distribution checksum.

The JVM bench connects three real Rust nodes A–B–D through Kotlin callbacks and
`ChunkWriter`, with LAN/dials disabled and only D given RPC access. At chunks
**20/182/514**, it funds, syncs, pays 0.037 ZEC, stalls callbacks, reconnects and
checks receipts/mining against Zakura RPC. Expect three
`PASS Kotlin JNI chunk=...` lines and `BUILD SUCCESSFUL`. Reruns delete only
disposable `bench/.state-ffi` wallets.

**v0.6.0 local verification:** Kotlin compilation and Android lint passed (zero
errors, 27 warnings); the updated host-JVM contract passed all three chunk sizes
against a fresh isolated `ths` chain. A run against the accumulated chain near
height 280 timed out waiting for funds after large cold-sync responses and a link
disconnect. That long-history/20-byte-link case remains unresolved; the fresh-chain
pass does not establish reliable recovery or bounded sync under that load. No
v0.6.0 APK, Android screen or physical radio was tested locally.

## Public testnet in v0.7.0

Existing homes retain their network. New testnet keys use `utest1` and an
automatic scan start. Use the [TLS gateway](install.md#public-testnet-v070) and
Valar, not gateway funding. **Prepare for offline** manually splits a mined note
(default 10 × 0.0101), paying the ZIP 317 action-count fee. Wait for `mined` and
scanned spendable notes before disconnecting; unmined change is unusable.
[Funded Linux evidence](open-work.md#built) does not verify Android payments.

Current `versionName` is `0.7.7`, `versionCode` `16`. CI builds APK/checksum only
when Android or `ozm-ffi` changes; releases stopped publishing APKs after v0.7.3.
Since v0.7.2 Android's platform cleartext opt-in is disabled, but native mesh
TCP/BLE links remain plaintext beneath sealed envelopes. Inventory requires
v0.7.2+ peers; older peers only live-forward.
