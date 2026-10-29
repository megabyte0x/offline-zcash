# Privacy and threat model

**Current app: v0.7.7, public testnet / Regtest, test coins only.** Ironwood hides
amounts/recipients on chain; signed and sealed `ozm1` payloads hide payment
details from relays. Headers, timing, sizes and public txid/status notices still
expose metadata. Gateway/lightwalletd chain reports are trusted, not proved.

The [implementation trust inventory](#trust-assumptions-at-v070) takes precedence
over the earlier mainnet design below. Its baseline audit is v0.7.2, with named
v0.7.3–v0.7.7 amendments, **not a new whole-codebase security audit**. Co-signers,
Tor, independent chain proofs and encrypted-at-rest keys remain unbuilt.

Related: [design.md](design.md), [components/double-spend.md](components/double-spend.md), [components/mesh.md](components/mesh.md), [components/gateway.md](components/gateway.md), [components/payee-verifier.md](components/payee-verifier.md). Deep sources: [threat_model.md](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), [report §Metadata](../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md), [reconciliation](../archive/reports/Design%20review%20reconciliation.md).

## Decided

Mainnet requirements from the 25–26 October 2026 reviews
([reconciliation](../archive/reports/Design%20review%20reconciliation.md) G1, G2, G4, G6),
using the original BitChat/Noise carrier, now superseded by `ozm-mesh`.

| # | Finding | Decision |
|---|---|---|
| P1 | Public signed intent (nullifiers, txid, payee, amount, expiry, payer key) was a plaintext payment record; BitChat broadcast signatures do not encrypt | Send details/accountability signature only to the payee over Noise; relays/gateway get tx bytes only |
| P2 | Per-payer counters and accountability signatures link payments to a stable identity | Known fleets only, never strangers; hot-list nullifiers, not payer keys |
| P3 | First-spy gateway can withhold, delay or lie | Assume ADV-G on every network, without spending/viewing keys. Verify every acted-on fact with PoW, MMR anchor paths, txid branches and enrollment-key bundle signatures. Require ≥ 2 gateways agreeing on the tip, else `checked (single-source)` and stricter cap. Conflict omission still needs multiple gateways, sharing and loss caps ([gateway.md](components/gateway.md), [payee-verifier.md](components/payee-verifier.md)). Replaces "accept in MVP v1" |
| P4 | Old anchors, long expiry, uniform padding and uplink bursts fingerprint the fleet | Accept "fleet users", not "all Zcash users", as the anonymity set; use uniform policy to blend within it |
| P5 | Stable BitChat IDs/announcements expose topology; 4 devices give a tiny set | Harness and mainnet: private, same-size traffic, no payment data outside Noise. Plan ID rotation after measuring outbox cost (open 3), then Logos ([archived plan](../archive/docs-2026-10-27/later/logos.md)); replaces "accept in demo" |
| P6 | Nullifier/`rk` in the sighash let co-signers identify signed notes without amounts/recipients or an FVK | Redacted PCZT, no viewing key; bilateral payees may share the FVK by agreement, not requirement (corrects earlier P6) |
| P7 | lightwalletd sees submitter IP/timing | Tor, fresh circuit per tx, immediate submission. Random delay dropped 26 Oct to narrow the sweep window. Build after loopback `ths`, not a later mainnet stage ([gateway.md](components/gateway.md)) |
| P8 | Public ths dev keys derive from all-zero entropy (`schema.rs` L42) | Regtest only, never real value |
| P9 | Nullifier sharing reveals receiving payee/time to fleet payees and links it to the mined tx, not amounts | Closed fleets only; share txid/expiry/nullifiers, never payer/amount ([pending-tier.md](components/pending-tier.md)) |
| P10 | Each consulted co-signer (≥ 3) learns A's txids, nullifiers, `rk`, `cv`, `cmx`, ciphertexts, expiry/timing and holds partial spending authority | Accept for v1.1, without amounts/recipients/FVK. Original demo: separate offline Mac, not gateway ([double-spend.md](components/double-spend.md)) |
| P12 | Mesh-wide acks expose txid/auth digest/status **before mining**, tying chain transactions to place/time, not amounts/parties | Owner, 26 Oct: least-work MVP/testnet option. **Mainnet gate:** fleet-key-encrypt `ack`/`cnf`, keep `tip` public (#6 option B) |
| P11 | Device payment key/`seq` expose stable identity and running offline-payment count to payees | Accept within fleet (P2); count may reset per enrollment |

## Findings

### What leaks, by layer

| Layer | Leak | Source |
|---|---|---|
| On chain | Nullifiers, note commitments, action count, anchor, expiry height, `valueBalance`, exact size; no fee field. 820 B per action | [ZIP 229](https://zips.z.cash/zip-0229), [ZIP 225](https://zips.z.cash/zip-0225) |
| On chain | Ironwood formal verification covers soundness and balance, not privacy | [crypto.news](https://crypto.news/zcash-ironwood-upgrade-whats-changed-after-the-orchard-bug/) |
| On chain | ZIP 315 recommends anchor H−3 for its wallet policy; expiry defaults depend on library/height: `ths` Common 2.2.0 uses target+40, the app's Common 4 NU7 default is target+120 (~50 min), overridden by fleet policy. Long expiry/old anchors distinguish classes, not proof a tx was offline; fees also fingerprint | [ZIP 315](https://zips.z.cash/zip-0315), [ZIP 218](https://zips.z.cash/zip-0218), [ZIP 317](https://zips.z.cash/zip-0317), [prior art](prior-art.md) |
| Broadcast | The first node to broadcast is the apparent origin; Zcash txs deanonymized from timing "with a high level of accuracy" (pre-Zebra) | [Biryukov & Tikhomirov 2019](https://orbilu.uni.lu/bitstream/10993/39724/1/biryukov-tikhomirov-deanonymization-and-linkability) |
| Broadcast | Long-lived nodes over Tor face exit-banning and fingerprinting; a global passive adversary defeats Tor | [Biryukov & Pustogarov 2015](https://arxiv.org/abs/1410.6079), [Kappos & Piotrowska 2019](https://arxiv.org/pdf/1902.07337) |
| Light client | lightwalletd learns IP, sync height and full-tx fetches; may show fake payments if headers are not validated | [ZIP 307](https://zips.z.cash/zip-0307), [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf) |
| Keys | A UFVK reveals incoming, outgoing and change history; a UIVK only incoming. ZF's FROST book assumes participants share the FVK, but our co-signer signs from a redacted PCZT and needs none (P6) | [ZIP 316](https://zips.z.cash/zip-0316), [ZF FROST book](https://frost.zfnd.org/zcash/technical-details.html), [double-spend.md](components/double-spend.md) |
| Mesh | Public broadcast reveals the sender's neighbourhood; peer-ID rotation does not remove timing correlation. Announces every 4 s, then 15–30 s | [bitchat_mesh.md](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/bitchat_mesh.md) L34, L99 (bitchat@`04995f68`) |
| Mesh | Android Wi-Fi Aware uses a hard-coded PSK `"bitchat_secret"` | bitchat-android@`137ce6d` `WifiAwareMeshService.kt` L69–70 |
| Radio | LoRa devices fingerprinted by deep-learning RFFI, including unseen devices; RSSI/TDOA localization; selective jamming cut reception to 1.3% | [Liverpool TIFS 2022](https://livrepository.liverpool.ac.uk/3148161/), [arXiv 2006.01278](https://arxiv.org/pdf/2006.01278), [Aras et al.](https://arxiv.org/pdf/1712.02141) |
| Radio | Amateur bands forbid encryption (47 CFR 97.113(a)(4)), so encrypted Zcash payloads cannot use them | [physical_mesh_links.md](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/physical_mesh_links.md) L49, L77–78 |
| Legal | Relay and gateway operators are choke points; Samourai founders sentenced Nov 2025 for unlicensed money transmission | [US DOJ SDNY](https://www.justice.gov/usao-sdny/pr/founders-samourai-wallet-cryptocurrency-mixing-service-sentenced-five-and-four-years) |
| Endpoint | Supply-chain duplicate keys in radio firmware | [CVE-2025-52464](https://nvd.nist.gov/vuln/detail/CVE-2025-52464) |

### Adversaries

| Code | Adversary | Main capability here |
|---|---|---|
| ADV-SND | Dishonest payer | Double-spends in the offline window ([double-spend.md](components/double-spend.md)) |
| ADV-G | Malicious or compromised gateway | Withholds, delays, time-shifts, forges chain bundles; first-spy. Cannot alter a signed tx |
| ADV-S | lightwalletd/indexer operator | IP, timing, interest; can lie about chain state |
| ADV-P | Zcash P2P observer, chain analyst | Broadcast origin; anchor/expiry/size fingerprints |
| ADV-GPA | Global passive network adversary | Correlates Tor traffic; links gateway uplink to chain |
| ADV-N | Malicious mesh node or Sybil swarm | Neighbour timing, topology, eclipse, dropping |
| ADV-L | Local RF observer | Direction finding, RF fingerprinting, jamming, burst traffic analysis |
| ADV-E | Endpoint attacker | Malware, supply chain, key theft |
| ADV-C | Captor or coercer | Seized devices, disclosed keys |

### Top threats and residual risk

Qualitative ratings from [threat_model.md §7](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), updated for BitChat, **assuming the design mitigations**. Shipped-app risks follow in the implementation inventory.

| Threat | Mitigation | Residual |
|---|---|---|
| Payer double-spends while offline | Pending tier with caps/PaymentStatement and partition-local nullifier sharing; v1.1 co-signer group (payer + 3-of-4) | High at pending; escrow prevention conditional on honest **non-rollbackable** C and every spend/recovery path using the lock. Written failure model and tests required before claiming safety |
| Gateway first-spy and chain-state liar | Tor, fresh circuit per tx; ≥ 2 gateways; PoW-validated headers, MMR anchor paths, txid branches, signed bundles; `checked (single-source)` with one gateway | Medium (first-spy); omitted conflicts remain undetectable from one source |
| Fleet fingerprint (anchor, expiry, padding, uplink bursts) | Freshest anchor; one fleet expiry; standard 10,000-zat fee; 2-action padding; spread broadcasts | Medium |
| Mesh topology and origin via stable peer IDs and announcements | Private messages only; same-size traffic | Medium–High (tiny set) |
| Sybil/eclipse feeding fake chain data | Known peers; header validation; ≥2 gateways | Medium–High |
| RF direction finding, fingerprinting, jamming | Low power, short bursts, multiple paths | High against a determined local adversary |
| Seed restored on a second device | One spending device per account | Looks exactly like a double-spend; conflict evidence cannot tell |
| Device compromise or seizure | Full-disk encryption; minimal history; no keys on relays or gateway | Medium |
| Viewing-key exposure | Never share a UFVK; UIVK only for watch-only | Medium |
| Operator legal exposure | Commodity hardware, anyone can be a gateway | Medium–High in hostile jurisdictions |

### Assumptions: harness versus the design target

Right: design target. Left: planned harness checks, not passed tests.

| Assumption | Local Regtest harness (planned chain-proof checks) | Mainnet (design target) |
|---|---|---|
| Payers are known and accountable | Closed fleet | Holds only inside a fleet; fails with strangers |
| Gateway is honest | Planned lying-gateway tests inject forged bundles deliberately; current app does not verify bundles | Assume ADV-G: every bundle fact verified; ≥ 2 gateways |
| Proof of work on headers | Regtest has `with_disable_pow(true)` (issue log #8). Production checker and explicit `regtest-no-pow` exception with mainnet vectors are planned, not shipped ([chain.md](components/chain.md)) | Equihash and difficulty against a pinned floor on every header the payee accepts |
| Gateway can reach the network | Loopback to `ths` | Intermittent; Tor is often blocked in shutdowns, so the gateway may be exposed or absent |
| Uplink comes before expiry | Explicit mining exercises block age, not real outage availability | Shutdown profile selected: 72,576 blocks (~21 days), assuming ≤7-day uplink. Not an availability guarantee or cover for arbitrary shutdowns. Renewal needs A; debt remains ([expiry/re-issue](components/expiry-reissue.md)) |
| No radio adversary | TCP/container checks do not exercise radios | Fails: ADV-L can locate and jam transmitters |
| Anonymity set | Worthless coins, so untested | Fleet users only; small |
| Keys | Public dev keys (all-zero entropy, P8) | Real seeds, hardware or FROST-held |
| Legal exposure | None | Operators may face money-transmission or radio charges |

### Fleet fingerprint (#6, 26 October)

- **On chain:** 72,576-block expiry/old anchors mark a *delayed-payment class*, not this fleet. Absolute expiry reveals policy-derived build height (E − 72,576). Keep ordinary markers: `lock_time` 0, minimum-2 padding (4/7 sampled Ironwood-only mainnet txs: 2 actions / 9,166 B / 10,000 zats), conventional ZIP 317 fee, freshest verified anchor. FROST adds no marker (64-byte signature, rerandomized `rk`). Reused repair nullifiers link observed versions; the canonical chain shows only the winner. #6 research
- **Mesh:** P12's pre-mining txid/nullifier ↔ place/time link is worse than on-chain markers. Public `tip` reveals only gateway presence/freshness. BitChat's outer group `senderID` is claimed, unsigned and unbound to the inner author (`BLEService.swift` L2830–2855 at `04995f68`).
- **Anonymity set:** ~4.17 M ZEC in Ironwood at height 3,531,315 (ZecBlock, 26 Oct) measures money, not users. No post-NU7 user estimate or numeric anonymity promise; measure before mainnet.

## Open

1. Test: packet capture shows no plaintext amounts, payees or keys on the mesh (reconciliation §6 test 5).
2. Test: a transaction against a fabricated anchor is rejected by the payee ([payee-verifier.md](components/payee-verifier.md)).
3. [unverified] Whether BitChat private messages leak recipient identity to relays beyond the routing header; peer-ID rotation cost for the outbox.
4. [unverified] Tor reachability from the gateway in a real shutdown; fallback transports (bridges) not evaluated. Source for "often blocked" was the review, not an archive note.
5. [unverified] `schema.rs` L42 all-zero entropy: from the reviews; ths file path and commit not re-checked here.
6. [partly measured] Clustered pre-NU7 sample: only 4/1,500 mined txs had ≥10,368 blocks left at inclusion. Supports concern, not a representative anchor-age or future-NU7 distribution. Expired/replaced nullifiers stay linkable; quantify joint expiry/anchor/burst distinction in problem #6 ([expiry research](components/expiry-reissue.md#lifetime-freshness-privacy-and-upgrades)).
7. [unverified] Duress or hidden-wallet support for shielded Zcash on any hardware signer.

### Corrections to archive text

- Report flow step 5 (intent fan-out to gateways) and research note §9 (signed intents gossiped mesh-wide) → payee only (P1).
- Threat note "rotate peer IDs per session" vs report "stable `--nodekey`": both written for Logos; the BitChat transport keeps stable peer IDs for now, a stated limitation (P5).
- Report and MVP v1 design "gateway over Tor and PoW-validated headers in a later mainnet stage" → part of the design, with a build order (P3, P7; [gateway.md](components/gateway.md)).
- Report and card note: hot list of payer keys → nullifiers only (P2).
- Threat note §1 says NU7 "not yet active": active from height 1 on the local chain and 4,465,026 on public testnet; mainnet activation is scheduled at 3,543,405 (around 5 November 2026), not yet reached at this document's update.
- This file, 25 October morning, P6 "FROST participants hold the escrow FVK": not required (R1, R2); the co-signer works from the sighash alone.

## Trust assumptions at v0.7.0

**Audit scope:** written 27 October 2026 at v0.5.1 (`3395bd1`); every row rechecked
against v0.7.1 (`9651853`) before v0.7.2's contained fixes. Historical line numbers
remain at `3395bd1`; named amendments cover known changes through v0.7.7, not a
fresh whole-codebase audit. F1–F10 consolidate controls shared by the rows.

**v0.7.3 discovery:** only `gateway` remains; payer/payee/relay roles never had
effect. Invited devices trust their named gateway, not first use. Zeroing the
unauthenticated 8-byte LAN fleet tag and 4-byte BLE hint removed the membership
marker (S4/PO2): invited devices now auto-connect to any Offline Zcash device,
like open devices. Stable peer IDs, IPs and app presence remain visible.

**v0.7.0 traffic evidence; v0.7.5 cadence amendment:** compact sync and new-tip/control messages use
512 B / 4 KiB / 16 KiB buckets; `pay`/`sub`/`rcpt` padding is unchanged.
**Sync/payment sizes and timing differ**; no cover traffic or recipient hiding.
Signed height/hash pushes follow every observed tip (five-second gateway poll),
replacing v0.7.0's 15-minute/200-block coalescing. Fallback: 15 minutes, backing
off to an hour unchanged. Only predecessor mismatch exchanges checkpoint hashes;
normal requests omit them. Results push immediately. Signed presence is immediate
on link-up, otherwise hourly with three-hour expiry; stable-link gateway
role/change/loss detection can take an hour.

The **v0.7.0 replay, not current traffic evidence**, measured 1,144,132 compact
payload bytes, 2,118,573 framed sync bytes, **2,404,889 total BLE-link framed bytes /
2,520,079 TCP** per 86,390 seconds. Includes hourly presence, key exchanges, acks
and keepalives; excludes reconnects/reorgs/retransmits and radio/IP headers
([measurement boundaries](components/mesh.md#v070-compact-sync)).

**v0.7.6–v0.7.7 recovery/work:** Leave preserves keys/history; used invite-free
backups restore deferred incoming queues too. Events rotate above 8 MiB. Incoming
checks survive sender loss/restart, but are not acceptance or spendable funds.
Chain checks, connections and submissions have deadlines ([gateway limits](components/gateway.md#v077-busy-gateway-fix)).
Storage, custody migration and stopped polling of `mined` entries are detailed in
F3/F8/F10 below.

### v0.7.5 readiness: unresolved security work

- **Frontier truth (G3/B1):** size matching block metadata is not proof. Bind
  frontier/root to an authenticated history leaf; complete PoW/MMR/inclusion
  verification (tasks 7–9).
- **One public testnet operator (G3/L1):** only testnet.zec.rocks was reachable.
  Compact ranges download once from the primary; applied batch-end roots are
  compared with reachable secondary GetTreeState roots, not ciphertexts/nullifiers
  byte-for-byte. Matching roots do not prove those contents genuine/complete.
  Single/colluding operators remain trusted, including the chain clock for
  offline-created wallets. Independent reachable operators and proofs are needed.
- **Birthday/interest leak (G6/L3):** removing a height input does not hide first
  heights, creation-day estimates or recovery ranges. Local creation metadata is
  not a privacy protocol. Tor/isolation, cover traffic, less identifying start
  buckets and block-based rather than per-txid status discovery remain work.
- **Automatic split fingerprint (G6/L3):** equal small outputs and correlated
  funding/split timing/fees may fingerprint wallets. Defaults favor readiness,
  not unlinkability; mainnet remains unsupported/unapproved.

[Open-mode controls](#open-mode-controls-v060) detail v0.6.0 authentication,
bindings, gateway pins and API protection. **Inferred** means a threat, not a
demonstrated exploit.

Paths: `core/` = `crates/zcash-offline-core/src/`, `mesh/` = `crates/ozm-mesh/src/`, `node/` = `crates/ozm-node/src/`, `kt/` = `android/app/src/main/java/org/zcash/offline/`.

Status: **Enforced** = stated control in code, not adversary defeated;
**Partial** = named limits remain; **Not / Deferred** = no complete control;
**Unverified** = not demonstrated. Rows use the v0.7.2 baseline plus named amendments.

### Code facts behind most rows

- **F1. Identity, not metadata:** v0.6.0 signs inner envelopes and payload-bound destination acks; v0.7.2 requires fresh Ed25519 key/peer-ID proof for inventory. This is key possession, not membership. Hello and outer sender/recipient stay cleartext; strangers/Sybils can live-forward. Older v0.7.x peers cannot fetch v0.7.2 inventory.
- **F2. Unverified chain:** role/pin-checked signed sync authenticates sources only. Compare reachable servers' common-height hashes, frontier roots and inclusion answers; withhold disagreements. Unreachable/lagging servers reduce source count and trigger a warning; one source is accepted on testnet. v0.7.5 downloads compact contents once. Single/colluding sources remain trusted; PoW/MMR/inclusion proofs (tasks 7–9) are mainnet gates.
- **F3. Reports, not finality:** v0.7.0 accepts newer signed downgrades and classifies submission errors exactly. v0.7.2 removes cached-mined fallback when tracked txs disappear. v0.7.7 stops polling `mined` entries: saved ten-confirmation reports do not monitor later reorgs.
- **F4. Nonces, not anonymity:** v0.7.2 persists `notify: ["nonce:<128-bit random>"]` instead of party names and broadcasts signed nonce/result bindings (P12). Legacy requests/private results still work. Gateways see authenticated submitters, duplicate txids/timing and, if enrolled, the public fleet directory.
- **F5. Refusal is not cancellation:** v0.7.0 warns "may still mine until expiry"; results can supersede refusal.
- **F6. Invited-key custody:** `fleet-init` generates named devices' spending/Ed25519 keys; unsigned invites carry raw private keys. v0.7.6 Organiser defaults to gateway-only export, but CLI/module callers can export other devices. Invite-free wallets generate local keys the organiser never receives.
- **F7. UI/API:** v0.6.0 text-only rendering, per-run token and loopback Host validation; no protection against same-user key/token access.
- **F8. Bounded mesh:** v0.7.2 caps stored packets at 4,096 / 64 MiB; claimed sender at 512 / 16 MiB; seen/acked IDs at 32,768 each; messages at 2 MiB; partial reassembly at eight / 4 MiB; links at 32; queued payloads at 64 MiB. Reject >five-minute future skew before dedup. Preserve held messages, evicting that sender's oldest acks first; origins may also evict their broadcasts and (v0.7.6) directed replaceable presence/tip notices. One-time v0.7.6 migration drops this device's unmarked directed messages for app retry, retaining other senders' custody. Spoofed headers can evict claimed-sender acks and delay confirmation. Global caps bound Sybils, not availability; admission rewrites storage, so sustained floods can deny service. Memory/handler queues are bounded too.
- **F9. Expiry unlock:** v0.7.0 releases unspent reservations only at locally scanned tip > inclusive expiry; rollback relocks. Standalone expiry reports cannot unlock; the scanned source is still trusted.
- **F10. Private modes, plaintext:** v0.7.2 enforces Unix 0600 on keys, wallet, payment history, events, trust pins, mesh cache and old/temp replacements. v0.7.6 adds/migrates 0700 device/backup/export folders. Same-user malware/other modules and system/cloud backups still expose keys. Windows relies on account ACLs. Strict Ed25519 (`verify_strict`) predates v0.7.2; static X25519 has no forward secrecy.

### 1. Payer (Alice)

| # | Assumption | S/P | What breaks if violated | Likelihood / impact | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| A1 | Payer does not double-spend pending | S | Multiple payees or online sweep bypass local locks | High / per-payee admission bound only | Locks/conflict checks/floor; v0.7.2 caps (B2) | **Partial**, not prevention | Sharing/co-signers; reorg-restored debt can exceed caps |
| A2 | Payer is accountable (P2) | S | Throwaway IDs defeat debt claims | High / High | v0.6.0 forbids open/unenrolled pending | **Enforced** | Inclusion remains gateway-reported |
| A3 | Anchor is a real root | S | Payer/view-source collusion fabricates notes | Medium / High | Signed/pinned sync and comparisons (F2) | **Partial / Deferred** | Tasks 7–9: MMR, PoW, inclusion |
| A4 | The statement matches the transaction | S | Amount, payee or txid mismatch | — | `node/app.rs:630-644` | **Enforced** | — |
| A5 | Device protects spending key | S | Theft from `secret.json`, organiser copy (F6) or invite | Medium / High | 0600 `node/lib.rs:185-193`; Android backup off | **Partial** | Keystore/Keychain, on-device keys before mainnet |
| A6 | Synced notes/balance are true | S | Source fabricates or hides notes | Medium / Medium | F2 | **Partial / Deferred** | B1: agreement/authentication ≠ proof |
| A7 | Refusal prevents settlement | S | Already-submitted tx mines after A pays again | Medium / Medium | Locks; "refused, may still mine until expiry"; superseding results (F5) | **Partial** | Never interpret refusal as cancellation |
| A8 | Withheld txs release notes | S | Source swallows tx, stalls/falsifies expiry | Medium / Medium | Rollback-aware local scan unlock (F9) | **Partial**, source trusted | Tasks 7–9 for verified expiry |
| A9 | Paying B hides A → B | P | Header IDs; 12 KiB `pay` bucket (`mesh/packet.rs:41-70`, `core/envelope.rs:20, 50`) | High / Medium | Sealed payload | **Not** | Fixed size, recipient hiding, cover traffic before mainnet |
| A10 | Max hides payer state | P | Reveals spendable value/input shape, not entire balance | Low / Low | `core/builder.rs` excludes locked/uneconomic notes | **Not / Deferred**, testnet limit | Amount/input privacy policy, not arithmetic fix |

### 2. Payee (Bob)

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| B1 | Tip/anchors/nullifiers are real | S | Source fabricates roots/frontiers or hides spends | High / High | Pinned sync, rollback-aware scanning, comparisons (F2) | **Partial / Deferred** | Tasks 7–9 before mainnet |
| B2 | Pending exposure is bounded | S | Cross-payee spends, reorg debt, false chain state | High / new admission ≤3 ZEC per payee | v0.7.2 serialized admission: 3 outstanding / 1 ZEC per signer, 3 ZEC total. Expired debt counts; mined releases; downgrade counts again | **Partial**, reorgs can exceed caps | Verified freshness/sharing; caps are provisional test-coin policy |
| B3 | `included`/`mined` are true | S | Authenticated sources lie about confirmations | Medium / High | Newer signed downgrades; no cached-mined fallback (F3) | **Partial**, truth unproved | Tasks 7–9, independent gateways before mainnet |
| B4 | Statements require enrollment | S | Stranger impersonates an enrolled payer for pending | — | Signed envelopes/statements; fleet-only pending since v0.6.0 | **Enforced**, open waits for inclusion | — |
| B5 | Address → mesh ID proves ownership | S+P | False bindings redirect receipts | High / High | v0.6.0 receiving-authority proof, first-use pin, address payments | **Enforced**, receiving only | Spending-authority proof, independent crypto review |
| B6 | B stores the tx before the receipt | S | Lost payment | — | fsync first `node/app.rs:672-699` | **Enforced** | — |
| B7 | Forwarding hides the payee | P | Submitters/duplicate txids/timing correlate parties | High / Medium | Persisted nonces, signed broadcasts (F4) | **Partial** | Tor/recipient hiding; P12 accepts public status/txid on testnet |
| B8 | Address doesn't link payments | P | Static device address (`core/fleet.rs:129-131`) | High / Low–Medium | Shielded on chain | **Not / Deferred** | Diversified authority proofs, binding discovery, durable receipt routing—not just new address strings |

### 3. Relays and other mesh devices

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| R1 | Relays forward | S | Withholding | Medium–High / Medium | Flooding, 24 h store, app retries, v0.6.0 authenticated acks | **Partial** | Independent routes; signatures cannot force forwarding |
| R2 | Relays cannot read payments | P | Public status/txid/nonce, not amounts/statements (P12) | High / Medium | X25519 + ChaCha20-Poly1305 private bodies; plaintext notices | **Partial** | Fleet-key-encrypt notices before mainnet |
| R3 | Past traffic stays private after key leakage | P | Static keys; 24 h sealed-packet retention | Medium / High | — | **Not** | Forward secrecy: rotation/ratchet before mainnet |
| R4 | Relays cannot impersonate | S | Forged control/payment/results | High / High | Signed envelopes/notices; strict Ed25519 already in v0.7.1 | **Enforced** | Headers spoofable; compromised keys still trusted |
| R5 | Resources resist exhaustion | S | Sybil/churn/I/O denial | Medium / Medium | v0.7.2 limits (F8) | **Partial** | Headers do not authenticate sender; quotas ≠ availability |
| R6 | Routing stays private | P | Observers/authenticated strangers learn metadata | High / Medium | v0.7.2 inventory challenge (F1) | **Partial** | Membership/link encryption; old peers only live-forward |

### 4. Gateway device

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| G1 | Gateway submits received txs | S | Withholding/delay; nearest open announcer wins | Medium / High | A/B both submit `node/app.rs:494-496, 702-704`; retries | **Partial** | ≥2 gateways; inclusion proofs detect withholding |
| G2 | Chain reports are honest | S | Fake `included`/`mined` | Medium / High | Signature only `core/messages.rs:90-99` | **Designed only** | Tasks 7–9 before mainnet |
| G3 | Sync data is real | S | Poisoned compact blocks/birthday frontier | Medium / High | F2 | **Partial / Deferred** | B1: chain proofs |
| G4 | Gateway is legitimate | S+P | First-contact Sybil/eclipse, dishonest pin | High / High | v0.6.0 role checks, signed presence, durable pin; block/warn changes | **Partial** | Signed enrollment, chain proofs |
| G5 | Parties stay private | P | Submitters/duplicate txids/timing correlate | High / Medium | v0.7.2 nonce results (F4) | **Partial** | B7; not anonymity |
| G6 | No first-spy/interest leak | P | First tx observer; sync heights (`node/app.rs:263-268`), v0.7 birthdays | High / Medium | — | **Not**, Tor task 10 unbuilt | Testnet accepts; Tor before mainnet |
| G7 | Honest path classifies correctly | S | Unknown wording becomes success | Medium / Medium | v0.7 exact code/message pairs, structured nullifier conflict; queued-download/full/behind-tip retry, not acceptance; unknown wording rejects | **Enforced** | Deliberately update tested server-version mappings |
| G8 | Gateway holds no wallet | P | v0.7 gateway is also a wallet; lightwalletd first-spies its own txs/IP | High / Medium (inferred) | — | **Not** | Separate gateway identity or Tor |
| G9 | Gateway resists floods | S | Sybils/network withholding deny service | Medium / Medium | v0.7.2 pre-proof admission: 1/s burst 4/sender; 256 held/sender; global 4,096 / 128 MiB, strangers 1,024 / 32 MiB; bounded notify/workers; exact retries reuse verification | **Partial** | Preserve held bytes/enrolled reservation, not availability or Sybil resistance |

### 5. lightwalletd server operator

testnet.zec.rocks; the Zaino backup returned HTTP 503 on 27 October 2026.

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| L1 | Chain is true | S | Fabricated compact actions/tree/nullifiers/confirmations; empty compact headers | Low–Medium / High | Reachable-source comparison, withhold disagreement (F2); v0.7.5 primary-only compact ranges | **Partial**, only primary reachable | Tasks 7–9; agreement ≠ proof |
| L2 | `SendTransaction` forwards | S | Servers censor despite OK | Low–Medium / Medium | v0.7.2 concurrent unchanged-byte submission to every connected configured server despite first failure; any acceptance means only submitted | **Partial** | Independent reachable operators, proofs |
| L3 | Fleet stays anonymous | P | Gateway IP, fleet txs/timing, birthdays, sync ranges, txid queries (inferred) | High / Medium | TLS | **Not** | Tor per tx; block-based status |
| L4 | Service is available | S | All servers fail, including Zaino 503 | Medium / High | Configurable TLS list/failover; single-source UI | **Partial**, backup unavailable | Independent reachable endpoints |
| L5 | TLS authenticates server | S | Mis-issued trusted certificate impersonates | Low / High | v0.7.0 HTTPS/WebPKI cert/hostname checks, no insecure fallback | **Enforced**, CA trusted | SPKI needs operator-coordinated keys/rotation; no stable pins available |

### 6. Zcash network, miners and testnet

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| Z1 | Consensus mines at most one conflicting spend | S | — | — | Duplicate nullifiers rejected by nodes | **Enforced** (network) | — |
| Z2 | 10 confirmations mean final | S | Testnet minimum-difficulty blocks after 450 s gaps make cheap reorgs feasible | Medium / Low (test coins) | — | **Not** | Testnet is a functional test only |
| Z3 | Common 4.0 output is valid on public testnet | S | Payments rejected | Medium / Medium | Regtest acceptance plus funded v0.7.5 Linux Basecamp split/offline payment/restart/reconnect | **Exercised 28 Oct** (gateway-reported ten confirmations, not independent proof) | Broader adversarial/platform tests remain; [dated evidence](open-work.md#built) |
| Z4 | The fleet fee fits every transaction shape | S | Large splits need more than 10,000 zat | High / Medium | Builder and verifier use max(fleet floor, ZIP 317 padded-action fee); optional change included | **Enforced** (v0.7) | Keep 10,000 zat for two actions, not all shapes |
| Z5 | On-chain privacy | P | Fingerprints: 72,576-block expiry (`core/fleet.rs:37`), anchor age, 2-action shape, split outputs | High / Medium | Accepted (P4) | **Partial** | Accept on testnet |
| Z6 | Testnet stays live through the expiry window | S | A reset or upgrade kills unmined txs | Low / Medium | — | **Not** | — |

### 7. Organiser and invite issuer (fleet mode)

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| O1 | Organiser doesn't misuse invited keys | S+P | Steal/sign/decrypt for invited, not invite-free wallets (F6) | Low / High | v0.7.6 gateway-only export; invite-free local keys | **Partial** | On-device invited keys, signed public enrollment |
| O2 | Invites stay confidential in transit | S | An intercepted invite gives every key | Medium / High | 0600 on disk | **Not** | As O1 |
| O3 | Fleet file and invites are authentic | S | A tampered `fleet.json` swaps payee addresses or gateway keys | Low–Medium / High | Consistency only `node/lib.rs:196-214` | **Not** | Organiser signature and fingerprint check |
| O4 | Policy values are sane | S | Bad floor, fee, branch or expiry (`core/fleet.rs:15-44`) | Low / Medium | — | **Partial** | Signed policy |
| O5 | Only trusted devices get the gateway role | S | Rogue gateway | Low / High | `node/app.rs:170-176, 813` | **Enforced** (fleet) | — |

### 8. Open-mode strangers on the LAN or BLE mesh

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| S1 | Strangers cannot inject | S | Own-identity traffic intentionally accepted | High / High | v0.6.0 signatures; v0.7.2 inventory proof/caps (F1/F8) | **Partial** | Membership, Sybil resistance |
| S2 | No Sybil | S | Unlimited IDs hijack first contact/eclipse | High / High | Gateway/address pins, comparisons; v0.7.2 admission bounds | **Partial / Deferred** | Trusted enrollment/proofs; quotas cannot stop new IDs |
| S3 | No name/address impersonation | S | Display-name collisions | High / High | v0.6.0 authority proofs/pins; route by mesh key, not unknown names | **Partial** | Human-verifiable identity/enrollment |
| S4 | Membership/topology hidden | P | Stable beacon ID, IP, app presence | High / Medium | v0.7.3 zero LAN tag/BLE hint removes membership marker only | **Not / Deferred** | Android/Linux identity/outbox rotation needs protocol change, incompatible with old peers |
| S5 | Remote strings cannot execute | S | Stored-XSS regression | Medium / High | v0.6.0 text-only UI/token/Host; regression tests | **Enforced** | No remote strings in HTML construction |
| S6 | No stranger pending | S | No accountability | High / High | v0.6.0 open/unknown wait for gateway inclusion | **Enforced** | — |

### 9. The faucet

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| FA1 | The public faucet doesn't link requesters | P | Learns requester IP and address | High / Low | — | **Not** | Request via the gateway or Tor, fresh address |
| FA2 | Faucet pays NU7 Ironwood | S | No funds arrive | Medium / Medium | 27 Oct: Valar failed, zecpages timed out; later Linux run used owner TAZ | **Faucet unverified** | Funded app tests do not establish faucet reliability |
| FA3 | Gateway faucet resists draining | S | New IDs evade throttles | Medium / test coins only | v0.6.0 request/device/60 s, ten notes, configured total cap; Regtest mining gateways only, off on testnet | **Partial** | Sybil-resistant enrollment; never real value |

### 10. Software distribution

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| D1 | Catalogue and packages come from us | S | Catalogue on `main` points at mutable `releases/latest`; packages unsigned (`trustedSigners: []`); a compromised account ships a key-stealing module | Low / High | Content roots and SHA-256 (integrity only) | **Not** | Sign `.lgx`, pin versions before mainnet |
| D2 | The APK signing key is secret | S | Public test key (`android/test-signing.p12`): anyone can ship an update inheriting app data and keys | Medium / High | Documented warning | **Not** | Private release key and new package ID before real value |
| D3 | The CLI installer is authentic | S | Checksum from the same release (`scripts/install.sh:70-91`); macOS binary unsigned | Low / High | — | **Partial** | Signed releases |
| D4 | CI is trustworthy | S | Compromised runner/account/pinned dependency ships malware | Low / High | v0.7.2 SHA-pins all external actions; tagged releases skip Rust/Gradle branch caches | **Partial** | Reproducible/signed releases, account controls |
| D5 | Basecamp isolates modules | S | Other modules/compromised host invoke spending IPC (inferred) | Low–Medium / High | No isolation guarantee | **Not / Deferred** | New host protocol with origin-bound caller metadata; name-only allow-list is insufficient and breaks older hosts |

### 11. Passive network or radio observer

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| PO1 | The mesh hides who pays whom | P | Cleartext sender/recipient, size and time; `sub` timing links the txid to A and B | High / Medium | Sealed payload | **Not** | Uniform size, cover traffic |
| PO2 | BLE doesn't track devices | P | Static service UUID, unencrypted GATT and stable peer ID | High / Medium | No device name; payment bodies sealed; v0.7.3 advertises a zero fleet hint | **Not / Deferred** | As S4: identity/outbox/radio coordination needs a protocol change |
| PO3 | The LAN doesn't leak | P | Beacons and plaintext TCP | High / Medium | — | **Not** | Link encryption |
| PO4 | The uplink doesn't leak | P | Gateway IP and tx timing at lightwalletd | High / Medium | TLS | **Not** | Tor |
| PO5 | No active radio adversary | S+P | Jamming, direction finding | Low–Medium / High | Accepted | **Not** | — |

### 12. Device OS and platform

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| OS1 | The OS RNG is sound | S | — | — | `core/fleet.rs:91-108` | **Enforced** (OS) | — |
| OS2 | Key storage resists malware and seizure | S | Plaintext `secret.json` | Medium / High | 0600; Android backup off | **Partial** | Keystore, Keychain, Secure Enclave |
| OS3 | Wallet/history files are private | P | Same-user malware/root reads plaintext | Medium / Medium | v0.7.2 atomic 0600 writes/migration, including old/temp files (F10) | **Enforced**, modes not encryption | Windows account ACLs; keystore deferred |
| OS4 | Only user calls local API | S | Same-user token/key access; hostile web origins | Medium / High | v0.6.0 token, loopback Host/default bind | **Partial** | OS account isolation required |
| OS5 | Android sandbox holds | S | Native metadata plaintext; manifest cannot enforce native TLS | Low / Low | v0.7.2 `usesCleartextTraffic=false`; backup off; launcher only export; Rust testnet TLS | **Partial** | TCP/BLE link encryption; hardware unverified |
| OS6 | Clock is roughly right | S | Wrong expiry/presence; lost seq history permits old-time replay | Low / Medium | Durable monotonic seq with clock-derived floor; v0.7.2 five-minute skew cap, bounded cache lifetime | **Partial** | Trusted time/identity reset |
| OS7 | A seized device doesn't expose past traffic | P | No forward secrecy | Medium / High | — | **Not** | Key rotation |

### What each party can learn

| Party | Amounts | Addresses | Sender ↔ receiver | Timing | Location / topology | Balance |
|---|---|---|---|---|---|---|
| Payer | Own | Payee's | Own | Yes | Payee's hop count | No |
| Payee | Paid to it | Payer's (fleet file) | Yes | Yes | Payer's hop count | Partly, via a Max payment |
| Relay | Size bucket only | No | **Yes** (header IDs); public result txids | Yes | **Yes** | No |
| Gateway | No | Public fleet directory if enrolled; no explicit notify pair | Submitters/duplicate txids/timing remain correlatable (F4) | Yes, first spy | Partly | No |
| lightwalletd | No | No | No, but sees the fleet's whole tx set | Yes | **Gateway IP** | No |
| Network / miners | No | No | Nullifier linkage on chain | Yes | No | No |
| Organiser | Invited wallets only | Invited directory | Invited wallets/captures | With captures | If on the mesh | Invited wallets only (holds their keys) |
| Open-mode stranger | Size only | Bindings are privately exchanged, but addresses can be shared publicly | **Yes**, header IDs/result txids | Yes | **Yes**, plus IPs | No |
| Faucet | Its payout | Requester's | No | Yes | Requester IP | No |
| Software distribution | Everything, via a malicious build | | | | | |
| Passive observer | Size only | No | **Yes**, header IDs/result txids | Yes | **Yes**, RF location | No |
| Device OS | Everything on the device | | | | | |

### What each party can do

| Party | Steal | Double-spend | Censor | Delay | Lie about mined | Deny service | Impersonate |
|---|---|---|---|---|---|---|---|
| Payer | No | **Yes** (pending) | — | Yes | No | Spam | Throwaway IDs in open mode |
| Payee | No | No | Partly (A also submits) | Yes | No (can falsely refuse) | — | No |
| Relay | No | No | **Yes** (drop; not forged acks) | Yes | Only if also a trusted view source | Yes, within caps | Cannot forge signed bodies without keys |
| Gateway | No | Can hide conflicts | **Yes** | **Yes** | **Yes** | Yes | Open-mode announcement |
| lightwalletd | No | Can hide conflicts | **Yes** | Yes | **Yes**, fleet-wide | **Yes** | It is the chain |
| Network / miners | No | Enables it (testnet reorgs) | Yes | Yes | Reorgs undo confirmations | Yes | No |
| Organiser | Invited keys only | Invited keys only | Yes | Yes | If trusted as gateway | Yes | Invited identities only |
| Open-mode stranger | Not through the fixed XSS/API paths | Never offered pending | Yes | Yes | If initially picked as gateway | Yes, within caps | Own names/Sybils, not another key's signed body |
| Faucet | No | No | Withholds funds | Yes | No | — | No |
| Software distribution | **Yes** | **Yes** | Yes | Yes | Yes | Yes | **Yes** |
| Passive observer / jammer | No | No | Jamming only | Jamming only | No | Jamming only | No |
| Device OS | **Yes** | **Yes** | Yes | Yes | Yes | Yes | **Yes** |

### Remaining work after the v0.7.2 contained fixes

| Rows | Deferred work and reason |
|---|---|
| A3/A6/A8, B1/B3, G1–G4, L1/L4, Z2 | Tasks 7–9, independent reachable operators, verified freshness; authenticated/agreed reports are not chain proof/finality. |
| A1/B2, P9/P10 | Cross-device sharing/co-signers and adversarial review; per-payee caps do not bound fleet loss. |
| A9, B7, G5/G6/G8, L3, PO1/PO3/PO4 | Transport/routing/deployment changes for Tor, recipient hiding, cover traffic, fleet-encrypted results and separate gateway identity. |
| A5, O1–O4, G4, S2/S3 | On-device fleet keys/signed public enrollment change the trust lifecycle; pins are not membership or Sybil resistance. |
| B8/A10/Z5 | Diversified receipt bindings and wallet amount/input policy; Max necessarily reveals the paid value to its recipient. |
| PO2/S4, R3/OS7 | Android/Linux discovery/key/outbox coordination for ID rotation and forward secrecy. |
| D1–D3/D4 | Owner-controlled signing keys/distribution policy, private APK key, reproducible releases; CI pins/cache isolation do not authenticate packages. |
| D5 | Upstream host-enforced caller origin/authorization, not module-name allow-lists. |
| OS2/OS3 | Platform storage/recovery design: keystore/Keychain/Secure Enclave, Windows ACL integration. |
| L5 | Operator-agreed SPKI keys and rotation/recovery; opportunistic leaf pins create brittle TOFU on top of mandatory HTTPS/WebPKI. |
| R5/R6, G9, OS6, PO5 | Membership, trusted time, independent routes; quotas/key possession guarantee neither availability nor radio privacy. |
| Z3/FA2 | Beyond the 28 Oct owner-funded v0.7.5 Linux flow: chain proof, Android/current Mac/radio checks and faucet reliability. |

### Top 10 residual risks (v0.7.2 baseline, amended through v0.7.7)

1. **Trusted chain reports** (L1–L4, B1/B3), without proofs or a reachable second public endpoint.
2. **Organiser custody/unsigned invites** (O1–O3), exposing invited-device keys.
3. **First-contact Sybil/eclipse** (G4, S2), before pins help.
4. **Pending double-spend** (A1, B2), without sharing/co-signers.
5. **Unsigned packages/public APK key** (D1–D3).
6. **Plaintext keys/host trust** (OS2, D5), beyond Unix modes/API tokens.
7. **Metadata/first-spy correlation** (A9, B7/G5/G6, PO1–PO4), including public txid/status.
8. **Censorship/resource denial** (R1/R5/R6, G1/G9), despite bounded stores/inventory authentication.
9. **No forward secrecy** (R3, OS7), exposing captured traffic after key seizure.
10. **Testnet/hardware gaps** (Z2/Z3, FA2): cheap reorgs, no independent chain proof or current Mac/phone/radio verification.

### Statements earlier in this file that are out of date for v0.7.2

The inventory above supersedes the four-Mac/BitChat design where they differ:

- **P1/P5:** `ozm-mesh`/`ozm1`, public sender/recipient headers, UDP/BLE discovery
  and plaintext TCP (F1, S4, PO1–PO3). Packet-capture open item 1 must include these leaks.
- **P1/P7:** submitters/nonces remain visible (F4), not the old
  `notify: [payer, payee]` pair. Third-party lightwalletd replaces the assumed own
  node; Tor is unbuilt.
- **P3:** Medium residual assumed PoW/MMR/inclusion proofs and two gateways;
  shipped authentication/comparison leaves **High** risk (F2).
- **P2/P11:** closed-fleet accountability becomes no open pending and capped
  invited pending (A1/A2/B2); monotonic sequences use a clock floor (OS6).
- **P9/P10/P12:** sharing/co-signers, conflict bundles and mainnet fleet-key
  encryption are unbuilt; only signed nonce-result broadcasts ship (F4).
- Public testnet, empty compact headers/birthdays/frontiers, custody, API/host,
  distribution, storage, faucet and address-binding risks are covered by the new
  inventory rows. Splits/Max exceed the original 2-action/10,000-zat shape and pay
  larger ZIP 317 fees (Z4); fingerprints remain.

## Open-mode controls (v0.6.0)

Controls introduced in v0.6.0, with later amendments noted:

- **Authentication:** sender-signed, recipient-bound inner envelopes, including
  sync/faucet/submission requests and replies; destination-signed delivery acks.
  Sealing alone is not authentication. Gateway announcements/address bindings are
  individually sealed; mesh IDs/public routing keys are broadcast. Hourly
  presence/three-hour expiry above replace v0.6.0's 35-second lifetime; missed
  presence can take three hours to expire a gateway.
- **Gateway choice:** open devices rank fewest hops, highest advertised tip, then
  mesh ID. First use durably pins the key in `data/trust.json`; nearer/newer
  candidates cannot replace it. An unavailable pin warns/blocks switching.
  v0.7.3 invited devices instead trust the fleet's named gateway. To change
  operator, verify its mesh ID outside the app; use Basecamp Leave session
  (v0.7.3), or stop the device and remove **only** `data/trust.json`, never
  `secret.json` or wallet data. First-contact TOFU permits Sybils; hops/tips and
  sync/inclusion/mined reports are not chain proofs.
- **Address binding:** domain-separated Schnorr knowledge proof of the Ironwood
  incoming-viewing scalar, bound to exact address/mesh ID; no key is shared.
  This is **receiving, not spending authority**; copied IVKs qualify. First
  receipt routing pins address → mesh key; changed/ambiguous bindings disable
  receipts, never redirect funds. The typed address sets the payment destination.
  Experimental, without independent cryptographic review.
- **Acceptance:** open/unknown payers never get pending. `Included` receipts wait
  for pinned-gateway inclusion reports, not independent proof/finality.
- **Local API:** per-run random token and loopback Host header; UI/CLI obtain
  token from local page. Render remote strings as text. Bind to `127.0.0.1`;
  `--api 0.0.0.0` is isolated-test-only, not a secure network API. Same-user
  programs remain trusted.
- **Regtest faucet:** one request/device/60 s, persisted across gateway restarts;
  at most 10 notes, configured total cap (default 1 ZEC). Fresh IDs evade it.
- **Compatibility:** v0.5.x ignores unsupported v0.6.0 messages without crashing;
  v0.6.0+ rejects unsigned legacy envelopes/acks. Upgrade payment/sync endpoints
  together.
