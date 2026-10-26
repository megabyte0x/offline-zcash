# offline-zcash

Offline Zcash payments. A payer builds, proves and signs an Ironwood (NU7, v6) transaction with no internet. The transaction hops across a mesh of devices to the payee, who checks it offline and returns a signed receipt. A device with internet then submits it to the chain.

**v0.1.0:** four devices (payer, relay, payee and gateway) run in four containers on an emulated BLE mesh. A payment crosses two hops and is mined on a local NU7 Regtest chain. All seven gating tests pass. Results are in [docs/mvp-v0.1.0.md](docs/mvp-v0.1.0.md).

**v0.2.0:** testers install `ozm-node` ([docs/install.md](docs/install.md)), run `ozm-node join <invite.json>` and `ozm-node start`, and find each other on the same Wi-Fi. Offline devices get chain data and test funds from the gateway over the mesh. See [docs/mvp-v0.2.0.md](docs/mvp-v0.2.0.md).

```bash
ths start --no-open              # local chain: zcashlabs/thus-spoke-zakura v0.4.1
bench/mesh-bench.sh all          # v0.1.0: four-device line mesh, seven gating tests
bench/lan-bench.sh all           # v0.2.0: tester path on one LAN with discovery
```

| Path | What |
|---|---|
| [crates/zcash-offline-core](crates/zcash-offline-core) | Wallet sync, offline builder, payee checks, sealed envelopes, signed messages |
| [crates/ozm-mesh](crates/ozm-mesh/README.md) | BitChat-style mesh: TTL flooding, BLE-sized frames, store-and-forward, delivery acks |
| [crates/ozm-node](crates/ozm-node) | Device daemon: roles, payment store, gateway, HTTP API and web UI |
| [bench](bench/README.md) | Multi-container mesh bench and gating tests |
| [docs](docs/README.md) | Design, components, open work |

Regtest only. The keys are disposable, and nothing here has been reviewed for use with real funds.
