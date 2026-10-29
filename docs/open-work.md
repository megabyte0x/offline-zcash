# Open work

Ordered. Each item names the component doc that owns it. Update this list as items close.

**Current release: v0.7.7 (29 October 2026).** The evidence below stays attached
to the version/platform that produced it. Design decisions are not shipped
controls; the application still trusts gateway chain reports. [Current flow](how-it-works.md).

## Built

v0.1–0.6 were exercised on the local `ths` NU7 chain. v0.7 adds public-testnet light-client operation; v0.7.5 was exercised with owner-funded TAZ in Linux Basecamp. Releases currently ship only the macOS and Linux Basecamp packages (four `.lgx` per platform plus merged catalogue packages and checksums); CLI tarballs and the Android APK are not released for now. **Current focus: the Basecamp apps on macOS and Linux.** Android is not the current focus; its version tracks releases and CI builds the APK when `android/` or `crates/ozm-ffi` change, but no Android work is planned for now. Releases are on GitHub.

**Funded Linux verification, 28 October:** Basecamp 0.3.2, Ubuntu 24.04/Xvfb,
one invited gateway and three invite-free wallets with separate keys/state.
The 0.7.4 control mined 100,000 zat; paying lagging Bob 130,000 zat automatically
refused after one 200-block batch. Its original transaction
`efc4aba879d2a20623cf784b01f2bf7a4ce350cb6534f2006c92ea09dd8af36a`
never left the gateway. The fix submitted the unchanged bytes with both payees
stopped; it mined at 4,551,323 and Bob scanned the credit after restart.
Gateway sends now submit locally. Catching-up recipients durably defer checking,
and exact already-scanned transactions reconcile after root pruning.

Alice's 20,007-zat offline payment survived restart and gateway reconnect;
Bob returned Max (140,007 zat, exactly 10,000 fee). Alice's automatic eight-note
split `1fefac6b137d62fae181e996525d9dbf05bce8208d55fc9e814296c091228e9c`
cost 45,000 zat, while older funds stayed usable. After it mined, a selected
split note funded 100,003 zat to Bob with the gateway stopped. Both wallets
later reported ten confirmations for
`04b18100e9b792ae57eb11337d64090bdfd5e47ab4c18e1d67a86d256a800266`.
The lagging Carol wallet passed the formerly stalled 272-chunk range without
clearing its mesh store, then caught up to 4,551,449 with both incoming payments
Mined, 90,010 zat spendable and no refusal. Across all four wallets, nine unique
transactions cost 125,000 zat: 9,875,000 of the funded 10,000,000 zat remained,
with zero locks or pending checks. The original refused transaction's SHA-256
still matched the 0.7.4 bytes. All devices ran the same tested portable engine.
The simple wallet, Debug toggle, copied address and 800px stacked layout were
rendered and inspected; scanned-only credits appear without a fabricated receipt.
Strict Clippy and all 57 workspace tests pass; real Regtest mesh/LAN/open benches
also passed. The [testnet skill](../.agents/skills/testing-basecamp-testnet/SKILL.md)
records setup, capture and sanitized snapshots for repeat runs.
These are shared-orb TCP links, not physical Wi-Fi/radio or OS-isolated internet.
One public TLS source was reachable; inclusion and confirmations remain
gateway-reported. macOS/phone/radio and independent chain proofs remain open.

**v0.7.5, item 1:** automatic per-device scan starts and background sync
replace birthday setup and a required manual sync. On 28 October, the real
Basecamp 0.3.2 AppImage in Ubuntu 24.04/Xvfb reached testnet height 4,550,718
1.072 seconds after the gateway link came up; the capture 4.2 seconds after
clicking Start already showed ready and the receive address. No invite, dial,
birthday input or refresh was used. CLI link-to-tip was 0.845 seconds. These
are fresh empty-wallet measurements, not funded-payment or macOS evidence.
Offline/delayed-contact interpolation, floor/clamp boundaries and reset
preservation are covered by the wallet test.

**Item 2:** downloads no longer hold the payment/apply lock; Pay, Max and Prepare
stay available during sync. A stalled-download regression failed after ten seconds
before the fix and passed in 0.51 seconds afterwards, including the durable reservation.
Real Linux Basecamp sent 0.0137 Regtest ZEC while at height 109 of 709; proving
took 207 ms and the gateway reported mined 269 ms after the build event, while
the wallet was still behind. RPC independently confirmed height 710. A 100 ms
per-block latency proxy made the 600-block catch-up observable; it did not fake
blocks or submission results. Screenshots/recording are in the run artifacts.
**Item 3:** mesh-decrypted notes persist as pending incoming, separate from
spendable notes. Real Basecamp received 0.0103 Regtest ZEC at height 710: the
durable incoming event followed mesh arrival by 138 ms (133 ms verifying).
Before mining the wallet showed 3.0137 spendable plus 0.0103 incoming; automatic
scan of block 711 changed it to 3.024 spendable with no pending output or duplicate.
The Gateway view showed the same pending value. The real web wallet separately
received 0.0019, showed zero spendable before mining and 0.0019 after scanning 712.
Core coverage checks serialization, commitment merge and exclusion from Max;
signed-notice integration coverage checks expired/conflict removal and that a
Mined report alone does not make an unscanned incoming note spendable.

**Item 4:** profile-first catch-up optimization. Against live testnet.zec.rocks,
the same 85,000 blocks (4,465,798–4,550,797) took 42.13 / 40.84 / 41.80 seconds
on v0.7.3 and 14.12 / 13.07 / 14.36 seconds on the candidate: **41.80 → 14.12 s
median** on this 8-core, 16-GiB orb. The baseline's median phase totals were
25.59 s roots/checkpoints, 11.36 s tree insertion, 3.65 s decryption, 0.20 s
install/persistence and 12.05 s fetch (overlapping application, not additive).
Parallel subtree construction/root calculation retains the same 200 checkpoints;
all retained roots, hashes, checkpoint positions, notes, nullifiers and reservations
match v0.7.3 in all three final runs. The bench-only builds cap the observed tip
for identical ranges and disable automatic startup only to time the explicit CLI
sync; production has neither override. Intermediate optimization runs took
14.66–21.39 s before removing a redundant predecessor lookup during prefetch.
One baseline invocation raced API startup and was discarded before any scan;
subsequent runs wait for HTTP readiness. Zaino was unavailable, so independent
public-secondary agreement is not claimed; a controlled gRPC test accepts the
matching root and refuses mismatched roots/hashes and a prefetched range on a
changed fork. The final portable module in real Linux Basecamp caught up 85,080
blocks to 4,550,877 automatically in 27.228 seconds, with the receive address
visible throughout (packaged/unbounded live-tip run, not the capped CLI bench).
macOS has not been run here.

**Item 5:** first-note metadata survives reset, with sparse 1,000-block hashes.
Synthetic 6,000-block forks test first note before/after the common sparse ancestor,
empty-wallet recovery, reseeding, replacement roots and reservation persistence.
The existing receipt test checks mesh anchor+1 then commitment merge. Real Linux
Basecamp recovered automatically from `ths reorg --depth 2`: height remained 712,
hash changed from `39e1ab…` to `9e18b4…`, and orphaned incoming value disappeared
(3.024 → 3.0137 ZEC), with `rescanned: true` and no Refresh click. The first-note
height stayed 107 and the device birthday stayed 5. A sparse match is a conservative
fork bound; old wallets without historical samples can only fall back to their
stored birthday.

**Item 6:** automatic preparation tries 8/4/2 smaller outputs at a ≤2% fee, with
half reserved for change. Real Basecamp started an eight-note split 1.102 seconds
after opening, without a Prepare click: 3 ZEC locked, a different 0.0137 ZEC note
stayed spendable. Mining was deliberately paused to inspect progress. After one
Regtest block (713), automatic scanning showed ten spendable notes totaling
3.01325 ZEC (45,000-zat fee), including 149,977,504-zat change. Advanced manual
controls expand/collapse normally. The contract test covers both sides of the
750,000-zat economic boundary, smaller/larger splits, no unmined selection,
failed witness suppression and restart. No public-testnet funds/faucet were used.
The LAN/open benches account for actual mined preparation fees rather than
assuming the funded balance stays fee-free.

**Earlier candidate integration:** rebased on v0.7.4, with package versions 0.7.5 and Android
code 14. Strict workspace Clippy and all 55 tests pass, including `ozm-ffi`.
The real Regtest mesh, LAN and invite-free benches pass (23, 11 and 25 checks).
All four portable LGX packages build sequentially; installing them in the real
Linux Basecamp AppImage preserves the wallets and resumes sync automatically.
macOS paths were checked in code, not executed; no Android UI/build or funded
public-testnet payment is claimed. Screenshots and interaction recordings are
under `.amp/in/artifacts/v074/` (the original artifact directory name).

**Security after readiness:** [G3/G6/L3 and remaining work](privacy-and-threats.md#v075-readiness-unresolved-security-work)
cover history-leaf/frontier proofs, the single reachable testnet operator,
birthday/range privacy and the automatic split fingerprint. None is implemented
or represented as solved by these UX changes.

**Next witness work:** immediate mesh receipt does not yet mean immediate spending.
The pending note has no canonical position/witness. A contained implementation
needs an authenticated single-block/frontier or Ironwood subtree-root path before
it can bypass catch-up; that is deferred rather than trusting a gateway's Mined
report as a spend witness. Normal scanning remains the spendability gate.

| Version | What it proves | Doc |
|---|---|---|
| v0.1.0 | Offline payer builds and proves a v6 Ironwood transaction; it hops A → relay → B → gateway over `ozm-mesh` (TTL flood, 512-byte frames, store-and-forward, acks); B checks it offline and returns a receipt; the gateway submits, mines and returns a signed result. `bench/mesh-bench.sh all` on four containers | [mvp-v0.1.0](mvp-v0.1.0.md) |
| v0.2.0 | Installable `ozm-node` (join, start, fund, pay; web UI on `127.0.0.1:7780`), fleet invites, UDP LAN discovery, wallet sync and test funds over the mesh. `bench/lan-bench.sh all` | [mvp-v0.2.0](mvp-v0.2.0.md), [install](install.md) |
| v0.3.0 | The same device as a Logos Basecamp core module and QML app; `.lgx` packages for linux-amd64, linux-arm64 and darwin-arm64; install guide tested on Basecamp 0.3.2 | [basecamp-install](basecamp-install.md), [../logos/README.md](../logos/README.md) |
| v0.4.0 | B1: attach byte streams with BLE timers; UniFFI/Kotlin device API; Android 10+ arm64 BLE + LAN test shell and connectedDevice foreground service; APK/checksum CI/release packaging. Rust chunk/stall/multi-hop/ack checks and host-JVM A–B–D payments through Kotlin links; **no phone/radio/UI validation** | [android-install](android-install.md), [mesh BLE](components/mesh.md#ble-link-android-first) |
| v0.4.1 | Linux `ozm-ble` BlueZ central + peripheral backend, CLI `--ble`, Basecamp `ble: true` / Setup toggle, BLE/TCP neighbour labels; bundled libdbus. Protocol tests only; **D1/D2/D5 real-hardware gates open**, macOS D3/D4 not implemented | [desktop BLE](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open), [install](install.md#bluetooth-on-linux) |
| v0.5.0 | One shared Basecamp engine plus three apps: Offline Zcash, Organiser (fleet/invite export and join), Gateway (RPC check, live RPC/mining settings, balance/submissions). Four `.lgx` packages × three platforms, CLI tarballs, Android 0.5.0/code 6. Invite roles remain authoritative; wire format unchanged | [basecamp-install](basecamp-install.md), [../logos/README.md](../logos/README.md) |
| v0.5.1 | Public custom Logos catalogue: install all four packages by name, merged portable variants, content-root verification and checksums; changelog-driven release notes. Android 0.5.1/code 7. Official-catalogue submission remains a draft | [basecamp-install](basecamp-install.md#install-by-name-recommended), [changelog](../CHANGELOG.md), [submission](logos-catalogue-submission.md) |
| v0.6.0 | Invite-free local identities, open LAN/BLE discovery, signed/sealed envelopes and destination-authenticated acks; nearest-first gateway selection then persistent pinning, proof-bearing/pinned address bindings and rate-limited funds. Address payments and exact builder-fee Max in Basecamp/web/Android. Open/unknown payers wait for inclusion, never pending acceptance. Loopback API token/Host checks and text-only web rendering. Six-device no-invite bench. Chain views remain gateway-reported | [basecamp-install](basecamp-install.md#open-mode-trust-and-faucet-limits), [install](install.md), [changelog](../CHANGELOG.md) |
| v0.7.0 | Public-testnet light-client gateway over TLS lightwalletd, fleet network/birthday, Common 4.0, compact sync, Prepare for offline and gateway-reported `included`/`mined` (details below). Android 0.7.0/code 9 | [install](install.md#public-testnet-v070), [chain](components/chain.md), [gateway](components/gateway.md) |
| v0.7.1 | UI-only release: the same payment-state labels and explanations in web, Basecamp and Android; clearer setup, pay, Prepare for offline, Organiser and Gateway screens; accessibility. Android 0.7.1/code 10 | [changelog](../CHANGELOG.md) |
| v0.7.2 | Contained hardening: Unix 0600 files, bounded mesh and gateway resources, key-possession proof before inventory, nonce-addressed gateway results (P12), invited pending caps, multi-server submission. Older v0.7.x peers only live-forward. Android 0.7.2/code 11 | [privacy-and-threats](privacy-and-threats.md#trust-assumptions-at-v070), [gateway](components/gateway.md#v072-contained-hardening) |
| v0.7.3 | Scope: Basecamp apps on macOS and Linux. Gateway is the only role: every device pays, receives and relays; Organiser has one gateway checkbox per device and requires at least one; invited devices trust the fleet's named gateway; legacy roles in old invites are accepted and ignored. LAN/BLE fleet tag and hint are zero (v0.7.2 peers still connect); Android `fleetHint()` removed. Leave session in all three Basecamp apps moves `ozm` to a timestamped backup (not available on Android or in the `ozm-node` CLI; use another `--home`). Regtest Gateway accepts only `http://` JSON-RPC; testnet Gateway shows the fleet's lightwalletd servers. Android 0.7.3/code 12 | [basecamp-install](basecamp-install.md#leave-session), [mesh](components/mesh.md), [privacy-and-threats](privacy-and-threats.md#trust-assumptions-at-v070), [changelog](../CHANGELOG.md) |
| v0.7.4 | Patch: a device's own delivery acks for received sync chunks no longer fill its mesh-store quota, so long catch-ups over the mesh and later payments keep working; Leave session keeps the invite-free wallet (the next invite-free start on the same network restores it); Basecamp upgrade troubleshooting. Releases ship only the macOS and Linux Basecamp packages. Android 0.7.4/code 13 (not released) | [mesh](components/mesh.md), [basecamp-install](basecamp-install.md#leave-session), [changelog](../CHANGELOG.md) |
| v0.7.5 | Automatic scan starts/sync, Pay during catch-up, durable incoming notes/verification, faster scans/deep-fork recovery, economical note preparation and simple/Debug wallet. Funded Linux evidence above. Android code 14 (not released) | [builder](components/offline-builder.md), [mesh](components/mesh.md), [changelog](../CHANGELOG.md) |
| v0.7.6 | Replaceable presence/tip notices and store migration; bounded events; deferred verification without live sender; owner-only folders. Gateway-only Organiser, used-wallet backup selection and explicit fresh-wallet control. Android code 15 (not released) | [mesh](components/mesh.md#v076-replaceable-notices-and-upgrade-migration), [recovery](basecamp-install.md#leave-session), [changelog](../CHANGELOG.md) |
| v0.7.7 | Chain checks and server connections have deadlines; mesh submissions wait for a busy check; duplicate in-flight requests join; stop repeatedly polling mined entries. Android code 16 (not released) | [gateway](components/gateway.md#v077-busy-gateway-fix), [changelog](../CHANGELOG.md) |

**v0.7.0:** configurable TLS lightwalletd gateway (no full node/viewing-key scanning),
fleet network/birthday, Common 4.0, compact Ironwood chunks and empty runs,
persisted ShardTree with 200 checkpoints and reorg rollback. Gateway pushes
`submitted` / `included` at 1 / `mined` at 10, all gateway-reported; retries every
three blocks, conservative exact error mapping, source comparison/single-source
warnings. Prepare for offline self-splits in CLI/web/Basecamp/Android use the
ZIP 317 action-count fee; notes unlock only after local scan passes expiry.
Sync/control padding is smaller; new-tip sync replaces blind 30-second polling.
The dated 3,457-block replay measures **2.40 MB/day total stable BLE-link framing**
(2.12 MB sync-only), or 2.52 MB TCP, including hourly signed presence and idle
control. Presence remains immediate on link-up and expires after three hours;
gateway changes/loss may take an hour to notice on stable links. These are
lossless serialization/framing results, not physical-radio measurements.
Since v0.7.5 tips push on every observed height/hash (five-second gateway poll),
not the replay's 15-minute coalescing; that traffic number is not a current budget.
See [chain evidence](components/chain.md#dated-service-checks),
[testnet installation](install.md#public-testnet-v070) and
[funding failures](components/offline-builder.md#v070-testnet-note-plan).
**Still open:** independent public confirmation and radio/phone/macOS checks.
The funded split → offline mesh payment → ten gateway-reported confirmations
ran on Linux in the 28 October verification above.
The approximate 1.2 MB/day target must be compared with full framed wire bytes,
not only compact payload. PoW/MMR/inclusion proofs remain tasks 7–9.

**27 October release checks:** `cargo test --workspace` passed 37 tests;
`cargo clippy --workspace --all-targets -- -D warnings` passed. Mesh bench: 23/0;
LAN bench: 10/0, including received-note spending after the payment-result
refresh fix. Kotlin-only links passed at 20/182/514-byte chunks, with receipts,
mining, restart inventory and callback backpressure. All four `nix build .#lgx`
packages built on Linux x86_64. Android release Kotlin compilation and lint
passed; no arm64 APK/device or macOS run is implied. Web screenshots use the live
testnet wallet; QML screenshots/interaction use a fixture bridge, not Basecamp.

The owner tested **v0.5.1 on three real Macs successfully**. v0.7.x (including v0.7.7)
still needs real-Mac retesting. Basecamp Local Network
permission support is pending [logos-basecamp#527](https://github.com/logos-co/logos-basecamp/issues/527);
fallback: dial `<operator-ip>:7777`. Linux/Xvfb runs do not verify macOS or radios.

Task 5 is **partial**: signed statements/receipts, fee/sequence checks, local conflicts and invited pending caps are built; device-bound enrollment, binary formats, invoices and family accounting are not. Still unbuilt: nullifier sharing (6), chain-state bundles (7–9), Tor (10), the co-signer group and iOS. **Next transport steps** (after the Basecamp macOS/Linux focus): real Android phones ([mesh.md BLE link](components/mesh.md#ble-link-android-first), B2–B7) and Linux desktop adapters ([D1/D2/D5](components/mesh.md#linux-desktop-ble-implementation-radio-gates-open)): one-hop Linux↔Linux/Android, Basecamp, forced multi-hop, BLE→LAN gateway, durability and capture. BLE links are implemented, radios unverified. D3 adds macOS; D4 waits for Basecamp's Bluetooth usage-description support. The gateway's result is "gateway-reported", not proved, and v0.7.7 stops recurring polling after `mined` rather than continuously tracking later reorgs.

## Design problems (in order)

Status saved 26 October 2026. “Settled” means design agreed, not implemented.

| # | Problem | Status / owner |
|---|---|---|
| 1 | Offline double-spend | **Re-decided 26 Oct:** pending below the floor; payer + 3-of-4 co-signer group above it; receipt-gated near a gateway ([double-spend](components/double-spend.md), [pending-tier](components/pending-tier.md)) |
| 2 | Chain-state trust at B | **Settled**; leftovers decided 26 Oct ([payee-verifier](components/payee-verifier.md)) |
| 3 | Transaction expiry and re-issue | **Decided 26 Oct** for a minimal build (10 and B-initiated repair deferred); not implemented ([expiry-reissue](components/expiry-reissue.md)) |
| 4 | Gateway protocol | **Spec v1 written 26 Oct** ([gateway Protocol v1](components/gateway.md#protocol-v1-owner-decisions-26-october-byte-layouts-proposed-not-implemented)): anyone submits, tip/conflicts broadcast mesh-wide, pulled catch-up; timing, PMv2, Tor isolation, limits and forwarding wording decided; header delivery = sampled proofs (heuristic, λ 64, 100-header suffix, post-NU7 checkpoint), byte layout still to write; must also run on public testnet; bundle/receipt wire encoding, signature coverage, retries/idempotency and lag/fork handling; also how one gateway orders and submits many payers' transactions arriving together (A→B, Y→Z) while each faces its own online-sweep race, which is not yet researched ([gateway](components/gateway.md)) |
| 5 | Transport durability | Original BitChat-fork plan superseded by `ozm-mesh`: persisted sealed packets, inventory/acks, app retries and A/B submissions are built. Android/Linux BLE streams are implemented; radio gates remain. Offline Protocol sidecar was rejected after research (custody refuses DMs, forged-ACK risk, AGPL/commercial); no current BitChat-fork dependency |
| 6 | Fleet fingerprint | **Researched 26 Oct:** keep expiry, immediate send and standard tx shape; P12 plaintext acks kept for MVP/testnet (least work), encryption is a mainnet gate ([privacy-and-threats](privacy-and-threats.md)); was: expiry/anchor/shape/uplink privacy tradeoff ([privacy-and-threats](privacy-and-threats.md)) |
| 7 | Stack compatibility | **Gate 5 passed 26 Oct** on Zebra 7 Regtest; Common 4 migration completed for v0.7.0. Funded v0.7.5 payments exercised current bytes on public testnet; older FROST research remains separately scoped ([chain](components/chain.md)) |
| 8 | Offline limits and phone proving cost | **Decided 26 Oct** (32-note inventory, ≥ floor needs escrow, 3 ZEC escrow cap per payee, foreground-only on iOS, co-signers on B/C/D; [offline-builder](components/offline-builder.md)); was: note budget, no unmined change, real Ironwood phone time/RAM ([offline-builder](components/offline-builder.md)) |
| 9 | Policy values | **Decided 26 Oct** for harness/testnet (#9 research): current limits kept, 1 ZEC escrow cap per payer and per group, 3 ZEC combined ceiling, defaults in [pending-tier](components/pending-tier.md) and [gateway](components/gateway.md), fleet owner key signs checkpoints. Mainnet loss budget and delivery target still needed before launch; was: measured exposure, freshness, anchor window, confirmations and loss appetite ([pending-tier](components/pending-tier.md)) |

### Decided 26 October (minimal build)

- **#3:** proposals 1 (shutdown profile, 72,576 blocks), 2, 3, 5–9, 11, 12 accepted; 4 for A's automatic re-issue only; 10 and B's `ozc1:rq:` deferred; 13 rejected ([expiry-reissue](components/expiry-reissue.md#proposals-and-owner-decisions-26-october)).
- **#2 leftovers:** counters reset at `confirmed` (10) on both networks; `checked (single-source)` is Receipt state bit 0x80 ([pending-tier](components/pending-tier.md)).
- **Delivery:** gateways at hubs plus app copies, no carrier protocol yet. **Demo:** 4 co-signer processes across the four Macs, two on one Mac for the demo only.
- Before **co-signer safety claims**, test the group's [failure model](components/double-spend.md#failure-model-gate-1-written-26-october-reasoned-not-yet-tested) (written 26 Oct) and resolve the randomizer / DKG gates ([double-spend Open](components/double-spend.md#open)).

## Build MVP v1 (in order)

This is the implementation backlog, distinct from the problem-by-problem design order above. Problem #3's accepted/deferred items are recorded in its component doc; exact repair layouts remain open. Research PoCs do not implement the app.

| # | Task | Done when | Doc |
|---|---|---|---|
| 1 | Offline builder: one v6 Ironwood transaction with Zakura Common 2.2.0, no network, fleet expiry and fee | Raw bytes written; mined on `ths` — **done in v0.1.0** ([mvp-v0.1.0](mvp-v0.1.0.md)) | [offline-builder](components/offline-builder.md) |
| 2 | Payee verifier in a separate process: read, decrypt own output, proofs and signatures, v6/Ironwood-only/branch ID/expiry/anchor checks | Accepts the tx from 1; receipt says "passes checks at height H; pending" — **done in v0.1.0** (offline checks; chain-state bundle checks are task 7) | [payee-verifier](components/payee-verifier.md) |
| 3 | Gateway submitter: `sendrawtransaction`, then a signed bundle back (tip header, txid branch) | Idempotent resubmission; B verifies the branch and moves to "included" — **v0.1.0: idempotent submit and a signed "gateway-reported" result; the txid branch is not built yet** | [gateway](components/gateway.md) |
| 4 | Mesh transport: tx + statement A→B, B persists, B→D forward, receipts back | **Built:** TCP v0.1.0, LAN v0.2.0, Android BLE stream v0.4.0 and Linux backend v0.4.1. Physical-radio gates open | [mesh](components/mesh.md) |
| 5 | Complete the pending tier: device-bound enrollment, binary statements/receipts, invoices and family accounting | **Partial:** fee/sequence/local conflict checks and invited admission caps built. Complete the approved ten-confirmation reset/repair model and durability gates; never grant open pending acceptance | [pending-tier](components/pending-tier.md) |
| 6 | Payee-to-payee nullifier sharing over `ozm-mesh` (replace the archived BitChat-group carrier), with C as second payee; gateway mempool receipt | Same note paid to B then C while connected: C refuses with reason 8 | [pending-tier](components/pending-tier.md) |
| 7 | Chain state, step 1: bundle wire format with every slot, signed with the gateway's enrollment payment key; gateway rebuilds the history MMR from `getblock h 0` + `z_gettreestate`, persists leaves, rebuilds on reorg; payee checks signature, anchor-in-window, expiry margin, txid branch, MMR anchor path to header N+1 | Tests 8–14 pass on `ths` | [gateway](components/gateway.md), [payee-verifier](components/payee-verifier.md) |
| 8 | Chain state, step 2: Equihash (200,9) and difficulty checker against a pinned floor from a setup checkpoint; `regtest-no-pow` flag | Tests 15–16 pass | [payee-verifier](components/payee-verifier.md), [chain](components/chain.md) |
| 9 | Chain state, step 3: two-gateway tip agreement; `checked (single-source)` with the stricter cap; targeted conflict notices; full nullifier list on request | Tests 17–19 pass with a second gateway process | [payee-verifier](components/payee-verifier.md), [pending-tier](components/pending-tier.md) |
| 10 | Tor submission path on the gateway: fresh circuit per tx, immediate submission | A send reaches a public endpoint through Tor; endpoint sees only the exit IP | [gateway](components/gateway.md) |

## Problem #2, decided 25 October 2026: chain state without trusting the gateway

B is offline and learns the tip, anchors, nullifier spends and inclusion only from D. The **unbuilt design** treats D as untrusted (ADV-G) on every network and requires proof of work and Merkle paths; the current testnet app instead trusts gateway reports. The design table is in [payee-verifier.md](components/payee-verifier.md), gateway duties in [gateway.md](components/gateway.md). Three sub-decisions:

1. **Single-gateway degradation.** With one gateway reachable, B still verifies PoW and paths but marks the state `checked (single-source)` and applies the stricter ⌈C/2⌉ cap until a second source confirms. [decided]
2. **Nullifier delivery.** Targeted conflict notices (`nf → T′` with T′'s bytes and txid branch) by default; the full nullifier list for the fleet window on request. [decided]
3. **Gateway signing key.** D signs bundles with its enrollment payment key, the same per-device Ed25519 scheme as payer and payee, so two conflicting signed tips from one gateway are portable evidence. [decided]

The Regtest harness has no proof of work. The planned validator therefore needs an explicit `regtest-no-pow` exception tested with mainnet vectors, leaving the other bundle checks unchanged. That production validator/flag is not yet built.

## Tests that gate MVP v1

1. Fabricated-anchor transaction is rejected by the payee. ([payee-verifier](components/payee-verifier.md))
2. Deliberate conflicting spend: B reports "conflicted", not "final". ([double-spend](components/double-spend.md))
3. A sends to B → B confirms only after saving → A disappears → B restarts → D appears later → B delivers, D submits, confirmation returns. **Passes in v0.1.0** on `ozm-mesh`. ([mesh](components/mesh.md))
4. Packet capture: no plaintext amounts, payees or keys on the mesh. ([privacy-and-threats](privacy-and-threats.md))
5. Same tx bytes accepted by Zebra 7, not only Zakura. **Passed 26 Oct (scoped):** official Zebra 7.0.0 (`e8db87e`) fresh NU7 Regtest; a Common 2.2.0 payer + 3-of-4 FROST 9,166-byte v6 spend (txid `72beac5d…49d7`) was accepted to the mempool and mined at 103 (#7 research). Public testnet and mainnet untested. ([chain](components/chain.md))
6. BitChat delivery time for a 9,166-byte tx plus statement (base64 ≈ 12.7 KB) over 1 and 3 hops. Measured over TCP links in v0.1.0; BLE still to measure. ([mesh](components/mesh.md))
7. A statement with a wrong `auth_digest`, wrong amount, replayed `seq` or non-fleet fee is refused with the right reason code; a receipt is sent only after the payee's store is fsynced. ([pending-tier](components/pending-tier.md))
8. **New-payment acceptance** anchors inside/outside the provisional 100-block window: accept/refuse with reason 6. Not a consensus age bound; test existing-family historical-root renewal separately after #3 approval. ([payee-verifier](components/payee-verifier.md))
9. A tampered bundle and a bundle signed by an unknown key are rejected before any other check. ([gateway](components/gateway.md))
10. Txid branch verifies for a block with 3 transactions and fails for the wrong txid or the wrong position. ([gateway](components/gateway.md))
11. Expiry on both sides of the owner-approved lag/uplink/confirmation/margin budget (#3 proposal 6). Initial H + 20 remains **provisional and insufficient** against thirty-minute view lag (~72 NU7 target blocks), not a safety guarantee. ([payee-verifier](components/payee-verifier.md))
12. The gateway's rebuilt history roots match every header's `hashBlockCommitments`, including after `ths reorg` and `ths rollback`; the rebuilt root agrees with `zcash_history::Tree`. ([gateway](components/gateway.md))
13. Real anchors verify against header N+1 and the tip; a flipped anchor, leaf, peak position or `authDataRoot` fails. ([payee-verifier](components/payee-verifier.md))
14. Conflict notice: a deliberate conflicting spend delivered as `nf → T′` with T′'s bytes and branch ends as `conflicted` (extends test 2). ([payee-verifier](components/payee-verifier.md))
15. A real mainnet header passes the Equihash (200,9) and difficulty check and a corrupted one fails, with `regtest-no-pow` off; with it on, a live 177-byte Regtest header is accepted and all other bundle checks still run. ([chain](components/chain.md))
16. An anchor from before an upgrade activation is proved against the pre-activation tree; needs a mainnet-like chain (issue log #1 in [AGENTS.md](../AGENTS.md)). ([chain](components/chain.md))
17. Gateway equivocation (one key, one `seq`, two bodies) is kept as evidence; two different tips are treated as lag or reorg, and the payee refuses pending acceptance until they agree. ([gateway](components/gateway.md))
18. With one gateway, the state is `checked (single-source)` and the ⌈C/2⌉ cap is a hard cap; a second agreeing gateway lifts it. ([pending-tier](components/pending-tier.md))
19. The full nullifier list for the fleet window arrives on request and the payee finds the same conflict as the targeted notice. ([gateway](components/gateway.md))

## Next phase (MVP v1.1): co-signer group

- Payer + 4 co-signers, threshold 7 of 8 identifiers (A holds 4), stock `zakura-reddsa` `frost` DKG, redacted PCZT, durable lock and nonce store; weighted signing already mined on `ths` ([double-spend](components/double-spend.md)).
- Gates: the [Open list](components/double-spend.md#open) plus the [problem #3 gates](components/expiry-reissue.md#ten-gating-tests-not-completed-by-the-research).

## MVP v1.2

- Co-signer reconfiguration (replace a lost co-signer by sweeping escrow notes to a new group key online), proposed in [double-spend Open 3](components/double-spend.md#open). Replaces the fenced-cold-share / tlock options.

## Known gaps carried forward (not fixed by design)

- The gateway is the first spy, and one gateway cannot prove that no conflict exists (completeness); ≥ 2 gateways, nullifier sharing and loss caps bound it. ([gateway](components/gateway.md))
- Headers cannot prove the heaviest chain; the pinned difficulty floor must track mainnet. ([payee-verifier](components/payee-verifier.md))
- Fleet transactions are fingerprintable on chain. ([privacy-and-threats](privacy-and-threats.md))
- Shutdown profile chosen (21 days, assumes ≤7-day uplink). Neither covers arbitrary shutdowns, and thirty-minute **new-acceptance** freshness remains provisional/separate. Renewal needs A even in 2-of-2 escrow; refusal leaves a debt claim. ([expiry-reissue](components/expiry-reissue.md))
- Common 4 already knows mainnet NU7, but application mainnet support, proofs, privacy, hardware/security gates and explicit real-value approval remain. ([chain](components/chain.md))
- Unmeasured: Equihash and Ironwood proving on Mac/iPhone; mainnet proof sizes. The 100-block new-payment anchor window, 20-block initial expiry margin, thirty-minute freshness and ten confirmations are **provisional** with the reasons above, not validated mainnet safety bounds. ([gateway](components/gateway.md), [pending-tier](components/pending-tier.md))
