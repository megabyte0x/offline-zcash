# Official Logos catalogue submission

**Draft, not submitted.** Forking failed with HTTP 403 on 27 October 2026;
no fork or PR exists. Package versions below are v0.7.7. Contribution-format
findings date from 27 October; confirm them with maintainers before submitting.

## PR title

Add Offline Zcash service and three Basecamp apps (testnet / Regtest)

## PR body

Please add these four packages at tag
[v0.7.7](https://github.com/megabyte0x/offline-zcash/releases/tag/v0.7.7):

| Package | Source directory | Type | Required dependency |
|---|---|---|---|
| `zcash_offline_module` | `logos/zcash_offline_module` | core (Rust cdylib) | none |
| `zcash_offline_ui` | `logos/zcash_offline_ui` | ui_qml | `zcash_offline_module` |
| `zcash_offline_organiser` | `logos/zcash_offline_organiser` | ui_qml | `zcash_offline_module` |
| `zcash_offline_gateway` | `logos/zcash_offline_gateway` | ui_qml | `zcash_offline_module` |

Source: https://github.com/megabyte0x/offline-zcash

All packages build with `nix build .#lgx-portable` from their directories on
`x86_64-linux`, `aarch64-linux` and `aarch64-darwin`. They pin module-builder
`8f56bf8`. Three apps share one engine; installing Gateway does not grant that
role. This is unaudited testnet/Regtest software, not a mainnet wallet.
Linux catalogue install/load/status passed with `logosctl` 0.3.2; funded Linux
Basecamp flows have [dated evidence](https://github.com/megabyte0x/offline-zcash/blob/main/docs/open-work.md#built). Current Mac builds and
physical BLE need verification; the Mac run used v0.5.1.

Our public catalogue already publishes merged three-platform LGXs, schema-v2
index, complete manifest/root hashes, icon sidecars and SHA-256 checksums:

- Identity: https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
- Index: https://github.com/megabyte0x/offline-zcash/releases/latest/download/index.json
- Releases: https://github.com/megabyte0x/offline-zcash/releases/tag/v0.7.7

Packages are **unsigned** (`trustedSigners` is empty). Merkle roots check content,
not publisher identity. For native publication, use the official build/merge/sign
pipeline and key; no private key is supplied or requested.

## Contribution-format constraint to resolve with maintainers

The official repository's README prescribes Git submodules and versioned
`metadata.json`, not a hand-edited index or asset URL list. Its current
`ci/combined.Jenkinsfile` scans only `submodules/<directory>/metadata.json`,
and platform jobs build `./submodules/<directory>#lgx-portable`.
This source is a **monorepo**: four metadata/flakes are under `logos/`, and
the core flake reads the shared `../../crates`. Adding four ordinary gitlinks
to the repository root would therefore be ignored, not publish four packages.
Do not copy engines or strip each directory into a standalone repository.

For native signed publication, add **one pinned source submodule**, then extend
the pipeline's package-name → flake-directory mapping for the four table rows.
Keep artifact names/tags keyed by metadata `name`, not the slash-containing
source path. Use Git flake refs with `?dir=logos/<name>` to retain access to the
repository's shared crates. Apply the mapping consistently to discovery,
version checks, target evaluation and platform builds. Existing root-layout
modules keep their current paths. Publish via the existing merge/sign/index
steps; do not hand-edit `index.json`.

The owner can prepare the source gitlink in a fork with:

```sh copy Copy Source Registration
git switch -c add-offline-zcash
git submodule add https://github.com/megabyte0x/offline-zcash submodules/offline-zcash
git -C submodules/offline-zcash checkout v0.7.7
git add .gitmodules submodules/offline-zcash
```

**Alternative, subject to catalogue policy:** include these packages through
`includesUrl`. This avoids build changes but leaves them unsigned. Add an
includes document, not an inline package list or generated index:

```json copy Copy Catalogue Include
{
  "schemaVersion": 1,
  "includes": [{
    "repo": "https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json",
    "packages": [
      {"name": "zcash_offline_module", "version": "0.7.7"},
      {"name": "zcash_offline_ui", "version": "0.7.7"},
      {"name": "zcash_offline_organiser", "version": "0.7.7"},
      {"name": "zcash_offline_gateway", "version": "0.7.7"}
    ]
  }]
}
```

Point the official `logos-repo.json`'s `includesUrl` to that public document.
Maintainers must choose native signed publication or an explicitly unsigned
include before this can be described as an official-catalogue release.

References inspected: official README, `.gitmodules`, `ci/combined.Jenkinsfile`,
`ci/linux.Jenkinsfile`, `ci/macos.Jenkinsfile`, `rebuild-index.yml`, and the
[catalogue format](https://github.com/logos-co/logos-modules-release-tool/blob/94462e6633f2b536424c10ceda3c371f7ec8e09d/docs/catalog-format.md).
