#!/usr/bin/env bash
# Real Kotlin/JNA -> UniFFI -> Rust mesh/payment engine -> local Regtest gateway.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
STATE="$ROOT/bench/.state-ffi"
test ! -d "$STATE" || rm -rf "$STATE" # disposable keys and wallets, generated only by this bench
mkdir -p "$STATE"
cargo build --release --locked -p ozm-node -p ozm-ffi --features ozm-ffi/bindgen
./target/release/ozm-bindgen generate --library target/release/libozm_ffi.so --language kotlin --config crates/ozm-ffi/uniffi.toml --out-dir android/generated --no-format
./target/release/ozm-node fleet-init --out "$STATE/invites" --name ffi-bench --network regtest --device A --device B --device D:gateway >/dev/null
address="$(jq -r '.devices[] | select(.name=="D") | .address' "$STATE/invites/fleet.json")"
ths faucet "$address" --amount 1 --json | jq '{status, txid}'
./android/gradlew -p android --no-daemon :contract:run --args="$STATE http://127.0.0.1:18232"
