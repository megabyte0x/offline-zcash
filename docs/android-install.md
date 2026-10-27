# Android test app (v0.5.0)

**Regtest coins only.** Android 10+ (API 29), **arm64-v8a**, Bluetooth LE central and peripheral support. This is an experimental test shell, not a mainnet wallet. The APK builds and the Kotlin↔Rust link contract is tested on a host JVM; **no real phone, radio, screen-off behaviour or phone proving cost has been verified**. B2–B7 in [mesh.md](components/mesh.md#milestones-and-gating-tests) are hardware gates, not passed tests.

## Install and join

1. Download `offline-zcash-0.5.0-arm64-v8a.apk` and its `.apk.sha256` from the [release](https://github.com/megabyte0x/offline-zcash/releases). To test unreleased `main`, download the `android-apk` artifact from a successful main-branch [CI run](https://github.com/megabyte0x/offline-zcash/actions/workflows/ci.yml), for example `gh run download <run-id> -R megabyte0x/offline-zcash --name android-apk`, or build below. Put both files in one directory and run `sha256sum -c offline-zcash-0.5.0-arm64-v8a.apk.sha256` there. If you built the APK yourself, it is `android/app/build/outputs/apk/release/app-release.apk` with no checksum file; use that path in step 2.
2. Transfer the APK and **your own** `<name>.invite.json` to the phone, open the APK in Files, and allow installation from that source. Alternatively, over USB: on the **phone**, enable Developer options and USB debugging; on the laptop, install [Android Platform-Tools](https://developer.android.com/tools/releases/platform-tools) (`adb`); connect and unlock the phone, accept the debugging prompt, check that `adb devices` lists it, then run `adb install -r offline-zcash-0.5.0-arm64-v8a.apk` (or the `app-release.apk` path). Never share an invite: it contains the device's private keys. Do not use the same invite on two running devices.
3. Enable Bluetooth and open **Offline Zcash**. Allow **Nearby devices** on Android 12+, or **Location** on Android 10/11 (also enable the system Location switch there). Allow notifications on Android 13+ so the foreground mesh notification is visible. No location permission is requested on 12+. The notification prompt only appears together with the Bluetooth prompt; if you denied it, enable notifications for Offline Zcash in Android Settings. If a permission was denied, tap **Start mesh / grant permissions** to retry.
4. Tap **Join with invite file**, select your JSON file, and wait for your name and `running`. Joining starts BLE and LAN discovery. On subsequent app/service restarts the installed invite and wallet are reused automatically; do not join again. The shell does not support changing identity. Never clear app storage or uninstall a funded device just to change fleets.
5. Join the laptop gateway's Wi-Fi. Internet is not needed on the phone or access point. At least one phone must share the laptop's LAN to bridge BLE-only phones. Laptops use TCP/LAN, **not BLE**. Neighbours show `TCP` for the laptop. Phones show `BLE` or `TCP`: phones on the same Wi-Fi also find each other over LAN, and the mesh keeps whichever direct link survives. To test the radio, keep at least one phone off the Wi-Fi so it can only reach the others over `BLE`; only direct neighbours are listed, not every reachable device. AP/guest-network client isolation must be off. Allow TCP 7777 and UDP 7778 through the laptop firewall on this trusted LAN. The phone holds a Wi-Fi multicast lock; it never needs access to lightwalletd or the node RPC.
6. With the gateway reachable, tap **Sync from gateway**, then **Request 3 × 0.1 test ZEC**. Requests are asynchronous; wait until tip is nonzero and spendable shows `0.3 ZEC` (may take minutes). Enter a fleet recipient's exact name and `0.01`, then **Build and send payment** once. First proving can be slow; do not tap repeatedly. Watch Payments for `sent` → `delivered` (mesh ack) → `accepted-pending` (signed payee receipt) → `mined` (signed gateway report). Fast transfers may skip visible intermediate states. `mined` is **gateway-reported, not an independently verified chain proof**. The recipient's balance changes after a later mesh sync.
7. Keep the mesh notification running to relay. Background scanning is 5 s on / 15 s off; foreground scanning is continuous. Vendor battery restrictions can still stop it. **Stop mesh** stops BLE and LAN; pending sealed packets/payments persist for restart. To restart it, tap **Start mesh / grant permissions** and wait for `running`; reopening the app from Home or Recents does not restart a stopped mesh. After toggling Bluetooth or changing permissions, tap **Stop mesh**, then **Start mesh / grant permissions**. Report phone model/Android version, link kind, times, and errors; never send private invite files.

Both APK variants use the same BLE service UUID. They are **not compatible with BitChat**. The checked-in **public test signing key** (password `android`, alias `androiddebugkey`) lets CI APKs upgrade without erasing wallets; anyone can sign an impersonating test APK, so install only from this repository's trusted CI/release. Package ID `org.zcash.offline.test` must never hold real funds. Backups are disabled.

## Session owner: laptop gateway

Install `ths` **v0.4.1**, Docker, `jq` and `ozm-node` v0.5.0 ([CLI install](install.md)); keep the gateway online during funding/mining. Alternatively use the [Basecamp Organiser and Gateway apps](basecamp-install.md) on the laptop. These commands run on the **laptop**, not the phone; run from an empty session directory. `ths start` stays running in a separate terminal. In an Amp orb use `amp orb services ensure` instead (an orb is not a phone's local LAN).

Before the release exists, build/install the CLI from this repository's `main`: `cargo install --locked --path crates/ozm-node --force`. This requires Rust and does not require the Android SDK. Then change to the empty session directory for the commands below.

```bash copy Copy Gateway Setup
ths start --no-open
# In a second terminal, in the session directory:
ozm-node fleet-init --out invites --name android-test \
  --device Alice:payer+payee --device Bob:payer+payee --device Gateway:gateway
ozm-node join invites/Gateway.invite.json --home "$PWD/gateway-home"
address=$(jq -r '.devices[] | select(.name=="Gateway") | .address' invites/fleet.json)
ths faucet "$address" --amount 1 --json
ozm-node start --home "$PWD/gateway-home" --rpc http://127.0.0.1:18232 --mine --no-open
```

Give Alice and Bob only their respective invites. The gateway serves test funds from its funded wallet; `--mine` mines submitted transactions (ths does not auto-mine external submissions). If discovery fails, check Wi-Fi isolation, firewall, permissions and the foreground notification before paying. BLE connection limits, throughput, disconnect recovery and background survival still require real-phone testing; do not interpret a passing JVM test as a radio test.

## Build and verify in a fresh Linux orb

From the repository root on `main` (v0.4.1 or later), with the repository's Rust/Docker/ths setup complete. No emulator or Bluetooth is needed. Install JDK 17, the pinned SDK/NDK, and cargo-ndk (downloads need internet):

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

SDK licence acceptance above is required. If the SDK is already installed, skip its download/unzip and export its existing `ANDROID_HOME`. `android/build.sh` generates Kotlin bindings from the host shared library, cross-builds `libozm_ffi.so` with cargo-ndk for arm64/API 29 (16 KiB ELF alignment), and builds the installable APK at `android/app/build/outputs/apk/release/app-release.apk`. Build products and disposable bench wallets are gitignored. The Gradle wrapper verifies its distribution checksum.

The JVM bench starts **three real Rust nodes A–B–D**, disables LAN discovery and manual dials, and passes every mesh byte through Kotlin `linkOpened`, `linkReceived`, `LinkSink.write`, `linkClosed` and the app's `ChunkWriter`. It requests funds, syncs, pays 0.037 ZEC at chunk sizes **20, 182, 514**, stalls callbacks, checks receipts and `mined` at both wallets against a real Zakura RPC lookup, and checks disconnect/reconnect. Expect three `PASS Kotlin JNI chunk=...` lines and a final `BUILD SUCCESSFUL`. Only D has an RPC URL. To rerun, the bench deletes **only its own** `bench/.state-ffi` disposable wallets.

Releases: workspace, Logos metadata/module and Android `versionName` are `0.5.0`, `versionCode` is `6`. CI builds the APK and checksum on main/PRs; a `v*` tag attaches both alongside CLI and four Logos packages per platform. Hardware gates remain open.
