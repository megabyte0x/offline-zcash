#!/bin/sh
# Install ozm-node from the GitHub Releases of megabyte0x/offline-zcash.
#
#   curl -fsSL https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
#
# Environment:
#   OZM_VERSION      version to install, e.g. 0.2.0 (default: 0.7.3, the newest CLI release)
#   OZM_INSTALL_DIR  where to put the binary (default: $HOME/.local/bin)
#   OZM_DRY_RUN=1    print what would be downloaded and installed, then stop
set -eu

REPO="megabyte0x/offline-zcash"
API="https://api.github.com/repos/$REPO"
BIN="ozm-node"

say() { printf '%s\n' "$*"; }
err() { printf 'install.sh: error: %s\n' "$*" >&2; exit 1; }

# --- target ------------------------------------------------------------------

os="$(uname -s)"
arch="$(uname -m)"

case "$arch" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64 | amd64) arch=x86_64 ;;
  *) err "unsupported CPU architecture: $arch (supported: arm64/aarch64, x86_64)" ;;
esac

case "$os" in
  Darwin)
    # A shell running under Rosetta reports x86_64 on Apple silicon; prefer the native build.
    if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
      arch=aarch64
    fi
    target="$arch-apple-darwin"
    ;;
  Linux) target="$arch-unknown-linux-gnu" ;;
  *) err "unsupported operating system: $os (supported: macOS, Linux)" ;;
esac

# --- tools -------------------------------------------------------------------

command -v curl >/dev/null 2>&1 || err "curl is required"
command -v tar >/dev/null 2>&1 || err "tar is required"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  err "sha256sum or shasum is required to verify the download"
fi

# api_get URL [extra curl args...]: GET a GitHub API URL, with the token if set.
api_get() {
  url="$1"
  shift
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" -H "X-GitHub-Api-Version: 2022-11-28" "$@" "$url"
  else
    curl -fsSL "$@" "$url"
  fi
}

# --- version -----------------------------------------------------------------

# Releases from v0.7.4 ship only the Basecamp packages; 0.7.3 is the newest CLI build.
version="${OZM_VERSION:-0.7.3}"
version="${version#v}"
tag="v$version"

tarball="$BIN-$version-$target.tar.gz"
checksum="$tarball.sha256"
base_url="https://github.com/$REPO/releases/download/$tag"
install_dir="${OZM_INSTALL_DIR:-$HOME/.local/bin}"
dest="$install_dir/$BIN"

if [ "${OZM_DRY_RUN:-0}" = 1 ]; then
  say "target:   $target"
  say "version:  $version"
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    say "source:   GitHub API assets of release $tag (token set)"
  fi
  say "tarball:  $base_url/$tarball"
  say "checksum: $base_url/$checksum"
  say "install:  $dest"
  exit 0
fi

# --- download ----------------------------------------------------------------

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t ozm-node)"
trap 'rm -rf "$tmp"' EXIT INT TERM

if [ -n "${GITHUB_TOKEN:-}" ]; then
  # Use API asset URLs when the caller supplies API authentication.
  release_json="$(api_get "$API/releases/tags/$tag" -H "Accept: application/vnd.github+json")" ||
    err "release $tag not found, or GITHUB_TOKEN cannot read $REPO"
  # asset_url NAME: print the API URL of the asset called NAME. In each asset
  # object GitHub lists "url" (.../releases/assets/<id>) before "name".
  asset_url() {
    printf '%s' "$release_json" | tr ',' '\n' | awk -v want="$1" '
      /"url"[[:space:]]*:[[:space:]]*"[^"]*\/releases\/assets\/[0-9]+"/ {
        u = $0; sub(/^.*"url"[[:space:]]*:[[:space:]]*"/, "", u); sub(/".*$/, "", u)
      }
      /"name"[[:space:]]*:/ {
        n = $0; sub(/^.*"name"[[:space:]]*:[[:space:]]*"/, "", n); sub(/".*$/, "", n)
        if (n == want && u != "") { print u; exit }
      }'
  }
  for f in "$tarball" "$checksum"; do
    url="$(asset_url "$f")"
    [ -n "$url" ] || err "release $tag has no asset $f (no build for $target?)"
    api_get "$url" -H "Accept: application/octet-stream" -o "$tmp/$f" ||
      err "download of $f failed"
  done
else
  for f in "$tarball" "$checksum"; do
    curl -fsSL -o "$tmp/$f" "$base_url/$f" ||
      err "download of $base_url/$f failed. Check that release $tag exists and has a $target build."
  done
fi

# --- verify ------------------------------------------------------------------

expected="$(cut -d' ' -f1 "$tmp/$checksum")"
actual="$(sha256 "$tmp/$tarball")"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  err "checksum mismatch for $tarball (expected $expected, got $actual)"
fi
say "Checksum OK: $actual"

# --- install -----------------------------------------------------------------

tar -xzf "$tmp/$tarball" -C "$tmp"
src="$tmp/$BIN-$version-$target/$BIN"
[ -f "$src" ] || err "$BIN not found inside $tarball"

mkdir -p "$install_dir"
cp "$src" "$dest.tmp"
chmod 755 "$dest.tmp"
mv -f "$dest.tmp" "$dest"

if [ "$os" = Darwin ]; then
  xattr -d com.apple.quarantine "$dest" 2>/dev/null || true
fi

say "Installed $("$dest" --version 2>/dev/null || echo "$BIN $version") to $dest"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) say "" && say "$install_dir is not on your PATH. Add it, for example:" &&
     say "  echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.profile" ;;
esac

say ""
say "Next steps:"
say "  $BIN join <invite.json>   # join the session from the invite you were given"
say "  $BIN start                # start the node"
say "See docs/mvp-v0.2.0.md in the repository for the full session guide."
