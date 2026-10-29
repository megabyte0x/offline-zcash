# Official Logos catalogue submission

Status: **draft for the owner to submit**. On 27 October 2026,
`gh repo fork logos-co/logos-modules-release --clone=false` failed with
`HTTP 403: Resource not accessible by integration`. No fork or PR was created.
The source repository is public; no download credentials are needed.

Draft package/version text updated for v0.7.7 on 29 October. The upstream
contribution-format findings retain their 27 October scope; recheck them with
maintainers before submission. Nothing has been submitted by this refresh.

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
`8f56bf8`. Metadata declares version 0.7.7. Three thin apps share **one** core
engine; installing an app does not grant a fleet role. This is experimental
NU7 public-testnet / local Regtest software, not a mainnet wallet; it is not security reviewed.
Linux catalogue install/load/status is verified with released `logosctl`
0.3.2. Funded Linux Basecamp testnet flows have dated evidence. The owner tested
v0.5.1 on three Macs; current Mac builds and real BLE radios need verification.

Our public catalogue already publishes merged three-platform LGXs, schema-v2
index, complete manifest/root hashes, icon sidecars and SHA-256 checksums:

- Identity: https://raw.githubusercontent.com/megabyte0x/offline-zcash/main/logos-repo.json
- Index: https://github.com/megabyte0x/offline-zcash/releases/latest/download/index.json
- Releases: https://github.com/megabyte0x/offline-zcash/releases/tag/v0.7.7

Our packages are **unsigned**. `trustedSigners` is empty. A Merkle root checks
content identity, not publisher identity; we do not claim the official Logos
signature. For native publication, please build/merge/sign with the existing
official pipeline and key; no private key is supplied or requested here.

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

**Lower-effort alternative, subject to the official catalogue's trust policy:**
use the documented catalogue `includesUrl` mechanism to include these exact
four packages from our identity URL. This needs no engine/build changes, but
does **not** sign our packages with the Logos key. Add an includes document
(or extend the existing one), not an inline package list or generated index:

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
