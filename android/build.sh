#!/usr/bin/env bash
# Android SDK/NDK and JDK 17 required; see docs/android-install.md.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
: "${ANDROID_HOME:?Set ANDROID_HOME to the Android SDK}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.2.12479018}"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
cargo build --locked --release -p ozm-ffi --features bindgen --bin ozm-bindgen --lib
./target/release/ozm-bindgen generate --library target/release/libozm_ffi.so --language kotlin --config crates/ozm-ffi/uniffi.toml --out-dir android/generated --no-format
cargo ndk -t arm64-v8a --platform 29 -o android/jniLibs build --locked --release -p ozm-ffi --lib
./android/gradlew -p android --no-daemon :app:assembleRelease
