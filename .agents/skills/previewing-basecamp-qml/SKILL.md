---
name: previewing-basecamp-qml
description: Renders the Offline Zcash Basecamp QML apps (wallet, Organiser, Gateway) in seconds with a mock zcash_offline_module bridge, clicks through a flow, records which module methods and arguments the UI sends, and saves screenshots. Use to check a QML or UI-flow change from the user's point of view without building .lgx packages or starting Basecamp.
---

# Previewing Basecamp QML

Building `.lgx` packages with Nix takes a long time on a fresh orb, and Basecamp needs a
glibc 2.39 container (see `testing-basecamp-testnet`). For QML layout, copy and button
flows, render the real `logos/*/qml/Main.qml` with PySide6 and a mock `logos` bridge instead.
This proves what the UI shows and **which module calls it makes**. It does not prove engine
behaviour: test that with `cargo test` against `crates/ozm-node` (targeted test names only),
and use `testing-basecamp-testnet` for released packages on the real chain.

## Setup (once per orb, about 20 s)

```bash
uv venv -q /tmp/qmlenv && VIRTUAL_ENV=/tmp/qmlenv uv pip install -q PySide6-Essentials
```

## Run a scenario

From the repository root:

```bash
QT_QPA_PLATFORM=offscreen /tmp/qmlenv/bin/python \
  .agents/skills/previewing-basecamp-qml/scripts/qml_preview.py \
  .agents/skills/previewing-basecamp-qml/scenarios/wallet-new-wallet.json /tmp/qml-shots
```

Output lines `call <method> <args>` are the module calls in order (the 3-second `status`
poll is omitted); `shot <path>` lines are screenshots. Inspect every screenshot with
`view_media` and an objective naming the expected result, then copy the ones the user should
review into `.amp/in/artifacts/`. `QML error:` lines mean the file does not load: fix them first.

## Scenario format

See the docstring in `scripts/qml_preview.py`. Key points:

- `replies` maps module method → reply JSON (what the Rust module would return). A list is
  consumed one reply per call, last one repeating. Copy real field names from
  `crates/ozm-node/src/app.rs` (`status`) and `logos/README.md` (method table).
- `steps` act on QML `objectName`s: `click`, `set` (`[name, property, value]`), `wait` ms,
  `scroll` (content Y of the first Flickable), `shot`. Popups are found too.
- Give new buttons/fields an `objectName` so scenarios can drive them.

Bundled scenarios:

| File | Flow |
|---|---|
| `scenarios/wallet-new-wallet.json` | Syncing wallet → Debug → Start a new wallet… → confirm; expect `leave` then `start {"network":"testnet","new_wallet":true}` |
| `scenarios/organiser-gateway-session.json` | Organiser without device names → Create gateway session; expect `fleet_init` with `devices:["Gateway:gateway"]` and `join_device:"Gateway"` |

## Time rules

- Do not rebuild Nix packages to check QML; do not rerun the full workspace tests for a
  QML-only change. Run one scenario per changed flow.
- For Rust engine changes run the narrowest test, e.g.
  `cargo test -p ozm-node --test session <name>`; incremental builds take about 20 s once
  `target/` exists. [blaze](https://ix.dev/blaze/) promises faster incremental rebuilds, but on a
  Linux orb `nix run github:indexable-inc/blaze` has no binary cache and compiles blaze itself
  from source with LTO (still compiling after 3 minutes on 29 October 2026). Use it only on a
  machine that already has it installed; it also needs Rust 1.99 while the module pins 1.97.1.
- The module crate `logos/zcash_offline_module/rust-lib` cannot `cargo check` outside Nix
  (its `logos-rust-sdk` path dependency is injected by the flake); keep its changes small and
  covered by ozm-node tests.
