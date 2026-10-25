# Later: Logos phase (not MVP)

After the BitChat MVP and mainnet, basecamps running Logos Delivery replace the Macs and BLE, and a logos-co Zebra module becomes the gateway.
Logos Delivery is a real-time, IP-only pub/sub network with bounded catch-up, not a delay-tolerant one, so multi-day partitions rest on long Store retention plus our own resend loop keyed by txid.
Nothing here is built or run. MVP transport: [../components/transport-bitchat.md](../components/transport-bitchat.md). Gateway: [../components/gateway.md](../components/gateway.md).

## Decided

- [decided] Not in MVP v1 or at the mainnet launch. The four-Mac harness demo uses BitChat ([reconciliation §3–4](../../archive/reports/Design%20review%20reconciliation.md)).
- [decided] Target `logosdeliverynode` from [logos-messaging/logos-delivery](https://github.com/logos-messaging/logos-delivery) **v0.39.2** (`fd07b72`), pinned exactly. v0.39.0 broke the binary name, env prefix (`WAKUNODE2_` → `LOGOS_DELIVERY_NODE_`) and C ABI.
- [decided] An IP backbone (5 GHz PtP, HaLow, 802.11s/batman-adv), not LoRa, carries Logos. LoRa or QR carries only the raw tx bytes through an application bridge ([report: IP backbone](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).
- [decided] Internet-free network: no preset, private `--cluster-id`, `--num-shards-in-network=1`, `--nodekey`, static `--entry-node` peers; discovery off; RLN off; Mix `None`.
- [decided] Two Store nodes suffice (a review asked for three; no code requires it).
- [decided] Payload confidentiality is ours: sign-then-encrypt envelopes, fixed size, one shared topic, never `ephemeral`.

## Findings

Sources: [logos_messaging.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_messaging.md), [reconciliation §4](../../archive/reports/Design%20review%20reconciliation.md), and logos-delivery code at v0.39.2.

### Logos Delivery facts

| Fact | Value | Source |
|---|---|---|
| Repo and binary | `logos-messaging/logos-delivery` (renamed from nwaku, then logos-messaging-nim); `logosdeliverynode` binaries for Linux x64 and macOS arm64 | [v0.38.0 notes](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.38.0); [releases](https://github.com/logos-messaging/logos-delivery/releases) |
| Transports | TCP, QUIC (default since v0.39.0, 3 s TCP fallback), WebSocket. No BLE or LoRa | [v0.39.0 notes](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0) |
| Store | Off by default (`--store=true`); default retention 2 days; `--store-message-retention-policy=time:1209600` for 14 days | [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md) |
| Sender with no peer | Gives up after 1 minute (`MaxTimeInCache`) and emits `error` | logos-delivery@fd07b72 `send_service.nim` |
| Store sync | Default window 1 h (`--store-sync-range`); designed for outages ≤1 h and ≤20 % loss | [WAKU-STORE-SYNC](https://lip.logos.co/messaging/core/raw/store-sync.html) |
| Store resume | `--store-resume` capped at 6 h | logos-delivery@fd07b72; WAKU-STORE-SYNC |
| Reliability | `--reliability` works only with `--entry-layer=messaging` or `channels`; `sent` needs a *different* Store node to confirm | [run-messaging.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-messaging.md) |
| Cluster IDs | 1 and 2 force the public `twn` / `logos.test` presets; never use them (avoid 3 and 16 too) | [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim) |
| RLN | Off by default with no preset; shipped backends need an Ethereum RPC or a LEZ host | networks_config.nim; [run-with-rln.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-with-rln.md) |
| SDS reliable channels | Ack timeout 5 s, 5 retransmissions (about 25 s of retrying); "Developer Preview" | [delivery module docs](https://docs.logos.co/messaging/delivery/use-logos-delivery-module-api-from-app) |
| Max message | 150 KiB default (`--max-msg-size`) | [default_values.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/waku_core/message/default_values.nim) |
| Mix | Needs ≥4 mix nodes (`MinMixPoolSize`; spec recommends ≥100); payload 3,984 B < 9,166 B tx; `Preferred` silently falls back to plain relay, `Required` fails at once | [waku_mix/protocol.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/waku_mix/protocol.nim); [modes.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/modes.nim) |
| Edge mode | Light client: relay and Store disabled; leaks peer ID and topics to its service node | [MESSAGING-API](https://lip.logos.co/messaging/application/raw/messaging-api.html) |
| Encryption | None on payloads by default. Node code accepts an app-supplied cipher per channel; the `delivery_module` channel API has no encryption callbacks | [channel_encryption.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/channels/encryption/channel_encryption.nim); delivery module docs |
| Presets | All hard-code `dns4` Status fleets, so an internet-free network starts from no preset and static peers | networks_config.nim; [report: internet-free Logos](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md) |
| Host size | Relay-only node about 0.5 GB RAM (2 GB recommended); Basecamp GUI 4–8 GB | logos_messaging.md §6 |

### Logos Zcash modules (brief)

- logos-co published eight Zcash repos in October 2026: wallet core, backend, CLI, UI, a lightwalletd routing module, and an in-process Zebra **v7.0.0** node (`logos-zebrad-module`, built by `logos-zebra-nix` with four non-consensus patches) ([logos_zcash_modules.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).
- Fit: the Zebra module forwards lightwalletd `SendTransaction` over Logos IPC, so it is the natural Logos-phase gateway; a relay module must be added to its `callers.json` allowlist ([grpc.rs L64–111](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/zebrad-c/src/grpc.rs#L64-L111)). Details belong in [gateway.md](../components/gateway.md).
- Not a fit for the builder: the wallet core signs and broadcasts in one job, exports neither raw bytes nor a PCZT, uses a 40-block expiry, and has no mobile targets or Logos Delivery code ([send.rs L326–382](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/send.rs#L326-L382)).
- Stack: upstream librustzcash (`zcash_primitives` 0.31.0, `orchard` 0.16) and Zebra, not the Zakura forks `ths` uses. Platforms: Linux, macOS, Windows. No LICENSE files; repos about two weeks old.

### Contradictions in sources

- Store retention: the Store guide says 4 h; the code default is 2 days (172,800 s). Code wins.
- QUIC: docs.logos.co lists it as "research only"; v0.39.0 release notes and code have it on by default. Code wins.
- RLN: the docs security page says "under research"; it is implemented. The design disables it regardless.
- Store nodes: review 1 asked for Store on all three nodes; two suffice ([reconciliation §3](../../archive/reports/Design%20review%20reconciliation.md)).
- The archived report's step-5 intent fan-out to gateways is superseded: gateways get tx bytes only.

## Open

- [unverified] Store sync and long `--store-sync-range` values (days) on SQLite.
- [unverified] Flags that turn off Rendezvous and peer-exchange serving in Core mode.
- [unverified] How an app talks to a headless `--entry-layer=channels` node (REST exists only for `messaging`).
- [unverified] Whether Edge mode is acceptable for phones after a privacy review.
- [unverified] Zebra 7 (upstream) accepts a tx built with Zakura Common 2.2.0, and the reverse ([MVP open check 2](../../archive/reports/MVP%20v1%20design.md)).
- [unverified] A 16 KiB envelope holds a 2-action tx plus signatures and tag; 4-action (15,478 B) does not fit.
- Bench test (report Phase 2): 4–6 nodes, no preset, private cluster, static peers, 14-day Store, reliability on; partitions of 1 h, 24 h and 7 days; packet capture shows zero DNS or internet egress. Pass: 100 % envelope delivery after a 7-day partition.
