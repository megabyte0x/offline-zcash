# Offline Zcash

Pay with Zcash when there is no internet. Your device builds, proves and signs a shielded payment on its own, then passes it from device to device over a local mesh until it reaches the person you're paying. They check it offline and send back a signed receipt. As soon as any device on the mesh has internet, it puts the payment on chain, and everyone sees it confirmed.

![An offline Zcash payment: Alice to Bob over the mesh, then a gateway puts it on chain and the confirmation comes back](docs/diagrams/1-payment-journey.png)

More diagrams, inside the mesh and at the gateway: [docs/how-it-works.md](docs/how-it-works.md).

It runs as an app in [Logos Basecamp](https://docs.logos.co/basecamp) or as a command-line node, and both kinds of device can be in the same session.

**Status:** test build. It runs on a local Zcash test chain (NU7, Ironwood) with test coins, never real ZEC, and has not been security reviewed.

## Try it

- **[Install it in Basecamp](docs/basecamp-install.md):** install the app, join a session with your invite, get test coins and pay someone.
- **[Use the command-line node](docs/install.md):** install `ozm-node`, then `ozm-node join <invite.json>` and `ozm-node start`.
- **[Run a session](docs/mvp-v0.2.0.md#running-a-session):** run the test chain and the gateway, and hand out invites.

Downloads are on the [releases page](https://github.com/megabyte0x/offline-zcash/releases/latest).

## For developers

| Path | What |
|---|---|
| [crates/zcash-offline-core](crates/zcash-offline-core) | Wallet sync, offline builder, payee checks, sealed envelopes, signed messages |
| [crates/ozm-mesh](crates/ozm-mesh/README.md) | BitChat-style mesh: TTL flooding, BLE-sized frames, store-and-forward, acks, LAN discovery |
| [crates/ozm-node](crates/ozm-node) | The device: roles, payment store, gateway, HTTP API and web UI |
| [logos](logos/README.md) | Basecamp core module and QML app |
| [bench](bench/README.md) | Multi-container tests: `bench/mesh-bench.sh all` (multi-hop) and `bench/lan-bench.sh all` (LAN) |
| [docs](docs/README.md) | Design, components, results per version ([v0.1.0](docs/mvp-v0.1.0.md), [v0.2.0](docs/mvp-v0.2.0.md)) |
