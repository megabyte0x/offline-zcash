# Mesh bench

`mesh-bench.sh` runs v0.1.0's gating tests. Four devices run in four containers, linked as a line A – R – B – D. Each link is an internal Docker network, so packets must hop, and only D reaches the `ths` chain. Results are in [docs/mvp-v0.1.0.md](../docs/mvp-v0.1.0.md).

```bash
amp orb services ensure          # or: ths start --no-open   (local NU7 Regtest chain)
bench/mesh-bench.sh all          # build, create keys, fund A, sync, start the mesh, run the tests
bench/mesh-bench.sh up           # only bring the mesh up (then use the UIs below)
bench/mesh-bench.sh test         # rerun the tests on a running mesh
bench/mesh-bench.sh down         # remove containers, networks and bench/.state
```

What `up` does:
1. Builds `ozm-node` in release mode and an image `ozm-node:bench` (Debian 12 with tcpdump).
2. Creates the fleet keys with `ozm-node fleet-init`. Since v0.7.3 the scripts use the new form, for example `--device A --device R --device B --device D:gateway`: every device pays, receives and relays, and only the gateway is marked. Legacy forms such as `A:payer` or `R:relay` are still accepted, but every role except `gateway` is ignored.
3. Funds A with ten 0.1 ZEC notes using `ths faucet`.
4. Syncs A and B once in a one-shot container on `ths-default`.
5. Starts the four daemons. Each device's UI and API are published on `127.0.0.1`: A on 7781, R on 7782, B on 7783, D on 7784.

Knobs: `FRAME_DELAY_MS` (default 20 ms per 512-byte frame, emulating BLE) and `AMOUNT` (default 0.05 ZEC).

Talk to a device: `docker exec ozm-A ozm-node status | payments | events`, and `docker exec ozm-A ozm-node pay --to B --amount 0.05`.

Evidence after a run is in `bench/.state/`: `results.jsonl`, `<device>.log`, `<device>/events.jsonl`, and `R/cap.pcap` (the capture checked by test 6).

## LAN bench (v0.2.0)

`bench/lan-bench.sh all` runs the tester path. Four devices `join` their invites and `start` on one internal Docker network (`ozm-lan`) with LAN discovery and no `--dial`. Only D, which is also on `ths-default`, reaches the chain. The checks cover discovery, sync and test funds over the mesh, paying, spending received funds, and rediscovery after a restart.

Docker does not publish ports for containers that sit only on an internal network.
Use `docker exec lan-A ozm-node status`, which bootstraps the local API token.
The API rejects non-loopback Host headers; do not expose it as a network API.
D's UI is on `127.0.0.1:7794`.

## Open bench (v0.6.0)

`bench/open-bench.sh all` creates one invited gateway and six independent devices
with **no invites or manual dials**, on an internal Docker LAN. Only the gateway
can reach Zakura. It checks selected gateway/sync for all six, faucet rate limiting,
an address-payment ring (each device pays and receives, with inclusion receipts, no pending acceptance, and mined
results), an off-mesh destination, and Max with more than two actions. RPC lookups
independently check mining; Max must leave spendable and locked balances at zero.
Logs, synced snapshots and payment books remain in `bench/.state-open/`.
`down` removes only this disposable bench. No real radios are exercised.

## Local API and web injection regression

With a local `ozm-node` running, `node bench/ui-security-check.mjs http://127.0.0.1:7780`
uses installed `agent-browser` to inject hostile remote fields and checks that
names, addresses, payment states, txids, history and refusals render literally,
without executing HTML. Rust integration tests cover missing/wrong API tokens,
DNS-rebinding Host headers and forged sync replies; mesh tests cover forged acks.
