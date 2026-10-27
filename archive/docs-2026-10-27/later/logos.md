# Later: Logos phase (not MVP)

The Logos pathway is tried first; the BitChat pathway is the fallback if Logos cannot work at all. The two are independent, and a bridge between them is optional. Build plan: [logos-v0.1.0-plan.md](logos-v0.1.0-plan.md).
Logos Delivery is a real-time, IP-only pub/sub network with bounded catch-up, not a delay-tolerant one, so multi-day partitions rest on long Store retention plus our own resend loop keyed by **(txid, auth_digest)**. A compatible v6 re-anchor keeps txid but changes authorizing bytes ([expiry/re-issue findings](../components/expiry-reissue.md)).
Nothing here is built or run. MVP transport: [../components/transport-bitchat.md](../components/transport-bitchat.md). Gateway: [../components/gateway.md](../components/gateway.md).

## Decided

- [decided, owner 26 October 2026] Two independent pathways. **Logos (first):** modified Basecamp or `logosctl` nodes with our Zcash module and its own UI, no BitChat on those machines. **BitChat (fallback):** used only if Logos cannot work at all; needing changes does not count. This supersedes "not in MVP v1; the four-Mac demo uses BitChat" ([reconciliation §3–4](../../archive/reports/Design%20review%20reconciliation.md)).
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
| Host size | Relay-only node about 0.5 GB RAM (2 GB recommended); measured about 36 MiB idle in the orb bench below. Basecamp GUI 4–8 GB | logos_messaging.md §6; bench below |

### Logos Zcash modules (brief)

- logos-co published eight Zcash repos in October 2026: wallet core, backend, CLI, UI, a lightwalletd routing module, and an in-process Zebra **v7.0.0** node (`logos-zebrad-module`, built by `logos-zebra-nix` with four non-consensus patches) ([logos_zcash_modules.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).
- Fit: the Zebra module forwards lightwalletd `SendTransaction` over Logos IPC, so it is the natural Logos-phase gateway; a relay module must be added to its `callers.json` allowlist ([grpc.rs L64–111](https://github.com/logos-co/logos-zebra-nix/blob/0b5e2ac1b69f90e84b6373eaac4df664a2c30aba/zebrad-c/src/grpc.rs#L64-L111)). Details belong in [gateway.md](../components/gateway.md).
- Not a fit for the builder: the wallet core signs and broadcasts in one job, exports neither raw bytes nor a PCZT, uses a 40-block expiry, and has no mobile targets or Logos Delivery code ([send.rs L326–382](https://github.com/logos-co/logos-zcash-wallet-core-module/blob/21b9f06ee10e0b7c43590b8f1a25396ccc15b6b2/rust-lib/src/send.rs#L326-L382)).
- Stack: upstream librustzcash (`zcash_primitives` 0.31.0, `orchard` 0.16) and Zebra, not the Zakura forks `ths` uses. Platforms: Linux, macOS, Windows. No LICENSE files; repos about two weeks old.
- **Re-checked 26 October 2026** ([org search](https://github.com/logos-co?q=zcash&type=all)): still exactly these eight repos. All are at the commits read above, with no pushes after 22 October.
- **What our `zcash_offline_module` can call, read for the Logos pathway** (logos-zebrad-module `a681945` README "API", "Settings" and "Why IPC"; `src/zebrad_module_impl.cpp` L19, L141; logos-zcash-node-module `952f78b` README "Callers", `rust-lib/src/gate.rs` L1–7):
  - **`zebrad_module`.** It exposes `configure`, `start`, `stop`, `status`, `grpc(path, bodyBase64)` and `logTail`. `grpc` reaches only Zebra's lightwalletd `CompactTxStreamer`, enough to `SendTransaction` and read `GetTreeState` and compact blocks.
    - It does not give full blocks with headers. The gateway's history-MMR rebuild needs Zebra JSON-RPC (`getblock`, `z_gettreestate`), which is available only through the opt-in `exposeJsonRpc` port with cookie auth.
    - Only `zcash_wallet_core_module` may call `grpc` by default. A `callers.json` such as `{"modules":["zcash_offline_module"],"allowHost":true}` in the instance directory replaces the list; the README labels it "for tests".
    - Regtest needs a `regtest.json` with upgrade heights, so the node runs its own Zebra Regtest chain, not the Zakura `ths` chain. For the laptop bench the gateway therefore calls Zakura RPC directly. `zebrad_module` is for testnet or mainnet, and only once the Zebra 7 ↔ Zakura transaction check below passes.
  - **`zcash_node_module`.** Only the wallet backend and wallet core may read its route table (same `callers.json` test override). Our module dials lightwalletd itself for pre-offline sync.
  - **`zcash_wallet_core_module`.** Refuses callers other than the backend, and exports no notes, witnesses or raw transactions. It is not reusable; our module keeps its own Zakura 2.2.0 wallet state. The two stacks can coexist in one Basecamp because each module runs in its own process.

### An offline-Zcash Basecamp module on the mesh (research, 26 October 2026)

Read in code, not built or run. Commits: logos-basecamp `572f722`, logos-liblogos `cd9f153`, logos-delivery-module `e2cb67a`, logos-delivery-test-harness `4c2ae5e`, logos-peering `e7e6f30`, logos-libp2p-module `9e53e54`, logos-rust-sdk `263d1c4`, logos-delivery `fd07b72` (v0.39.2).

- **A module is the right shape on desktops.** A `core` module is native code in its own `logos_host` process, and Basecamp does not sandbox it; only `ui_qml` apps are network-denied and must call modules (logos-basecamp `docs/spec.md` L157, L219). Rust modules are first-class: a `.lidl` contract plus `logos-module-builder` with `codegen.rust`, and typed calls to dependencies such as `delivery_module` (logos-rust-sdk README). The same module runs headless under `logosctl`, the `liblogos` CLI frontend (logos-logoscore-cli README), so a Mac gateway needs no GUI.
- **Mesh = IP only.** Logos Delivery and `libp2p_module` offer only TCP and QUIC (libp2p_module `metadata.json` L24); no logos-co, logos-messaging or vacp2p code mentions Bluetooth (GitHub code search). `delivery_module` accepts a private network without a preset: Logos's own test harness creates members with `messagingOverrides` `cluster-id` 42, `num-shards-in-network`, `kad-bootstrap-node` and `discv5-discovery: false` (harness `conf/member.json.tpl`). That answers whether the module can do this; the 7-day partition bench above is still the test.
- **Not on phones via Basecamp.** Basecamp's docs say its `ui-host` subprocess and `dlopen`ed plugins do not survive on iOS (`mock/README.md` L180–184). The `qt-ios` demo calls `logos_core_set_mode`, which no longer exists in liblogos, and the `MOBILE-HANDOFF.md` its READMEs cite is missing. Phones can embed `liblogosdelivery` directly: v0.39.2 has `liblogosdelivery-ios-*` and `-android-*` make targets (Makefile L621–710) plus `examples/ios` and `examples/mobile`. That is a library in our own app, not a Basecamp module.
- **BLE inside a module is possible but unproven.** A core module could bundle a BitChat-protocol BLE stack, but on macOS CoreBluetooth needs `NSBluetoothAlwaysUsageDescription` in the responsible app's `Info.plist`, and Basecamp's (`app/macos/Info.plist.in`) has none. [inference: TCC attribution of `logos_host` children was not tested]. The Rust BitChat ports (`ShilohEye/bitchat-terminal`, `vaibhav-mattoo/bitchat-tui`) were last pushed in mid-2025, so they likely trail the current protocol.
- **logos-peering** pairs two runtimes and lets one call an exported module on the other over mutual-TLS `tls_tcp`. It needs IP and is point-to-point, so it fits a camp Mac calling a gateway's `zebrad_module` over a LAN, not multi-hop relay.
- **Logos wants this built.** Its builders hub lists an unbuilt "Offline Circles Mesh" idea: relay Waku messages over Bluetooth and Wi-Fi mesh and reconcile on reconnect (logos-web `content/builders-hub/ideas/offline-circles-mesh/en.json`).

**Scope (owner, 26 October 2026): laptops only for this experiment; mobile comes later.** There are two independent pathways. They may interoperate later, but neither depends on the other:

1. **Logos pathway (tried first).** Basecamp nodes, modified where needed to run on an offline mesh, carry a Zcash offline module with its own UI. The sender's node sends a payment to a nearby node, and other modified Basecamp nodes hop it onward to one that has internet. That node submits it to the chain and sends the result back to both sender and receiver. There is no BitChat on these Macs. Logos is chosen because its nodes are lightweight enough for small hardware.
2. **BitChat pathway (fallback).** Used only if Logos cannot work with our system at all; needing changes does not count as failure. A BitChat fork with the offline Zcash changes runs on any phone and desktop with no Logos dependency. Its mesh design is being worked out in [double-spend.md](../components/double-spend.md) and [transport-bitchat.md](../components/transport-bitchat.md).

Logos pathway on laptops (proposed, not decided):

- **Node roles.** Every node runs the same build: the modified Basecamp (or headless `logosctl` on small relay hardware) with `delivery_module` on a private cluster and our `zcash_offline` module. The module has a `ui_qml` app for pay, receive and status. Logos Delivery relay does the hop-by-hop forwarding, and Store nodes give catch-up after a partition. A node with internet also acts as gateway. It submits through Zakura RPC on the `ths` chain, or through `zebrad_module` later on mainnet (relay added to `callers.json`). It then publishes the chain result on the same topic so the sender and receiver both get it.
- **Core library.** Put the builder, verifier, receipts, nullifier sharing and resend loop on the Zakura 2.2.0 pins in a Rust core that knows nothing about transport. The Logos module is a thin shell over it. If the BitChat pathway is ever needed, it reuses the same core without any Logos code.
- **Hardware size.** "Lightweight" fits headless nodes, not the Basecamp GUI. A relay-only Delivery node used about 36 MiB idle in the bench below, while Basecamp asks for 4 GB minimum and 8 GB recommended (Host size row above). Small relay boxes should run `logosctl` with the same modules, and the GUI is for user laptops.
- **Interop is optional.** Connecting Logos and BitChat is not needed for this experiment; a bridge is listed below as a later option only.
- The core does not wait on the double-spend design. Only two seams depend on it, and both stay open:
  - **Signing:** build to a proven, unsigned transaction plus sighash (the PCZT path), then apply signatures. This covers both solo and co-signed payments.
  - **Acceptance:** a policy fed by typed evidence messages.

### Laptop mesh compatibility for Logos (research, 26 October 2026)

Read in code at logos-delivery `fd07b72`, logos-delivery-module `e2cb67a` and nim-libp2p `29b3205`, plus Apple sources. Nothing was run.

**How Macs get an IP link with no internet:**

- **No ad hoc Wi-Fi.** Since macOS 11 there is no "Create Network", and `CWInterface startIBSSModeWithSSID` returns `-3930` ([Stack Overflow](https://stackoverflow.com/questions/70667333/programmatically-creating-an-ad-hoc-network-in-big-sur)).
- **One-hop hotspot.** The working workaround on Sonoma and Sequoia is Internet Sharing from a dummy network service, which makes one Mac a Wi-Fi access point ([Ask Different](https://apple.stackexchange.com/questions/464557/how-to-create-computer-to-computer-network-on-macos-sonoma-2023); [Apple Community](https://discussions.apple.com/thread/255897823)). That gives a star, not a multi-hop mesh.
- **Multi-hop needs routers.** Battery travel routers or OpenWrt 802.11s/batman-adv nodes, with the Macs as clients. This is the IP backbone already decided above.
- **AWDL is out for unmodified Logos.** Apple supports peer-to-peer Wi-Fi only through Network.framework `includePeerToPeer` ([TN3151](https://developer.apple.com/documentation/technotes/tn3151-choosing-the-right-networking-api)). BSD sockets need the unofficial `SO_RECV_ANYIF` option ([Yggdrasil](https://yggdrasil-network.github.io/2019/08/19/awdl.html)), which nim-libp2p never sets (`rg` found no match).

**Which parts of Logos can be plugged:**

- **Transports are fixed.** TCP is always on, WebSocket and QUIC are optional, and they are hard-wired in `waku_switch.nim` L172–182. There is no transport plugin ABI: the only C plugin ABIs are service discovery (`library/logosdelivery_service_discovery.h`) and RLN.
- **Discovery is pluggable, but not through `delivery_module` yet.** The service-discovery ABI returns `{peerId, addrs}` lists, so a provider could feed LAN peers found by Bonjour or announced over BitChat. Today `delivery_module` hosts it only over `libp2p_module` Kademlia (`src/service_discovery_plugin.h` L17–20), so a custom provider needs a module change or direct `liblogosdelivery` use.
- **The module API controls only topic and payload.** `delivery_module` exposes `send(contentTopic, payload)`, `subscribe`, `storeQuery` and reliable channels, with no timestamp or meta field (`src/delivery_module_plugin.h` L166–270). A bridge that republishes a message therefore gives it a new Logos message hash, so deduplication must use our own envelope ID, (txid, auth_digest).

**What "modify Basecamp for the mesh" means on laptops (proposed, not decided), in order:**

1. **Configuration, no fork.** Run `delivery_module` with no preset, a private cluster, fixed `--nodekey` identities and static entry nodes, over an offline IP network. Logos's own harness already does this.
   - **Multi-hop needs routers.** Relay forwards hop by hop, but only between nodes that share an IP link. A Mac hosting a hotspot cannot also join another Wi-Fi network on the same radio [inference: not tested]. A Mac chain across separate radio ranges therefore needs travel routers, OpenWrt mesh nodes or extra adapters.
   - **One hotspot Mac is enough for a single-room bench.**
2. **LAN discovery provider** through the service-discovery ABI, once static peer lists become a burden. This is a small `delivery_module` change.
3. **Fork logos-delivery for a radio transport.** Only this step lets Macs reach each other with no router: a nim-libp2p transport over BLE, or over AWDL with `SO_RECV_ANYIF`. BLE also needs a Bluetooth usage string in Basecamp's `Info.plist`. This is the fork the owner anticipated. It matches Logos's unbuilt "Offline Circles Mesh" idea, so raise it with Logos before building it alone.
4. **Later, optional:** a bridge between Logos and BitChat that republishes envelopes, deduplicates by envelope ID and carries a hop limit. Not part of this experiment.

**Running the Logos pathway offline (read in code, 26 October 2026):**

Commits: logos-delivery `fd07b72`, logos-delivery-module `e2cb67a`, logos-chat-module `8e5b68d`, logos-delivery-demo `2b6200d`, logos-logoscore-cli `c054ea0`, logos-basecamp `572f722`, logos-zcash-wallet-ui `dd72f45`.

- **The first module to create the node picks the network.**
  - `delivery_module` keeps one node per Logos runtime and refuses a second `createNode` with "Context already initialized" (`src/delivery_module_plugin.cpp` L649–653; the same rule is stated in logos-delivery-demo `src/delivery_demo_plugin.cpp` L36–41).
  - Basecamp itself never creates the node. Modules do: `chat_module` asks for `logos.test`, but it adopts an existing node "with the settings it was created with" (logos-chat-module `rust-lib/src/actions.rs` L259–298).
  - So our module should create the private-cluster node as soon as it loads. Any chat or other Logos app in that runtime then joins our offline network. A separate runtime (`logosctl --config-dir`, or Basecamp `--user-dir`) isolates it completely.
- **The configuration surface is enough.** `messagingOverrides` accepts `cluster-id`, `num-shards-in-network`, `entry-node`, `nodekey`, `nat`, `discv5-discovery`, `store`, `store-message-retention-policy`, `reliability`, `backfill-enabled` and `max-msg-size` (`api/conf/messaging_conf.nim`). The embedded library defaults `nat` to `none`; only the CLI defaults to `any` (`conf_builder/waku_conf_builder.nim` L784–788). A Basecamp node therefore makes no UPnP or NAT-PMP calls unless we ask for them.
- **Store sync is not available in the structured config.**
  - `store-sync` and `store-resume` exist only in the full kernel config (`tools/confutils/cli_args.nim` L429–438). That is reachable through the "kernel" entry layer, which has no send or subscribe, or through the legacy flat JSON shape, which the code marks for deletion (`api/conf/logos_delivery_conf_json.nim` L140–143).
  - Catch-up therefore relies on Store nodes with long retention, plus two built-in recovery paths that behave differently:
    - **After a restart,** startup backfill replays from the saved last-online time in 24-hour windows, page by page (`recv_service/backfill.nim` L149–182).
    - **After a partition heals without a restart,** reconnect backfill (`recv_service/recv_service.nim` L156–197, L215–235) sends a single Store query for the whole offline interval. It does not follow the pagination cursor and does not retry a failure.
  - Store servers reject any range longer than 24 h with `BAD_REQUEST` (`waku_store/common.nim` L14, L159–163; checked server-side at `waku_store/protocol.nim` L61–65), and a page holds 20 messages by default (`waku_store/common.nim` L10).
  - So after a partition longer than a day, or more than 20 missed messages on our shard, reconnect backfill recovers nothing beyond that. The 20-message limit was confirmed in the bench below; the 24 h limit was read in code only.
  - **Consequence:** `zcash_offline_module` runs its own catch-up through `delivery_module.storeQuery` (`src/delivery_module_plugin.h` L182–225), in windows of at most 24 h and following `paginationCursor`. It deduplicates by (txid, auth_digest) alongside the resend loop. Both reconnect gaps are worth reporting upstream to logos-delivery.
- **Clocks must agree within 20 seconds.**
  - The Store archive drops any message whose timestamp is more than 20 s from the Store node's own clock (`waku_archive/archive.nim` L37, L54–75; installed by default at `waku/node/waku_node/store.nim` L38).
  - Relay does not check timestamps (`waku_relay/protocol.nim` L574–595), so live delivery keeps working while Store silently stops archiving.
  - Offline laptops drift, so the mesh needs its own time source, for example an NTP server on a mesh router or the gateway, re-synced whenever it has internet. Our own envelopes must not depend on wall-clock agreement either.
- **Offline install works.** `logosctl install ./x.lgx` (or `--file`, `--dir`) skips the catalogue and needs no network (logos-logoscore-cli `docs/logosctl.md` L389–394). Sign the package with our own key and set `signature_policy: require` with that key in the keyring (same doc, L174–214). Basecamp uses the online catalogue only from its Package Manager UI (`docs/spec.md` L165, L297–303).
- **Hotspot Mac.** A Mac's Wi-Fi either joins a network or hosts one, never both ([How-To Geek](https://www.howtogeek.com/214053/how-to-turn-your-mac-into-a-wi-fi-hotspot); [Macworld](https://www.macworld.com/article/234236/how-to-share-a-wi-fi-connection-via-macos.html)).
  - If the hotspot Mac is also the gateway and shares a real internet connection, every Mac on the mesh gets internet and the test is no longer offline.
  - So either share a dummy service with forwarding blocked, or keep the gateway on a separate Mac that joins the mesh Wi-Fi and gets internet over Ethernet or USB tethering.
- **UI pattern to copy.** `logos-zcash-wallet-ui` is a `ui_qml` app (`interface: universal`) that depends on a backend module (`metadata.json`). `chat_module` is a Rust `cdylib` module built from a `.lidl` contract that depends on `delivery_module` (its `metadata.json`). Our pair is `zcash_offline_ui` (`ui_qml`) → `zcash_offline_module` (Rust `cdylib`) → `delivery_module`.

**Bench in the orb (measured 26 October 2026):**

Setup: prebuilt `logosdeliverynode` v0.39.2 (`waku-v0.39.2-amd64-linux.tar.gz`, no Nix) in Debian 12 containers. Four nodes in a chain on three `--internal` Docker networks, so each node reaches only its neighbours and nothing reaches the internet (checked: no DNS, no HTTPS from inside). Roles: `payer` – `relay1` – `relay2` – `gateway`, with Store (SQLite, 14-day retention) on the two relays. Flags: `--entry-layer=messaging --mode=core --cluster-id=99 --num-shards-in-network=1 --nat=none`, discv5, peer exchange and rendezvous off, fixed `--nodekey`, `--staticnode` to the neighbour, REST on. The gateway submitted to Zakura on the local `ths` v0.4.1 chain.

| Test | Result |
|---|---|
| Memory | About **36 MiB** per idle node, far below the 0.5 GB in the Host size row |
| Two-hop relay | Payer → gateway in **0.13 s**; `propagated` at once, `sent` (Store-confirmed) about 4 s later |
| End to end | A 9,166-byte v6 tx in a 12,247-byte JSON envelope reached the gateway byte-identical. `sendrawtransaction` accepted it, `ths mine 1` mined it at height 106, and the gateway's result message reached the payer in 0.84 s and the other end of the chain in 0.90 s, all live. The tx was a `ths wallet send` replayed after `ths rollback --depth 1`, not built by our offline builder |
| Clock skew (libfaketime on the sender, Store on a correct clock) | +10 s and 0 s: archived, `sent`. **+60 s and −60 s: still delivered live, not archived, sender gets an `error` event.** Confirms the ±20 s window; the sender can see the failure |
| Reconnect backfill (cut 30 s past detection, 30 messages, restore) | **20 of 30 recovered** as `history`, log `missedCount=20`: one page, no cursor, as read in code |
| Reconnect with no DNS | **Never reconnected.** Gateway cut for about 3 min, link restored at about 10:17; still `Disconnected` at 10:42 with the neighbour reachable over IP. Payer showed the same |

**New blocker found: a node with no DNS never comes back.** When every peer drops, `connectToRelayPeers` does nothing while the node is offline (`peer_manager.nim` L665–668). "Online" means a connected peer, *or* resolving `one.one.one.one` through `--dns-addrs-name-server` (default `1.1.1.1`, `1.0.0.1`) (`health_monitor/online_monitor.nim` L20–56). With no peers and no internet DNS the node is offline forever, so any partition that drops all peers is permanent until restart.
- **Workaround, tested:** a `dnsmasq` on the mesh that answers `one.one.one.one`, passed with `--dns-addrs-name-server`. The payer restarted this way recovered by itself after the cut.
- `dns-addrs-name-server` is not in the `delivery_module` `messagingOverrides` list (`api/conf/messaging_conf.nim`), so under Basecamp the mesh router must answer DNS on `1.1.1.1` itself (an address alias plus `dnsmasq`) [inference: not tested].
- Report upstream: offline detection should not depend on internet DNS.

**Also read in code, not run:** dial backoff is 120 s, 480 s, 1,920 s, 7,680 s, and after 5 failed dials a peer is never dialled again (`peer_manager.nim` L71–77, L510–532), with no flag to change it. A multi-hour partition can therefore exhaust a static neighbour. Our module should re-add its known peers (or the node should be restarted) when a link returns; test with a 3-hour cut.

**Prior art: [xAlisher/peers](https://github.com/xAlisher/peers) (read at `769fe2e`, 26 October 2026; MIT, alpha, Android only).** An Android messenger that embeds Logos (`liblogoschat` and `liblogosdelivery` as prebuilt arm64 `.so`), wire-compatible with Basecamp's `chat_module`. It has two offline transports beside Logos. Nothing was built or run here.

- **Logos on a phone works, in Edge mode, against an internet fleet.** Core/relay mode on a phone behind NAT lost its gossipsub mesh within about 30 s, so it delivered nothing. Edge mode (filter + lightpush) fixed it, with about 60 s warm-up before the first send lands (`docs/PROJECT_KNOWLEDGE.md` §10c). Co-located phones "do NOT peer over LAN — no local discovery" (same doc, §10b item 6). Its Logos transport therefore needs internet; it is not an offline Logos mesh.
- **Store catch-up, measured on device:** `contentTopics` plus a time window works, and adding a `pubsubTopic` filter "silently returns 0" (§10d). Matches our need to run our own `storeQuery` catch-up.
- **Bluetooth mesh is its own layer, not Logos.** Pure Kotlin GATT dual role (`BleMeshModule.kt`), at most 6 links, MTU 247, flood relay with TTL 3 and a dedup set (`src/native/bleFlood.ts`; `src/stores/bleStore.ts` L136), fragmentation with ARQ (`src/native/bleFrag.ts`), and a 4,096-byte frame cap at ingress (`BleIngressGate.kt` L35). A 1:1 message crossed BLE with Logos paused on 3 phones (commit `69878f2`). Its own review calls BLE ingress unauthenticated (`docs/adversarial-security-privacy-report-2026-08-04.md` F-09). Advertisements carry a "logos-online" flag bit (`bleStore.ts` L76–79), but nothing relays BLE traffic into Logos: the only re-forward is MeshCore → Logos.
- **MeshCore LoRa** carries about 160 bytes per packet with no fragmentation, so Peers mirrors short text instead of tunnelling (`docs/mesh-transport.md`). That is far too small for a 9,166-byte transaction.
- **Fit.** It is evidence for the BitChat pathway (phone BLE mesh, Logos-independent) and a working reference for embedding `liblogosdelivery` on Android. It does not make the Logos pathway offline-capable. Reusable ideas: BLE flood plus fragmentation for a roughly 12 KB envelope (about 55 fragments at 247 MTU, so the TTL-3, 6-link limits need testing), the "logos-online" advert bit as gateway discovery, and the bridge envelope with an origin and loop marker (`lr1:`, `docs/bridge-scenarios.md`). No iOS target.

**Module designs (proposed):** [logos-modules.md](logos-modules.md). The Zcash module codes against a `mesh_transport` contract with three swappable backends: `delivery_module`, `libp2p_module` (no online monitor, so no DNS blocker, but no Store), and our own `ble_mesh_module` ported from Peers' flood and fragmentation design. A fourth option, `mesh_offlineprotocol`, wraps the Offline Protocol Mesh SDK (AGPL), which has multi-hop store and forward built in and was benched in the orb. The file also holds the upstream issue drafts.

**Open for the Logos pathway:**

- [unverified] Our own windowed `storeQuery` catch-up after a 7-day partition, and recovery after the 5-failed-dial limit.
- [unverified] The same run under `delivery_module` in `logosctl` or Basecamp on Macs, including the `1.1.1.1` DNS workaround.

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
