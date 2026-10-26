# v0.2.0: install it and pay offline on a shared LAN

**Status:** built and passing in containers, 26 October 2026. Not yet run on real laptops or real Wi-Fi. v0.1.0 proved the payment path on a four-container line mesh ([mvp-v0.1.0.md](mvp-v0.1.0.md)). v0.2.0 makes the same code installable by testers on their own laptops.

## What v0.2.0 adds

| Piece | What it does |
|---|---|
| Release builds | macOS (Apple silicon, Intel) and Linux (x86_64, arm64) tarballs with checksums, built by `.github/workflows/release.yml` on a `v*` tag |
| `scripts/install.sh` | One-line install with checksum check ([install.md](install.md)) |
| Invites | `ozm-node fleet-init` writes one `<name>.invite.json` per person (fleet file plus that device's keys) |
| `ozm-node join <invite>` / `ozm-node start` | Installs the invite into `~/.ozm` and starts the device with LAN discovery and the UI at `http://127.0.0.1:7780` |
| LAN discovery (`ozm-mesh`) | UDP beacons (multicast and broadcast, port 7778) tagged with the fleet. Devices of one fleet on the same Wi-Fi connect with no addresses typed in |
| Sync over the mesh | Offline devices ask the gateway for blocks (`syncq`/`syncr`) and rebuild their wallet from them. A payee can spend what it received once the payment is mined, without its own internet |
| Test funds over the mesh | `ozm-node fund` (or the UI button) asks a Regtest gateway to pay test notes from its own wallet (`fundq`/`fundr`) |

The Logos Basecamp packaging and Bluetooth LE radios are not in v0.2.0 (see [later/logos-v0.1.0-plan.md](later/logos-v0.1.0-plan.md) milestones 5–7).

## Running a session

The organiser:
1. Runs the local chain and the gateway on one laptop with internet: `ths start --no-open`, then `ozm-node join D.invite.json && ozm-node start --rpc http://127.0.0.1:18232 --mine`.
2. Creates the fleet: `ozm-node fleet-init --out invites --name my-session --device D:gateway --device Alice:payer+payee --device Bob:payer+payee …`.
3. Funds the gateway's address (shown by `fleet-init`) with `ths faucet <address> --amount 2`.
4. Sends each person their own invite file. It contains their private keys (disposable Regtest keys).

Each tester:
1. Installs `ozm-node` ([install.md](install.md)).
2. Joins the same Wi-Fi as the gateway. The Wi-Fi does not need internet. It must allow devices to see each other (no client isolation) and pass UDP broadcast on port 7778.
3. Runs `ozm-node join <their-invite>.json` then `ozm-node start`. The UI opens in the browser.
4. Presses **Get test funds**, waits about a minute, then pays another tester from the **Pay offline** panel.

A payment shows `accepted-pending` once the payee has checked it offline and sent its signed receipt, and `mined` once the gateway reports it on chain.

## Test results

`bench/lan-bench.sh all` runs the tester path: `join`, then `start` on four containers on one internal Docker network, with no `--dial`, and only D reaches the chain. All 10 checks pass:

| Check | Result |
|---|---|
| A, B, C cannot reach the chain | pass |
| Every device discovered the other three; one link per pair (12 neighbour entries) | pass |
| B synced its chain view from D over the mesh | pass |
| A got 3 × 0.1 ZEC from D's faucet over the mesh and saw it after a mesh sync | pass |
| A paid B 0.05 ZEC offline; mined; result at A and B | pass |
| B's balance shows the 0.05 ZEC after a mesh sync | pass |
| B paid C 0.01 ZEC from what it received; mined | pass |
| C restarted and rejoined through discovery | pass |

The v0.1.0 line bench (`bench/mesh-bench.sh all`, 23 checks) still passes on the same code.

Bug found and fixed by this bench: when A had synced past B's view, B refused the payment with code 6 (unknown anchor), while A's own submission still got it mined. An offline payee now syncs from the gateway over the mesh and checks once more before refusing. In the passing run B was at height 178, synced, and accepted the payment in 647 ms.

## Limits

- The chain is local Regtest, and everyone depends on the organiser's gateway for chain data. Gateway results and blocks are trusted as reported (no proofs yet).
- Discovery needs one IP network shared by all devices. There is no Bluetooth and no multi-hop across separate Wi-Fi networks, except by adding `--dial host:7777` by hand.
- Mesh traffic is sealed end to end, but discovery beacons announce each device's mesh ID and fleet tag in the clear on the LAN.
- Keys are plain JSON in `~/.ozm/secret.json`.
