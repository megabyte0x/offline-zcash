# Logos Messaging (formerly Waku) as the transport for an internet-free Zcash "basecamp" mesh

Research date: 2026-10-24. Scope: Logos Messaging / Logos Delivery and the wider Logos stack. Most facts come from primary sources: the logos-delivery GitHub repo (code, CHANGELOG, operator docs), lip.logos.co specs, docs.logos.co and the Logos blog. Where docs.logos.co conflicts with newer code or release notes, both are flagged below.

Naming map (verified): "Waku" is "an old name for the Logos Delivery component"; "Codex" is the old name for Logos Storage; "Nomos" is "an old name for the Logos Blockchain" — [docs.logos.co glossary](https://docs.logos.co/get-started/glossary). The Nim implementation repo was renamed `nwaku` → `logos-messaging-nim` → `logos-delivery`, per the v0.38.0 note "Repository renamed from `logos-messaging-nim` to `logos-delivery`" — [logos-delivery v0.38.0 release](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.38.0).

---

## 1. What is Logos Messaging as of late 2026 (protocols, implementations, maturity, APIs)?

### Takeaway
Logos Messaging is two modules: **Logos Delivery** (the old Waku: a libp2p/GossipSub pub-sub network with Store, Filter, LightPush, Peer Exchange, RLN and Mix) and **Logos Chat** (end-to-end encrypted 1:1 chat, with group chat via de-MLS still in development). The reference implementation is `logos-delivery` (Nim). The latest stable release is v0.39.2 (2026-10-13). v0.39.0 (2026-09-30) was a major breaking release: it added a new layered "Messaging API" (send/subscribe plus reliable channels using SDS), a unified C FFI library (`liblogosdelivery`), QUIC on by default, and Mix sender anonymity. Everything is still pre-1.0 and testnet-grade, and most specs are "raw" status.

### Cited Findings
**Overall structure**
- Logos Messaging is "the messaging layer of the Logos stack", made of two modules: Delivery (peer-to-peer transport with spam protection) and Chat (E2E-encrypted conversations built on Delivery). It has no central broker and no gas fees, and is built on libp2p with RLN spam protection added. The docs say the page "reflects the state of this Logos component as it will exist on mainnet", so some features may be missing on the current testnet — [docs.logos.co/messaging](https://docs.logos.co/messaging)
- Delivery protocols listed: Relay (extends GossipSub), RLN Relay, Filter, Store, and Light Push. Store "temporarily retains messages for offline peers" and does not guarantee availability. Light Push gives an ack that at least one peer received the message, but no network-wide propagation guarantee — [docs.logos.co/messaging](https://docs.logos.co/messaging)
- FAQ: Delivery targets "short, ephemeral, real-time messages" and "doesn't currently support large data, for privacy reasons". The network applies no default payload encryption — [docs.logos.co FAQ](https://docs.logos.co/messaging/get-started/faq)

**Spec suite (lip.logos.co, the successor to rfc.vac.dev)** — [lip.logos.co index](https://lip.logos.co/)
- Stable: 11/WAKU2-RELAY, 14/WAKU2-MESSAGE.
- Draft: 12/FILTER, 13/STORE, 17/RLN-RELAY, 19/LIGHTPUSH, 34/PEER-EXCHANGE, 26/WAKU2-PAYLOAD, 53/X3DH.
- Raw: WAKU-MIX, WAKU-STORE-SYNC, WAKU-SYNC, RELAY-SHARDING, WAKU2-RLN-CONTRACT, MESSAGING-API, RELIABLE-CHANNEL-API, WAKU-P2P-RELIABILITY, WAKU2-NOISE, WAKU2-NOISE-SESSIONS, WAKU-RLN-KEYSTORE.
- AnonComms (raw): MIX, LOGOS-MIXNET, SDS, RLN-V2, RLN-API, DECENTRALIZED-MLS-OFFCHAIN-CONSENSUS (de-MLS), NOISE-X3DH-DOUBLE-RATCHET, Payment Streams.

**Messaging API (new high-level API)**
- MESSAGING-API spec (raw, API version 0.0.1, library name liblogosdelivery, last revised 2026-06-16) defines two modes. **Edge** is for mobile and browsers: it acts as a client of Lightpush/Filter/Store/Peer-Exchange. **Core** is the default: it runs Relay, serves Lightpush/Filter/Peer-Exchange, and uses Discv5 and Rendezvous. Functions: `createNode`, `send(MessageEnvelope{contentTopic,payload,ephemeral})` returning a RequestId, `subscribe`/`unsubscribe`. Events: `message_sent`, `message_propagated`, `message_error`, `message_received`, plus connection/topic health events — [MESSAGING-API](https://lip.logos.co/messaging/application/raw/messaging-api.html)
- The node app's API layer is chosen with `--entry-layer`: `kernel` (transport only, the CLI default), `messaging` (kernel + messaging client), or `channels` (kernel + messaging + reliable channels) — [modes.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/modes.nim); [v0.39.0 release](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)
- REST: with `--entry-layer=messaging --rest=true` the API is served at `/messaging/v1`. Endpoints: `POST /messaging/v1/messages`, `GET /messaging/v1/events/send/{requestId}` (events `propagated`, `sent`, `error`), `GET /messaging/v1/events/received`, and `GET /health` (wait for `connectionStatus` = `Connected`/`PartiallyConnected`) — [run-messaging.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-messaging.md)
- Logos Core module wrapper (`delivery_module`) API: `createNode(json)`, `start()`, `stop()`, `subscribe(contentTopic)`, `send(contentTopic,payload)`. Reliable Channels API (`channelCreate/channelSend/channelClose`) is labelled "Developer Preview, not feature-complete". Config example `{"mode":"Core","preset":"logos.test"}`; the docs page is accurate for delivery_module v0.2.1 — [docs.logos.co delivery module API](https://docs.logos.co/messaging/delivery/use-logos-delivery-module-api-from-app). The latest delivery_module release is v0.3.3 (2026-10-14) — [logos-delivery-module releases](https://github.com/logos-co/logos-delivery-module/releases)

**Release status and version numbers**
- Releases: v0.39.2 (2026-10-13, patch for a "status.prod store incident"), v0.39.1 (2026-10-06, bumped nim-libp2p to 2.4.1), v0.39.0 (2026-09-30), v0.38.1 (2026-05-11), v0.38.0 (2026-04-10 per GitHub; the CHANGELOG says 2026-03-16). Nightly pre-releases continue; the 2026-10-24 nightly adds "accept a message priority on send" and "mix: use shared RLN proofs for native Delivery routing" — [logos-delivery releases](https://github.com/logos-messaging/logos-delivery/releases); [CHANGELOG](https://github.com/logos-messaging/logos-delivery/blob/master/CHANGELOG.md)
- v0.39.0 highlights:
  - `logosdeliverynode` replaces `wakunode2` and adds a messaging REST API.
  - "sender anonymity through Mix (`anonymityLevel`), per-channel encryption with app-supplied ciphers, message segmentation, Store catch-up across process restarts".
  - QUIC on by default with TCP fallback; Kademlia/service discovery; pluggable RLN API module; `rlnkeystore` tool.
  - liblogosdelivery unified on the nim-ffi 0.3 typed C ABI, later moved to a CBOR call surface; Android and iOS builds restored.
  - "ReliabilityManager wired into the Reliable Channel, with SDS persistency".
  - Built with Nim 2.2.6, nim-libp2p v2.3.1 and zerokit RLN v2.0.2.
  - Env var prefix changed `WAKUNODE2_` → `LOGOS_DELIVERY_NODE_`, described as a "silent break".

  — [v0.39.0 release notes](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)
- The v0.38.0 notes describe liblogosdelivery as a "Major new FFI API". The REST Store API defaults to page size 20, max 100. "Lightpush no longer mounts without relay enabled" — [v0.38.0 release](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.38.0)
- Repo README: logos-delivery is "a Nim implementation of libp2p protocols for private communication" plus a C library and a CLI node. You build with `make logosdeliverynode`. It runs on Linux, macOS, WSL and Windows (MSYS2). "You need at least one bootstrap node to join the network." License Apache-2.0/MIT — [logos-delivery README](https://github.com/logos-messaging/logos-delivery)

**Other implementations and bindings** (GitHub org listing and API, checked 2026-10-24) — [github.com/logos-messaging](https://github.com/logos-messaging)
- `logos-delivery-go` (formerly go-waku): latest tag v0.10.3, last push 2026-10-01; the README still says "go-waku".
- `logos-delivery-js` (formerly js-waku): sdk-v0.0.37 and sds-v0.0.9 released 2026-02-03; last push 2026-05-21.
- `logos-delivery-rust-bindings`: "Rust wrapper over nwaku ffi"; only tag v0.5.0 (2023), but commits as of 2026-10-09.
- Also present: Python, Go and Dart bindings; `nim-sds` (SDS e2e reliability); `sds-go-bindings`; `nim-segmentation`; `logos-delivery-rest-api` (OpenAPI spec); `logos-delivery-simulator`.
- Archived: `logos-chat-nim` and the old `specs` repo.
- Logos Chat (`logos-messaging/logos-chat`) is a Rust library. All crates are at 0.1.0, it is "pre-1.0", "the protocol is not frozen", and the default transport is an embedded logos-delivery node over FFI — [logos-chat README](https://github.com/logos-messaging/logos-chat)
- The Vac roadmap weekly for 2026-08-24 reports Logos Delivery moving to nim-ffi v0.3.1-rc.0 (search snippet) — [roadmap.vac.dev 2026-08-24](https://roadmap.vac.dev/weeklies/2026-08-24)

### Inferences
- The project should target **logos-delivery v0.39.x** (the Nim node and `liblogosdelivery`). go-waku and js-waku look like legacy or maintenance codebases with infrequent releases. The Rust bindings' last tag predates the v0.39 C ABI break, so Rust users would likely have to build against current master.
- APIs and config flags changed significantly between v0.38 and v0.39 (binary name, env prefix, C ABI, rate-limit fields). Expect further churn before 1.0, so the project should pin exact versions.
- "Messaging API" plus "Reliable Channels" (SDS + segmentation + pluggable encryption) is the layer that most closely matches "send a Zcash transaction reliably to another basecamp". It is explicitly labelled developer preview.

### Gaps
- No formal maturity or stability statement (for example "production ready") was found for logos-delivery as a whole. Testnet v0.3 calls the testnet "under active development and may be reset".
- I could not confirm whether go-waku is formally deprecated or just in maintenance mode.

---

## 2. What does "Basecamp" mean in Logos vocabulary?

### Takeaway
**Logos Basecamp** is a specific Logos product: a desktop GUI "local-first launcher for peer-to-peer apps built on Logos". It runs the Logos Core runtime (liblogos) locally and loads modules (delivery/chat, storage, blockchain node, wallets). It does **not** mean a physical site or a class of network node. The project's use of "basecamp" for mesh nodes will collide with this product name and should be disambiguated in the report. Logos also has community "Circles" (local activist groups), which are unrelated to software nodes.

### Cited Findings
- "Logos Basecamp is a local-first launcher for peer-to-peer apps built on Logos". It lets users discover, install and run Logos modules and apps through a GUI. Platforms: Linux x86_64/aarch64 (Ubuntu 22.04+), macOS aarch64, Windows 10/11 x86_64, WSL2. Requirements: "4 GB RAM minimum (8 GB recommended)" and about 2 GB of disk. The page says it is "accurate for Testnet v0.3.1". Windows packages include `chat_ui`, `chat_module`, `delivery_module`, `storage_ui` and `storage_module`. The blockchain node and swap app run via the Linux AppImage under WSL2 — [Install Logos Basecamp](https://docs.logos.co/basecamp/install-logos-basecamp)
- Basecamp section pages: install, install/load a module, and "Swap ETH and LEZ tokens" (an atomic-swap PoC using Sepolia ETH and LEZ testnet tokens) — [docs.logos.co/basecamp](https://docs.logos.co/basecamp)
- Architecture: Logos is a "modular, microkernel-based technology stack". The runtime is the C++ library liblogos. Default modules are storage, messaging and blockchain. "Basecamp is a local-first launcher… [that] runs the runtime locally". The "Logos Node runs the same runtime without a UI". Basecamp includes a wallet, chat, file sharing and an explorer by default — [What is Logos](https://docs.logos.co/get-started/what-is-logos)
- Blog (5 May 2026, Corey Petty): Basecamp is "a fully local launcher for the Logos stack, running all modules on your device from a unified surface". Its principles are "Private by default. Modular. Complete user control." — [Logos blog: Basecamp](https://logos.co/media/article/logos-basecamp)
- Latest Basecamp release is 0.3.2 (2026-10-06) — [logos-basecamp releases](https://github.com/logos-co/logos-basecamp/releases)
- Testnet v0.3 (30 Sep 2026) added Monero and Ethereum wallets to Basecamp and Windows support, and replaced `logoscore`/`lgpm`/`lgpd` with the `logosctl` CLI. Zcash is not mentioned — [Logos Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live)
- The glossary defines Basecamp as above and `logoscore` as "A daemon that runs Logos modules from the command line". It has no entry for "Circles" — [glossary](https://docs.logos.co/get-started/glossary)
- "Logos Circles" are local community and activist groups (for example Aba, Abeokuta, Barcelona, Cape Town, Khartoum). They meet regularly on issues such as housing and digital autonomy, framed as the grassroots layer for "network states" built on the Logos stack. These sources are event and recruiting pages from the movement itself (secondary) — [Logos Circle Aba (Luma)](https://luma.com/tjebk7s7); [Circles regional lead listing](https://joinrise.co/logos/circles-regional-lead-part-time-1zq1)

### Inferences
- One could put Logos Basecamp (or the headless Logos Node / `logosctl`) on each physical basecamp to host `delivery_module`. The documented module config only covers the `logos.test`/`logos.dev` presets, which rely on Status-operated internet fleets. An isolated deployment would therefore need either undocumented overrides or the standalone `logosdeliverynode`/`liblogosdelivery`, which supports custom cluster IDs and entry nodes (see §3).
- Basecamp's 4–8 GB RAM GUI requirement is heavier than a bare delivery node (§6). For field hardware, the headless node is the better fit.

### Gaps
- No Logos source ties "Basecamp" to physical or off-grid deployments or to mesh networking.
- I did not verify whether `delivery_module`'s `messagingOverrides` accepts `cluster-id`/`entry-node` for a private network.

---

## 3. Can a Logos Messaging network run with ZERO internet? (discovery, transports, private network config, IP requirement, non-IP transports)

### Takeaway
**Yes, provided there is IP connectivity between nodes.** Logos Delivery is IP-only: TCP by default, QUIC/UDP (on by default since v0.39.0) and WebSocket. There is no BLE, LoRa or other non-IP transport, implemented or planned, in any source found. An isolated network works by doing the following:
1. Choose your own `--cluster-id` and `--num-shards-in-network` (or no preset at all).
2. Point nodes at each other with static `--entry-node` multiaddrs (`/ip4/...`), or bootstrap discv5/Kademlia from local nodes.
3. Disable RLN (the default when no preset is used) or plug in a custom RLN backend.
4. Avoid the built-in presets. They hard-code `dns4` hostnames of Status-operated internet fleets and, for TWN, require a Linea Sepolia RPC.

DNS discovery needs a resolvable DNS TXT record. No mDNS or local discovery was found.

### Cited Findings
**Transports**
- docs.logos.co: "By default, Logos Delivery nodes use TCP"; secure WebSocket is used in browsers; WebRTC, WebTransport and QUIC are listed as "research only". BLE, LoRa and non-IP transports are not mentioned — [docs.logos.co transports](https://docs.logos.co/messaging/concepts/transports). **This is contradicted by** the v0.39.0 release, which says "QUIC is now on by default (`--quic-support`), listening on UDP at the node's TCP port when `--quic-port` is not set", with a 3 s TCP fallback if UDP is unreachable — [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0); [configure-quic.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-quic.md)
- Default ports: TCP 60000, WebSocket 8000 — [configure.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure.md). Discv5 UDP 9000 and listen address 0.0.0.0 — [MESSAGING-API](https://lip.logos.co/messaging/application/raw/messaging-api.html). A port value of 0 auto-assigns — [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)
- The QUIC address is put in the ENR only once the node knows a reachable host: via `--ext-ip`, `--dns4-domain-name`, `--ext-multiaddr`, a NAT mapping, or discv5. "Behind a NAT without a mapping, the ENR advertises the bound ports and no host" — [configure-quic.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-quic.md)

**Peer discovery options**
- Mechanisms: static peers, DNS discovery (EIP-1459), Discv5, Peer Exchange. Target "at least 6 peers in the mesh" plus reserve peers — [docs.logos.co peer discovery](https://docs.logos.co/messaging/concepts/understand-peer-discovery/peer-discovery)
- Static peers: `--entry-node:<multiaddr>`, repeatable. The docs give an example connecting to local `/ip4/0.0.0.0/tcp/60002/p2p/<peerId>` peers. Discv5: `--discv5-discovery:true` with `--entry-node:<enr>` as bootstrap entries. All mechanisms can be combined — [connect.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/connect.md)
- DNS discovery: `--dns-discovery` and `--dns-discovery-url=enrtree://<key>@<fqdn>`, disabled by default. The URL points to a signed ENR tree at a DNS name — [configure-dns-disc.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-dns-disc.md)
- Peer exchange (34/WAKU2-PEER-EXCHANGE, draft): the responder returns ENRs from a cache it "SHOULD" fill via ambient discovery such as discv5. The protocol "SHOULD only be used if" discv5 is infeasible — [34/PEER-EXCHANGE](https://lip.logos.co/messaging/core/draft/34/peer-exchange.html)
- Kademlia service discovery: `--enable-kad-discovery`, `--kad-bootstrap-node` (full multiaddrs), and `--plugin-kad-discovery` (hosted by a plugin). There is also an "external service-discovery backend over a C plugin ABI" — [messaging_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/messaging_conf.nim); [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)
- Rendezvous is enabled in Core mode (`rendezvous: true`); Edge mode enables only peer exchange — [modes.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/modes.nim)
- A search of the logos-delivery source tree (2026-10-24) found no mDNS module. The tree listing has no path containing "mdns" — [repo tree](https://github.com/logos-messaging/logos-delivery)

**Private network / own fleet configuration**
- Run-messaging guide: "Set a network preset (`--preset=twn`, `logos.dev`, `logos.test`, `status.prod` or `status.staging`), or set `--cluster-id` and `--num-shards-in-network`" — [run-messaging.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-messaging.md)
- Built-in presets (code):
  - `twn`: cluster 1, 8 shards, RLN on.
  - `logos.dev`: cluster 3, 8 shards, RLN off, Kademlia on, mix on.
  - `logos.test`: cluster 2, 150KiB, RLN off, mix on.
  - `status.prod`/`status.staging`: cluster 16, shard 32, 1024KiB, RLN off.

  All of them list entry nodes and mix nodes as `/dns4/...status.im` hostnames or `enrtree://...@...status.im`. `MaxAllowedSubscribedShard = 60` — [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim)
- With no preset, the default is "Auto-sharding on one shard, a 150 KiB max message size, and no RLN validation" — [MESSAGING-API](https://lip.logos.co/messaging/application/raw/messaging-api.html)
- Config keys: `cluster-id`, `num-shards-in-network`, `autosharding-shards` (explicit shard IDs), `listen-address`, `nat`, `max-msg-size`, `entry-node`, `discv5-discovery`, `nodekey` (stable peer ID), `storenode`, and the store retention options — [messaging_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/messaging_conf.nim). Config sources, in precedence order: CLI > env (`LOGOS_DELIVERY_NODE_*`) > TOML file (`--config-file`) > defaults — [configure.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure.md)
- `max-pure-libp2p-peers` defaults to 20 without a preset. Unrecognized config options are errors — [v0.39.1](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.1); [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)

**Internet-dependent pieces**
- The TWN preset's RLN uses contract `0xB9cd878C90E49F797B4431fBF4fb333108CB90e6` on chain ID 59141 (Linea Sepolia). Validation needs `--rln-relay-eth-client-address` (an HTTP Ethereum RPC) — [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim); [run-with-rln.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-with-rln.md)
- Mix nodes for logos.dev/logos.test are Status-hosted `dns4` addresses, resolved in the background — [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim)

### Inferences
- **The minimum viable offline setup** is `logosdeliverynode` on every basecamp:
  - same custom `--cluster-id` (any value not used by public networks, e.g. not 1/2/3/16);
  - `--num-shards-in-network=1`;
  - static `--entry-node=/ip4/<lan-ip>/tcp/60000/p2p/<peerId>` lists, using `--nodekey` so peer IDs are stable;
  - optionally `--discv5-discovery` bootstrapped with local ENRs;
  - `--store=true` on one or more always-on nodes;
  - `--quic-support=false`, or open UDP, to avoid the 3 s QUIC fallback penalty on lossy links.
- **DNS discovery offline** would need a local authoritative DNS server serving the signed `enrtree` TXT records. This is feasible but adds a component and is not needed.
- **Physical layer:** any link that carries IP works, e.g. Wi-Fi mesh (802.11s/batman-adv/OLSR), Ethernet, or point-to-point Wi-Fi. LoRa/Meshtastic-class links (hundreds of bytes per packet, very low throughput) are a poor match: libp2p handshakes, GossipSub control traffic and default 150 KiB-capable messages all assume a reasonably fast IP link. A non-IP hop would need a custom bridge that ferries opaque payloads outside Logos Delivery.
- GossipSub mesh targets (about 6 peers) assume a reasonably connected network. A sparse chain of basecamps will run with degraded GossipSub health but still forward. This is not verified empirically.

### Gaps
- No official guide for a fully air-gapped or private fleet was found. The private setup above is assembled from config docs and code.
- The default `--nat` value and the best offline value (`none` vs `any`) were not verified.
- No statement was found about planned BLE, LoRa, Wi-Fi Direct or any non-IP transport. The docs list only WebRTC, WebTransport and QUIC as "researched".

---

## 4. Delay tolerance and store-and-forward

### Takeaway
Logos Delivery is **not a delay-tolerant network (DTN)**. It is a real-time pub-sub layer with bounded catch-up mechanisms:
- **Store nodes** keep messages for 2 days by default; this is configurable by time or size.
- **Store Sync** reconciles store nodes over a default 1-hour window. The spec says it is designed for outages of ≤1 h and ≤20 % loss.
- **Store catch-up/backfill** fetches missed messages after a restart.
- **SDS (Scalable Data Sync)** adds end-to-end acks, retransmission (default 5 retries, 5 s ack timeout), causal ordering and dedup inside "reliable channels".

The default maximum message size is **150 KiB**, configurable via `--max-msg-size` (Status runs 1024 KiB). Messages flagged `ephemeral` are never stored. For basecamps that are disconnected for days, retention and sync windows must be raised deliberately, and long partitions are outside the design envelope.

### Cited Findings
**Store protocol and retention**
- Store is disabled by default and enabled with `--store=true`. A node can be a store client without serving, via `--storenode=<multiaddr>` — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md)
- Retention policies (`--store-message-retention-policy`), which can be combined with `;`:
  - `time:<seconds>`;
  - `size:<n>GB/MB`;
  - `capacity:<count>` (deprecated).

  "By default … a time retention policy set to `172800` seconds (2 days)", persisted in SQLite between restarts. `--store-message-db-url=sqlite:///path` or `""` for in-memory — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md)
- The same doc calls SQLite "the only database engine supported", **but** it also describes Postgres partition behaviour, and v0.39.x release notes fix Postgres archive issues. Postgres is in use for fleets, so the "only SQLite" sentence looks stale — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md); [v0.39.2](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.2)
- Sizing: "Most Waku messages average a size of 1KB - 2KB… ~250MB for… 100k messages… the allowable maximum size for Waku messages is up to 1MB" — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md)

**Store Sync**
- Flags: `--store-sync=true`, `--store-sync-range` (default `3600` s), `--store-sync-relay-jitter` (default `20` s), `--store-sync-interval`. If `time` retention is shorter than range plus jitter, the range is reduced automatically. On Postgres, a `--store-sync-range` above about 2 h "is not covered" — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md); [configure.md flag groups](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure.md)
- WAKU-STORE-SYNC spec (raw, 2026-06-16):
  - sync at least every 5 min with a random peer;
  - recommended window of 1 h;
  - optional Store Resume after offline periods (recommended max 6 h).

  It is "designed for short-term offline periods (no more than 1 hour) and occasional losses (no more than 20%)" and "not optimised or designed for catastrophic loss recovery" — [WAKU-STORE-SYNC](https://lip.logos.co/messaging/core/raw/store-sync.html)

**Messaging-layer reliability**
- `--reliability` ("Confirm each send with a Store node") is on by default, except in the twn/status presets. The `sent` event means a Store node has the message. "A Store node does not confirm its own sends, so give each sender another Store node as a peer. Without a Store peer, each send gets an `error` event about a minute after it goes out. Without Store nodes, set `--reliability=false`" — [run-messaging.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-messaging.md)
- Messaging client knobs and defaults — [messaging_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/messaging_conf.nim):
  - `backfill-enabled`: Store catch-up after a start, default true;
  - `backfill-request-timeout-seconds`: default 10, range 1–300;
  - `max-parked-age-sec`: default 1800, the max age of a send waiting for rate-limit budget;
  - `send-queue-capacity`: default 1000;
  - `local-storage-path`.
- WAKU-P2P-RELIABILITY (raw):
  - store-based publish confirmation, with an outgoing buffer keyed by message hash and periodic presence queries/retransmits;
  - periodic store queries to detect missed messages;
  - lightpush to 2 service nodes, filter subscriptions to ≥2 nodes with 1-min pings.

  "No p2p strategy guarantees e2e delivery" — [P2P-RELIABILITY](https://lip.logos.co/messaging/application/raw/p2p-reliability.html)

**SDS end-to-end reliability, and reliable channels**
- SDS (raw, last revised 2026-05-11): each message carries a Lamport timestamp, a causal history (recommended 2 prior message IDs, with optional retrieval hints) and a bloom filter of received IDs.
  - Unacknowledged messages are periodically rebroadcast.
  - Messages with unmet dependencies are buffered and fetched from Store or peers, then eventually marked "irretrievably lost".
  - Ordering is by Lamport timestamp, with ties broken by message ID. Dedup by ID is "MAY".
  - Optional SDS-Repair: T_min ≥30 s, T_max 120–600 s.
  - Ephemeral SDS messages bypass buffering.

  — [SDS spec](https://lip.logos.co/anoncomms/raw/sds.html)
- Reliable Channel pipeline: "Outgoing pipeline: Segment -> SDS -> Encrypt -> Dispatch; Incoming: Decrypt -> SDS -> Reassemble -> Emit" — [reliable_channel.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/channels/reliable_channel.nim)
- Channel knobs — [channels_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/channels_conf.nim):
  - `segmentationSegmentSizeBytes`;
  - `segmentationParityRate` (Reed-Solomon parity fraction);
  - `segmentationMaxTotalSegments`;
  - `segmentationReconstructionTimeoutSeconds`;
  - `sdsAcknowledgementTimeoutMs`;
  - `sdsMaxRetransmissions`;
  - `sdsCausalHistorySize`.

  Documented defaults: `sdsAcknowledgementTimeoutMs` 5000, `sdsMaxRetransmissions` 5, `sdsCausalHistorySize` 2 — [delivery module docs](https://docs.logos.co/messaging/delivery/use-logos-delivery-module-api-from-app)
- Testnet v0.3: "Delivery can retrieve messages missed while offline from store nodes"; Chat message history persists across sessions — [Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live)

**Message size and ephemeral flag**
- `DefaultMaxWakuMessageSizeStr = "150KiB"` ("1 MiB is the PubSub default"), plus a 64 KiB protocol-overhead safety buffer — [default_values.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/waku_core/message/default_values.nim). Configurable via `max-msg-size` (e.g. "150 KiB") — [messaging_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/messaging_conf.nim). The status.prod preset uses "1024KiB" — [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim)
- 14/WAKU2-MESSAGE messages "can now be marked as `ephemeral` to prevent them from being stored". Ephemeral messages are also filtered out of Waku Sync — [CHANGELOG](https://github.com/logos-messaging/logos-delivery/blob/master/CHANGELOG.md)
- Service rate limits on service nodes: defaults `lightpush:5/1s`, `filter:100/1s`, `px:5/1s`, adjustable via `--rate-limit` — [run-messaging.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-messaging.md)

### Inferences
- A Zcash shielded transaction (typically a few KB to tens of KB) fits well within the 150 KiB default. Segmentation is only needed for unusual payloads such as batches or compact-block bundles.
- For intermittently connected basecamps (hours to days offline), a workable pattern is:
  - always-on "hub" store nodes with `time:` retention ≥ the longest expected partition (e.g. `time:1209600` = 14 days);
  - `--store-sync` between hubs, with a range raised (on SQLite) beyond 1 h when outages are longer;
  - application-level SDS/reliable channels or app-level acks plus idempotent re-send, since Zcash txs are naturally idempotent by txid.

  Store Sync and Resume are tuned for ≤1–6 h gaps, so multi-day DTN behaviour relies mostly on long Store retention plus app-level retry. This is untested.
- Never mark transaction messages `ephemeral` if store-and-forward is wanted.
- Because `sent` requires a *different* Store node to confirm, a two-node basecamp pair needs at least one third store-capable peer, or `--reliability=false` plus app-level acks.

### Gaps
- The default value of `--store-sync-interval` was not found.
- No published measurements of Delivery over high-latency, lossy or partitioned links were found.
- The default segment size and parity rate were not found.

---

## 5. Security and privacy properties

### Takeaway
- **Transport:** node-to-node links use libp2p Noise.
- **Payloads:** unencrypted unless the application encrypts them. Available options: 26/WAKU2-PAYLOAD "version 1" (AES-256-GCM symmetric or ECIES-secp256k1 asymmetric, no forward secrecy); Noise handshakes/sessions (raw specs); app-supplied AEAD per reliable channel (v0.39.0); and Logos Chat (Rust, 0.1.0, unaudited; de-MLS for groups is in development).
- **Relay:** uses StrictNoSign (no `from`/`seqno`/`signature`/`key`), giving pseudonymity with k-anonymity over shared pubsub topics. Direct peers, Store, Filter and LightPush servers can still link PeerID/IP to topics.
- **RLN:** spam protection is on-chain. TWN uses a Linea Sepolia contract. The Logos testnet is moving to an LEZ registry. The stock code has no shipped offline or static membership mode, but RLN can be disabled or replaced via a C-ABI plugin.
- **Mix:** sender anonymity (Sphinx, ≥3 hops, exponential per-hop delays) for LightPush and Store only. The code needs ≥4 mix nodes and the spec recommends ≥100, so in a small isolated mesh the anonymity set is tiny.

### Cited Findings
**Encryption**
- "Node-to-node connections use libp2p noise encryption. The network applies no default encryption to message payloads" — [docs.logos.co FAQ](https://docs.logos.co/messaging/get-started/faq)
- 26/WAKU2-PAYLOAD (draft): symmetric AES-256-GCM (16-byte tag, 12-byte IV); asymmetric ECIES on secp256k1 (Concat KDF SHA-256, HMAC-SHA-256, AES-128-CTR); an optional ECDSA signature inside the ciphertext; padding to multiples of 256 bytes. "Forward secrecy is not provided at this layer" — [26/WAKU2-PAYLOAD](https://lip.logos.co/messaging/application/draft/26/payload.html)
- docs.logos.co security page: confidentiality via "Waku Message (version 1) and Noise", which provide payload encryption and encrypted signatures — [security features](https://docs.logos.co/messaging/concepts/security-features)
- Reliable-channel encryption: "Pluggable per-channel encryption… No scheme is mandated." It is applied per segment to the whole SDS message "so its routing metadata never reaches the wire". The cipher "must be authenticated". A failure "fails the message -- never a fallback to plaintext"; without a cipher the channel is plaintext — [channel_encryption.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/channels/encryption/channel_encryption.nim)
- The delivery_module v0.2.1 docs warn: "Reliable channel payloads are not encrypted. Use the Chat module for end-to-end encryption" — [delivery module docs](https://docs.logos.co/messaging/delivery/use-logos-delivery-module-api-from-app)
- Logos Chat: 1:1 and group E2EE, "using de-MLS for groups" — [docs.logos.co/messaging](https://docs.logos.co/messaging). What-is-Logos says group support is "in development using de-MLS" — [What is Logos](https://docs.logos.co/get-started/what-is-logos). The logos-chat README lists HKDF, XEdDSA, MLS-style keypackages and an SQLCipher local DB, with no audit mentioned — [logos-chat](https://github.com/logos-messaging/logos-chat). Testnet v0.3: "accounts do not yet persist across different Chat sessions" — [Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live)

**Relay anonymity and metadata**
- 11/WAKU2-RELAY (stable), protocol `/vac/waku/relay/2.0.0`:
  - Messages MUST use `StrictNoSign`; `from`, `seqno`, `signature` and `key` "MUST be _absent_".
  - "removing identifiable information from messages cannot lead to perfect unlinkability."
  - A single pubsub topic gives k-anonymity.
  - A publisher's direct connections may identify its messages by traffic analysis.
  - The adversary model excludes a global observer.

  — [11/WAKU2-RELAY](https://lip.logos.co/messaging/core/stable/11/relay.html)
- PeerIDs give pseudonymity, which "does not provide complete anonymity". In Store, "the queried node can associate the querying node's PeerID with the topics it requests", and Filter is similar — [security features](https://docs.logos.co/messaging/concepts/security-features)
- Content topic scoping is a privacy trade-off: shared topics increase k-anonymity — [docs.logos.co/messaging](https://docs.logos.co/messaging)
- Peer exchange risks: a malicious responder can return attacker-controlled peers (eclipse or neighbourhood surveillance). There is also amplification DoS, and signed ENRs "could still help track peers" — [34/PEER-EXCHANGE](https://lip.logos.co/messaging/core/draft/34/peer-exchange.html)

**RLN (Rate Limiting Nullifiers)**
- The docs.logos.co security page calls RLN "under research", **which is stale**. RLN is implemented and deployed: the TWN preset sets `rlnRelay: true`, contract `0xB9cd878C90E49F797B4431fBF4fb333108CB90e6`, chain ID 59141, `rlnEpochSizeSec: 600`, `rlnRelayUserMessageLimit: 100` — [networks_config.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/factory/networks_config.nim)
- Classic flags: `--rln-relay`, `--rln-relay-dynamic` ("connect to an ethereum node to fetch the membership group"), `--rln-relay-eth-contract-address`, `--rln-relay-eth-client-address` (a Linea Sepolia RPC) — [run-with-rln.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/run-with-rln.md)
- v0.39.0 changes:
  - "pluggable RLN API module";
  - proof generation moved to the client;
  - "RLN proof validation that can be disabled to phase RLN in" (`rln-disable-validation`);
  - `DISABLE_RLN` builds the library without zerokit.

  — [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0); [messaging_conf.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/messaging_conf.nim)
- The plugin surface is "Backend-agnostic". The LEZ backend "never names a membership or a registry, never configures the backend and never starts it — the host owns all of that". It is selected when the host installs a plugin via `logosdelivery_rln_set_plugin` — [rln_plugin.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/rln/rln_plugin.nim); [rln_lez.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/rln/rln_lez/rln_lez.nim)
- The current source tree contains only `rln_evm` (Ethereum contract) and `rln_lez` RLN backends. The older nwaku "static group manager" (hard-coded memberships) appears only in historical CHANGELOG entries — [repo tree](https://github.com/logos-messaging/logos-delivery); [CHANGELOG](https://github.com/logos-messaging/logos-delivery/blob/master/CHANGELOG.md)
- Logos testnet RLN:
  - The RLN modules register the node on the LEZ registry once a payer account is funded (250,000,000 native LEZ units), and "Testnet has no faucet" — [logos-docs issue #519](https://github.com/logos-co/logos-docs/issues/519).
  - Minimum versions are delivery_module ≥0.3.3, liblogos_rln_module ≥0.12.0, and liblogos_lez_rln_module ≥5.1.0 — [logos-docs PR #571](https://github.com/logos-co/logos-docs/pull/571).
  - Gifted memberships (LIP-158): "A funded gifter node registers RLN memberships on-chain on behalf of authenticated clients" — [logos-rln-gifter](https://github.com/logos-co/logos-rln-gifter).
  - Testnet v0.3: RLN validation turns on about two weeks after the 30 Sep 2026 release — [Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live).
- RLN FURPS: memberships "registered and accounted for on Logos Blockchain"; slashing by recovering the secret key and burning the deposit. Offline or static membership is not specified — [roadmap.logos.co RLN](https://roadmap.logos.co/anoncomms/furps/rln)

**Mix (sender anonymity)**
- WAKU-MIX (raw, 2026-06-16): integrates libp2p mix so senders publish (LightPush) or query (Store) "without revealing their peer ID or IP address". Filter is out of scope and Relay-over-mix is future work. Replies use SURBs. Mix nodes advertise via an ENR bit plus a `mix-key`. The spec recommends a pool of ≥100 mix nodes and says spam protection is "future work" — [WAKU-MIX](https://lip.logos.co/messaging/core/raw/mix.html)
- libp2p MIX (raw, 2026-08-24):
  - Sphinx: Curve25519, AES-128-CTR, LIONESS; path "MUST consist of at least three hops".
  - Per-hop delays sampled from an exponential distribution.
  - Optional loop cover traffic.
  - Fixed-size packets; the example totals 4608 bytes with a 3984-byte payload field; larger messages must be fragmented by the origin protocol.
  - No QoS guarantees; "Latency is high by design".

  — [libp2p MIX](https://lip.logos.co/anoncomms/raw/mix.html)
- Code:
  - `MinMixPoolSize = 4`.
  - Default per-hop `ExponentialDelayStrategy(meanDelay = 50)` (ms), or a spam-protection delay strategy.
  - `anonymity-level`: `None` (plain relay/lightpush), `Preferred` (falls back to plain on failure or too-large messages), `Required` (Mix only; "A message that does not fit in one Mix message fails at once").

  — [waku_mix/protocol.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/waku/waku_mix/protocol.nim); [modes.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/api/conf/modes.nim); [mix_processor.nim](https://github.com/logos-messaging/logos-delivery/blob/master/logos_delivery/messaging/delivery_service/send_service/mix_processor.nim)
- Testnet v0.3: Storage downloads over Mix with RLN spam protection, but "only the downloader is protected". Mix is usable via the Messaging API, and operators run mix nodes through the Mix module — [Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live)

### Inferences
- **For Zcash payloads,** the Zcash shielded tx is already encrypted to the recipient. Logos-layer encryption mainly hides metadata (that a tx was sent, its size and its txid) from mesh relays and store nodes. Use a reliable channel with an authenticated cipher (e.g. a pre-shared per-basecamp-pair key, or ECIES v1) so relays and stores never see raw tx bytes or txids that could later be correlated with the chain.
- **Metadata exposure in an isolated mesh:**
  - every relay peer sees the content topic, timing and size;
  - store nodes see everything stored and who queried which topics;
  - IPs are LAN/mesh addresses, which may map to physical basecamps.

  Use a single shared content topic across all basecamps, pad payloads to fixed sizes, and run only Core (relay) nodes, avoiding Filter/LightPush clients to reduce linkability.
- **RLN offline:** the shipped backends need an Ethereum RPC (`rln_evm`) or a Logos/LEZ host (`rln_lez`). A fully offline deployment would either (a) run with RLN off and rely on GossipSub peer scoring plus `--rate-limit` service limits in a closed, permissioned mesh, or (b) implement a custom RLN plugin with a static, pre-distributed membership tree over the `logosdelivery_rln_set_plugin` C ABI. Option (b) is feasible in principle but not shipped.
- **Mix offline:** with the ≥4 mix nodes the code requires, a small basecamp mesh can technically run Mix. However, the anonymity set is limited to a handful of known, possibly physically identifiable nodes, and a 4608-byte Sphinx packet (under 4 KB usable) may not fit a full Zcash transaction. With `Required`, oversized messages "fail at once"; with `Preferred`, they fall back to the plain path. Mix is therefore of limited value here.
- In a closed mesh where every node is an operator-controlled basecamp, a malicious insider is a bigger risk than external spam. Static peering plus no peer exchange limits eclipse attacks.

### Gaps
- The Noise handshake specs (WAKU2-NOISE, NOISE-SESSIONS) and de-MLS were not reviewed in depth, and it is unclear whether logos-delivery ships Noise payload sessions today.
- No third-party security audit of logos-delivery v0.39 or Logos Chat was found.
- No formal anonymity analysis for Waku Mix with small node counts was found.
- Payment Streams (an AnonComms raw LIP) was not read.

---

## 6. Resource requirements (Raspberry Pi / phone / light mode)

### Takeaway
A relay-only node is light: about 0.5 GB RAM, with 2 GB recommended, especially with WSS. Building from source needs ≥2 GB RAM. Linux/macOS ARM64 is supported, and liblogosdelivery builds for Android and iOS. **Edge** mode (light client: LightPush/Filter/Store client, no relay) exists for mobile and browsers. No official Raspberry Pi statement was found, but aarch64 Linux support makes a Pi 4/5 with 2–4 GB plausible. Logos Basecamp (GUI) is heavier: 4 GB minimum, 8 GB recommended.

### Cited Findings
- "We recommend at least 2GB of RAM, especially if WSS is enabled. For a Relay-only node, 0.5GB of RAM is sufficient" — [docs.waku.org run-node (source)](https://github.com/logos-messaging/docs.waku.org/blob/develop/docs/run-node/index.md)
- "To compile the nwaku binary, a minimum of 2GB of RAM is required". A `s-1vcpu-2gb` droplet "is more than capable" — [droplet-quickstart.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/droplet-quickstart.md)
- Store sizing: about 250 MB for 100k messages of 1–2 KB — [configure-store.md](https://github.com/logos-messaging/logos-delivery/blob/master/docs/operators/how-to/configure-store.md)
- ARM64 support for Linux/macOS was added in nwaku (historical) — [CHANGELOG](https://github.com/logos-messaging/logos-delivery/blob/master/CHANGELOG.md). "Android and iOS builds are restored" in v0.39.0. There is also a Windows cross-compile — [v0.39.0](https://github.com/logos-messaging/logos-delivery/releases/tag/v0.39.0)
- Edge mode is "for resource-restricted environments such as mobile and browsers" — [MESSAGING-API](https://lip.logos.co/messaging/application/raw/messaging-api.html). The glossary defines a light node as "a resource-limited device or client that leverages service nodes" — [glossary](https://docs.logos.co/get-started/glossary)
- delivery_module apps need Linux/macOS on aarch64/x86_64 and "About 1 GB of RAM" — [delivery module docs](https://docs.logos.co/messaging/delivery/use-logos-delivery-module-api-from-app)
- Basecamp needs 4 GB RAM minimum (8 GB recommended) and about 2 GB of disk — [Install Basecamp](https://docs.logos.co/basecamp/install-logos-basecamp)

### Inferences
- A Raspberry Pi 4/5 (≥2 GB) running headless `logosdeliverynode` in Core mode, with SQLite store and RLN off, is a reasonable basecamp node. Cross-compile or use a prebuilt binary rather than building on-device if RAM is tight.
- Phones should run Edge mode attached to a basecamp Core node. Edge mode leaks the phone's PeerID and topics to that service node (§5), which is acceptable if the phone and node belong to the same operator.

### Gaps
- No CPU, bandwidth or power benchmarks for ARM SBCs were found.
- No official Raspberry Pi support statement or prebuilt ARM binaries were confirmed for v0.39.x.
- RLN proof-generation cost on mobile was not found.

---

## 7. Logos goals on mesh / offline / censorship resistance / payments; relevance of the broader stack (Logos Blockchain, Logos Storage)

### Takeaway
Logos emphasises structural privacy, sovereignty, censorship resistance and "local-first" apps. However, **no Logos source describes mesh networking, non-IP links or internet-free operation as a goal or feature.** Testnet v0.3 (30 Sep 2026) has no mesh features. Logos Blockchain (formerly Nomos; Cryptarchia PoS, Blend Network, LEZ) and Logos Storage (formerly Codex) are testnet-stage, need their own networks to reach consensus or retrieve data, and support no Zcash. Their only plausible offline role is indirect, e.g. Logos Storage as a content-addressed blob store on a LAN, which is untested. Settlement must happen on Zcash itself when a basecamp regains connectivity to the Zcash network.

### Cited Findings
- Design goals: "Privacy is implemented at the infrastructure layer"; sovereignty ("Nodes do not depend on third-party servers"); messaging provides "private, censorship-resistant communication". Blockchain: Cryptarchia private PoS plus Blend Network, with LEZ as an L2 with public and private accounts. Storage: content-addressed by CID — [What is Logos](https://docs.logos.co/get-started/what-is-logos)
- Testnet v0.3 (30 Sep 2026):
  - PoW onboarding ("removes centralised, traceable on-ramps");
  - decentralised LEZ sequencing;
  - downloads over Mix;
  - Logos SQL;
  - Monero and Ethereum wallets;
  - "testnet… may be reset".

  No mainnet date is given and Zcash is not mentioned — [Testnet v0.3](https://logos.co/media/article/logos-testnet-v03-live)
- Logos Storage is "the privacy-preserving filesharing component"; Logos Blockchain is "the foundational infrastructure layer" — [glossary](https://docs.logos.co/get-started/glossary)
- The FAQ recommends Logos Storage for long-term storage, since Store availability "isn't guaranteed" — [FAQ](https://docs.logos.co/messaging/get-started/faq)
- Logos Circles and network-state framing are positioned as the "technical foundation for cyber states, parallel societies, network states" (movement recruiting material, secondary) — [Circles listing](https://joinrise.co/logos/circles-regional-lead-part-time-1zq1)
- An AnonComms "Payment Streams" raw LIP exists — [lip.logos.co](https://lip.logos.co/)

### Inferences
- For this project, the useful Logos pieces are **Logos Delivery** (transport, Store, SDS reliable channels, optional pluggable encryption) and possibly **Mix** in larger meshes. The Logos Blockchain, LEZ and RLN-on-LEZ components assume connectivity to the Logos network and add internet dependencies, so they should be excluded or disabled for an internet-free deployment.
- Because "Basecamp" is a Logos product name and Logos already runs a testnet with wallets (Monero/ETH), readers may assume Logos provides a Zcash wallet or offline payment features. It does not; any Zcash integration would be custom.

### Gaps
- I did not read the Logos forum, so community discussion of mesh or offline use may exist there.
- The Payment Streams LIP was not reviewed for relevance to offline payment relaying.
- No mainnet timeline was found for Logos Blockchain, Storage or Messaging.
