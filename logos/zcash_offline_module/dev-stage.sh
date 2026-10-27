#!/usr/bin/env bash
# Stages the gitignored inputs rust-lib/ needs for local cargo (build, clippy,
# Cargo.lock regeneration): the repository crates at rust-lib/workspace-crates/,
# the logos-rust-sdk source at logos-rust-sdk-src/ and the generated scaffold at
# rust-lib/generated/provider_gen.rs, all from the builder pin in flake.lock.
# The nix build stages its own copies and does not use these.
#
# After changing dependencies: ./dev-stage.sh && (cd rust-lib && cargo update -w)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$here"

rm -rf rust-lib/workspace-crates logos-rust-sdk-src
mkdir -p rust-lib/workspace-crates
for c in zcash-offline-core ozm-mesh ozm-node; do
  cp -r "$repo/crates/$c" rust-lib/workspace-crates/
done

# A git flake ref (not path:) so flake.nix can read ../../crates; new files
# must be tracked or `git add -N`ed to be visible.
flake="git+file://$repo?dir=logos/zcash_offline_module"
cp -RL "$(nix build --no-link --print-out-paths "$flake#rust-sdk-src")" logos-rust-sdk-src
chmod -R u+w logos-rust-sdk-src

gen="$(nix build --no-link --print-out-paths "$flake#lidl-gen")/bin/logos-lidl-gen"
lidl="$(nix build --no-link --print-out-paths "$flake#lidl")"
mkdir -p rust-lib/generated
# Local checks only: the nix build passes the exact logos-protocol version.
"$gen" "$lidl/zcash_offline_module.lidl" --provider --no-trait \
  --protocol-version "${LOGOS_PROTOCOL_VERSION:-0.9.0}" -o rust-lib/generated/provider_gen.rs
echo "staged; cargo now works in $here/rust-lib"
