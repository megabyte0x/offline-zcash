# Installing ozm-node

`ozm-node` runs on macOS (Apple silicon and Intel) and Linux (x86_64 and arm64, glibc 2.35 or newer, for example Debian 12 or Ubuntu 22.04).

## Option 1: install script

Run:

```sh
curl -fsSL https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
```

The script picks the right build for your machine. It downloads the latest release, checks its SHA-256 checksum and installs `ozm-node` to `~/.local/bin`.

While the repository is private, you need a GitHub token that can read it. Export it first:

```sh
export GITHUB_TOKEN=<your token>
curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/scripts/install.sh | sh
```

You can set these options:

| Variable | Meaning | Default |
|---|---|---|
| `OZM_VERSION` | Version to install, for example `0.2.0` | latest release |
| `OZM_INSTALL_DIR` | Folder for the binary | `~/.local/bin` |
| `GITHUB_TOKEN` | Token for a private repository | none |
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

## Next: join a session

Follow the session guide, [mvp-v0.2.0.md](mvp-v0.2.0.md). In short:

```sh
ozm-node join <invite.json>
ozm-node start
```
