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
2. Creates the fleet keys with `ozm-node fleet-init`.
3. Funds A with ten 0.1 ZEC notes using `ths faucet`.
4. Syncs A and B once in a one-shot container on `ths-default`.
5. Starts the four daemons. Each device's UI and API are published on `127.0.0.1`: A on 7781, R on 7782, B on 7783, D on 7784.

Knobs: `FRAME_DELAY_MS` (default 20 ms per 512-byte frame, emulating BLE) and `AMOUNT` (default 0.05 ZEC).

Talk to a device: `docker exec ozm-A ozm-node status | payments | events`, and `docker exec ozm-A ozm-node pay --to B --amount 0.05`.

Evidence after a run is in `bench/.state/`: `results.jsonl`, `<device>.log`, `<device>/events.jsonl`, and `R/cap.pcap` (the capture checked by test 6).
