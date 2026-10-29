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

Mainnet design agreed in the 25–26 October 2026 reviews
([reconciliation](../archive/reports/Design%20review%20reconciliation.md) G1, G2, G4, G6).
BitChat/Noise references record the original carrier, superseded by `ozm-mesh`;
these tables are requirements/research, not claims that every mitigation ships.

| # | Finding | Decision |
|---|---|---|
| P1 | The old step-3 public broadcast of a signed intent (nullifiers, txid, payee, amount, expiry, payer key) was a plaintext payment record; BitChat public payloads are signed, not encrypted | **Fixed:** relays and the gateway get transaction bytes only; details and accountability signature go to the payee over a Noise-encrypted private message [decided] |
| P2 | Identity-keyed controls (per-payer counters, accountability signatures) link every payment to a stable identity | Closed, known fleet only; never with strangers. Hot lists hold nullifiers, not payer keys [decided] |
| P3 | The gateway is the first broadcaster (first-spy) and can withhold, delay or lie about chain state; it sees every transaction first | The design assumes ADV-G on every network. It holds no spending or viewing key; every chain fact the payee acts on is verified against proof of work, MMR anchor paths, txid branches and a bundle signature under the gateway's enrollment key; ≥ 2 gateways must agree on the tip, else `checked (single-source)` with the stricter cap. What one gateway can still do is omit a conflict notice; mitigations are ≥ 2 gateways, nullifier sharing and loss caps ([gateway.md](components/gateway.md), [payee-verifier.md](components/payee-verifier.md)). Supersedes the earlier "accept in MVP v1" [decided] |
| P4 | Fleet transactions are fingerprintable: stale anchor, long expiry delta, uniform padding, bursts at each uplink | Accept: anonymity set is "fleet users", not "all Zcash users". Uniform policy so the fleet at least blends with itself [decided] |
| P5 | BitChat stable peer IDs and neighbour announcements leak topology; with 4 devices the anonymity set is tiny | Stated limitation of the BitChat transport, on the harness and on mainnet alike: private messages only, same-size traffic, no payment data outside Noise. Mitigation plan: peer-ID rotation once its cost to the outbox is measured (open 3), and the Logos transport later ([archived Logos plan](../archive/docs-2026-10-27/later/logos.md)). Supersedes "accept in the demo" [decided] |
| P6 | The escrow co-signer does **not** need the escrow FVK: nullifier and `rk` are in the sighash, so it knows which notes it signs without seeing amounts or recipients. Sharing the FVK with a bilateral payee is a choice, not a requirement | Co-signer gets a redacted PCZT and no viewing key; a bilateral payee may hold the FVK only when the pair agrees [decided; corrects the earlier P6] |
| P7 | lightwalletd sees the submitter's IP and timing | Gateway submits over Tor, fresh circuit per tx, immediately (random delay dropped 26 Oct to narrow the sweep window). Part of the design; built after the loopback path to `ths` works (build order in [gateway.md](components/gateway.md)), not deferred to a later stage [decided] |
| P8 | ths dev accounts derive from all-zero entropy (`schema.rs` L42), so their keys are public | Fine for Regtest; never hold real value with them [decided] |
| P9 | Pending-nullifier sharing lets every fleet payee learn which payee received each pending payment and when, and, once mined, link that payee to the on-chain tx. Amounts stay hidden | Accept inside a closed fleet only; payload carries txid, expiry and nullifiers, never payer or amount ([pending-tier.md](components/pending-tier.md)) [decided] |
| P10 | Each consulted co-signer of the group (≥ 3 per payment) learns which txids are the payer's (nullifiers, `rk`, `cv`, `cmx`, ciphertexts, expiry, timing) and holds partial spending authority over escrow notes | Accept for v1.1; it sees no amounts, recipients or FVK. In the demo the co-signer is a separate offline Mac, not the gateway ([double-spend.md](components/double-spend.md)) [decided] |
| P12 | Gateway acks are broadcast mesh-wide (owner, 26 Oct): every device in range learns each txid, its auth digest and status **before mining**, and can tie that on-chain transaction to this area and time. Amounts and parties stay hidden | Accepted for MVP and testnet as the least-work option (owner, 26 Oct). **Mainnet gate:** encrypt `ack`/`cnf` to a fleet key, keep `tip` public (#6 option B) |
| P11 | The payer's device-bound payment key and `seq` give every payee a stable identity and a running count of the payer's offline payments | Accept inside the fleet (P2); the count can reset per enrollment [decided] |

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

Ratings are qualitative (from [threat_model.md §7](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), updated for the original BitChat design). These residuals assume the named design mitigations; current-app risks are listed in the implementation inventory below.

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

The design is written for the right-hand column. The left-hand column describes
the planned harness checks, not a passed test inventory. The current app trusts
chain reports; see the implementation inventory below for controls that ship.

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

- **On chain:** the 72,576-block expiry and old anchors mark a *delayed-payment class*, not this fleet specifically. Expiry is an absolute height, so knowing the policy reveals the build height (E − 72,576). These avoidable markers are already ordinary, so keep them: `lock_time` 0, minimum-2 padding (4 of 7 sampled Ironwood-only mainnet txs were 2 actions / 9,166 B / 10,000 zats), conventional ZIP 317 fee, freshest verified anchor. FROST adds no on-chain marker (normal 64-byte signature, rerandomized `rk`). Repairs reuse nullifiers, so anyone who sees both versions can link them; the canonical chain shows only the winner. #6 research
- **Mesh:** plaintext broadcast `ack`/`cnf` (P12) give a local observer an exact txid/nullifier ↔ mesh place and time link before mining, which is worse than any on-chain marker. Generic `tip` broadcasts reveal only gateway presence and freshness. BitChat's outer group `senderID` is claimed, unsigned and not bound to the inner author (`BLEService.swift` L2830–2855 at `04995f68`).
- Anonymity set: about 4.17 M ZEC was in Ironwood at height 3,531,315 (ZecBlock, 26 Oct). That is money, not users. No post-NU7 user estimate exists; measure before mainnet, with no numeric anonymity promise now.

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

**v0.7.2 audit baseline:** every row was rechecked against v0.7.1 (`9651853`)
before the contained fixes. Older line numbers refer to historical revisions,
not current source locations. Later amendments below update known behavior
through v0.7.7; they do not claim a fresh audit of every row or close mainnet gates.

Wallet/history/cache writes and upgrades enforce Unix 0600. Mesh memory, store,
sender, fragment, link and handler queues are bounded; packets more than five
minutes in the future are rejected. A fresh Ed25519 challenge gates inventory,
not live forwarding or fleet membership: a stranger with its own key can still
authenticate. Old v0.7.x peers live-forward but cannot fetch v0.7.2 inventory.
Submissions carry an opaque persisted 128-bit nonce instead of the parties'
names. Signed nonce results are broadcast in plaintext (P12); old clients retain
private results. The authenticated submitting peer and repeated txid/timing
remain visible, so this reduces the explicit pair leak, not first-spy tracking.

**v0.7.3 update:** the payer, payee and relay roles never had an effect and are
gone; only `gateway` remains. Invited devices trust their fleet's named gateway
rather than pinning the first one they hear. Devices no longer beacon the 8-byte
LAN fleet tag or the 4-byte BLE fleet hint (both are zero), so invited devices now
auto-connect to any Offline Zcash device, as open devices already did. The tag
authenticated nothing. This removes the fleet-membership marker from S4/PO2; the
stable peer ID, IP address and the fact that a device runs Offline Zcash remain
visible.

**v0.7.0 traffic evidence; v0.7.5 cadence amendment:** compact sync and new-tip/control messages use
512 B / 4 KiB / 16 KiB padding buckets; `pay`, `sub` and `rcpt` retain their old
padding. **Sync and payment traffic are distinguishable by size.** Their timing
already distinguished them; no cover traffic or recipient hiding is claimed.
Mesh sync now follows signed height/hash pushes on every observed new tip
(five-second gateway poll). v0.7.0 coalesced for 15 minutes or 200 blocks; its
dated replay below is **not current traffic evidence**. A 15-minute fallback
backs off to an hour when unchanged. Normal requests omit checkpoint history; only
a predecessor mismatch exchanges the retained hashes. Payment results still
push immediately. Full signed presence is hourly on stable links (three-hour
expiry), and immediate on link-up; gateway role/change or loss may take up to an
hour to be noticed on a stable link. The dated replay measured 1,144,132 compact
payload bytes, 2,118,573 framed sync bytes and **2,404,889 total BLE-link framed
bytes / 2,520,079 TCP** per 86,390 seconds, including hourly presence, key exchanges,
acks and keepalives. Reconnects/reorgs/retransmits and radio/IP headers are excluded
([measurement boundaries](components/mesh.md#v070-compact-sync)).

Chain state remains explicitly trusted TLS-lightwalletd/gateway reporting, not
PoW/MMR/inclusion proof verification. Reachable configured servers are compared
for common-height hashes, frontier roots and transaction inclusion;
disagreements withhold the affected answer. Unreachable/lagging sources reduce
the source count and display a single-source warning. Zaino returned 503 on
27 October; a single source is accepted for this testnet release. Tasks 7–9 are
still mainnet gates. Refusal is not cancellation, and expiry unlock uses the
locally scanned tip (itself gateway-reported), not a standalone status message.

**v0.7.6–v0.7.7 amendments:** device/backup/export folders are owner-only on
Unix (0700), files 0600, including migration of old folders. They are plaintext
and included in backups; same-user malware/other modules can read them. Leave
keeps keys/history; used invite-free backups, including deferred incoming queues,
can be restored. Organiser now exports only a gateway invite by default; it
does not hold keys generated by invite-free wallets. Old presence/tip notices
are replaceable; held payment custody is preserved during normal admission.
The upgrade drops this device's old unmarked directed messages once and the app
retries them; it retains other devices' custody. Events rotate above 8 MiB.
Deferred incoming checks survive sender disappearance/restart but are not
acceptance or spendable funds. Chain checks/connections/submission work have
deadlines ([gateway limits](components/gateway.md#v077-busy-gateway-fix)). The
watcher stops polling `mined` entries; saved ten-confirmation reports do not
continuously monitor later reorgs. None of these controls proves chain truth.

### v0.7.5 readiness: unresolved security work

- **Frontier truth (G3/B1):** matching the frontier size to block metadata is not
  proof. Bind the frontier/root to an authenticated history leaf, then complete
  PoW/MMR/inclusion verification (tasks 7–9). Not implemented here.
- **One public testnet operator (G3/L1):** only testnet.zec.rocks was reachable.
  Compact ranges now download once from the primary; locally applied batch-end
  roots are compared with reachable secondary GetTreeState roots. Compact
  ciphertexts/nullifiers are no longer compared byte-for-byte: a matching
  commitment root cannot prove they are genuine or complete. A single operator
  or colluding operators remain trusted, including for the chain clock used by
  offline-created wallets. Independent reachable operators and proofs remain work.
- **Birthday/interest leak (G6/L3):** removing a height input does not hide first
  requested heights, creation-day estimates or recovery ranges from gateways or
  lightwalletd. Creation metadata is private local data, not a privacy protocol.
  Tor/isolation, cover traffic, less identifying start buckets and block-based
  rather than per-txid status discovery remain work.
- **Automatic split fingerprint (G6/L3):** equal small outputs and correlated
  funding/split timing and fees may fingerprint the wallet. Defaults prioritize
  readiness, not unlinkability. Mainnet remains unsupported and unapproved.

**v0.6.0 implementation update:** the review below is preserved at its stated
v0.5.1 source revision. v0.6.0 signs every inner envelope and mesh ack (R1/R4/S1),
adds loopback API tokens/Host checks and text-only web rendering (S5/OS4), proves
address receiving authority and pins bindings (B5), removes pending acceptance for
open/unknown payers (A2/S6), and pins the first used gateway with change warnings
(G4). [Exact controls and remaining limits](#open-mode-controls-v060).
First-contact Sybils, dishonest pinned gateways and unverified chain views remain
risks. Receiving authority is not spending authority, and the new proof code is
experimental and not independently audited.

Originally written 27 October 2026 at `3395bd1` (v0.5.1), then rechecked after v0.7.1. Historical source line numbers remain at `3395bd1`; current controls below name their owning modules. **Inferred** marks a threat, not a demonstrated exploit.

Paths: `core/` = `crates/zcash-offline-core/src/`, `mesh/` = `crates/ozm-mesh/src/`, `node/` = `crates/ozm-node/src/`, `kt/` = `android/app/src/main/java/org/zcash/offline/`.

Status (v0.7.2 baseline, later amendments named in rows): **Enforced** (the stated control is in code), **Partial** (named limits remain), **Not / Deferred** (no complete control), **Unverified** (not demonstrated). Enforced controls do not mean an entire adversary is defeated.

### Code facts behind most rows

- **F1. Identities authenticate, outer metadata does not.** v0.6.0 signs every inner envelope and payload-bound destination ack. v0.7.2 adds a fresh key/peer-ID proof before inventory. The outer sender/recipient and hello remain cleartext; live forwarding permits strangers and Sybils.
- **F2. Offline chain views are unverified.** Signed, role/pin-checked sync authenticates the source, not the chain. Common-height hashes, frontier roots and inclusion answers are compared across reachable configured lightwalletd sources; v0.7.5 downloads compact contents only once. Collusion and a single source remain trusted.
- **F3. Results are rollback-aware, but gateway-reported.** v0.7.0 accepts newer signed downgrades and exactly classifies submission errors. v0.7.2 removes the cached-mined fallback when an actively tracked transaction disappears. v0.7.7 stops recurring polling of entries already `mined`; those saved results are not continuous later-reorg monitoring. No chain proof or finality claim follows.
- **F4. Explicit party-pair disclosure is reduced.** v0.7.2 sends only `notify: ["nonce:<128-bit random>"]` and broadcasts a signed nonce/result binding. Legacy requests still work. The gateway sees each authenticated submitter, can correlate identical txids, and enrolled gateways hold the public fleet directory. P12 exposes nonce/txid/status to mesh observers.
- **F5. Refusal cannot cancel a submitted transaction.** Since v0.7.0 the warning says it may still mine until expiry, refusal is not final, and gateway results can supersede it.
- **F6. The organiser holds invited keys, not invite-free wallet keys.** `fleet-init` generates spending and Ed25519 keys for its named devices; invites carry raw private keys and remain unsigned. v0.7.6 Organiser exports only a gateway invite by default. Other wallets generate keys locally; the organiser does not receive them. CLI/module callers can still export additional invited devices.
- **F7. UI/API controls already shipped.** v0.6.0 uses text-only rendering, a per-run API token and loopback Host validation. They do not protect against same-user malware reading keys or the token.
- **F8. Mesh resource limits now apply.** v0.7.2 caps stored packets at 4,096 / 64 MiB, each claimed sender at 512 / 16 MiB, seen/acked IDs at 32,768 each, messages at 2 MiB, partial reassembly at eight / 4 MiB, links at 32, and queued payloads at 64 MiB. Five-minute future skew is rejected before dedup. Normal admission preserves held messages, evicting that sender's oldest acks first; at the originating device its own broadcasts and (v0.7.6) directed replaceable notices can also give way. The one-time v0.7.6 migration drops this device's old unmarked directed messages; app state retries them. Other senders' custody remains. Spoofed headers can evict a claimed sender's acks at a relay, delaying confirmation; global caps, not identity quotas, bound Sybils. Storage is still rewritten on admission; sustained floods can deny service.
- **F9. Expiry unlock is already implemented.** v0.7.0 releases reservations only after local scan passes inclusive expiry, and relocks on rollback; reported expiry alone is insufficient. The scanned source is still trusted.
- **F10. Plain files have private modes, not encryption.** v0.7.2 enforces Unix 0600 on keys, wallet, payment history, events, trust pins and mesh cache, including old files and temporary replacements; v0.7.6 adds 0700 device/backup/export folders and tightens older folders. Same-user access and system/cloud backups still expose keys. Windows relies on its account ACLs. `verify_strict` was already used in v0.7.1. Static X25519 keys still provide no forward secrecy.

### 1. Payer (Alice)

| # | Assumption | S/P | What breaks if violated | Likelihood / impact | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| A1 | The payer does not double-spend a pending payment | S | Same note to multiple payees or an online sweep; a dishonest owner bypasses local locks | High / new acceptance bounded per honest payee, not across payees | Local locks/conflict checks/floor; v0.7.2 checks 3 payments / 1 ZEC per payer and 3 ZEC per payee before accepting | **Partial** (admission caps fixed; no prevention) | Deferred: nullifier sharing and co-signers; reorgs can restore old mined debt above the caps |
| A2 | The payer is known and accountable (P2) | S | Throwaway open identities cannot support a debt claim | High / High | v0.6.0 disables pending for open and unenrolled payers | **Enforced** (no open pending; already fixed) | Included remains gateway-reported, not final |
| A3 | The payer's anchor is a real chain root | S | Colluding payer and authenticated view source fabricate notes | Medium / High | Signed/pinned sync and reachable-server comparison, but the root is not independently verified | **Partial / Deferred** | Tasks 7–9: MMR paths, PoW and inclusion proofs |
| A4 | The statement matches the transaction | S | Amount, payee or txid mismatch | — | `node/app.rs:630-644` | **Enforced** | — |
| A5 | The payer's device protects its spending key | S | Theft from `secret.json`, the organiser's copy (F6) or the invite in transit | Medium / High | 0600 `node/lib.rs:185-193`; Android backup off | **Partial** | Keystore/Keychain and on-device keys before mainnet |
| A6 | Synced notes and balance are true | S | Lying gateway or lightwalletd shows fake notes or hides real ones | Medium / Medium | Signed sync and reachable-server comparison (F2) | **Partial / Deferred** | As B1; authentication/agreement are not proofs |
| A7 | A refusal means the payment won't settle | S | Payee refuses but A already submitted; A pays again, pays twice | Medium / Medium | Locks persist; UI says "refused, may still mine until expiry"; results can supersede refusal | **Partial** (warning enforced; refusal still cannot cancel) | Never treat a refusal as cancellation |
| A8 | Withheld transactions release their notes | S | Gateway or lightwalletd swallows the tx; a lying view can still stall or falsify expiry | Medium / Medium | Durable reservations release only at locally scanned tip > inclusive expiry, after rollback-aware scanning | **Partial** (expiry unlock enforced; chain view trusted) | Tasks 7–9 before claiming verified expiry |
| A9 | Paying B doesn't reveal that A paid B | P | Packet header shows sender and recipient; the 12 KiB bucket marks `pay` (`mesh/packet.rs:41-70`, `core/envelope.rs:20, 50`) | High / Medium | Sealed payload | **Not** | Fixed size, recipient hiding, cover traffic before mainnet |
| A10 | Max doesn't leak the payer's state | P | Max discloses the spendable value and input/action shape, not the entire wallet balance | Low / Low | Locked and uneconomic notes excluded by `core/builder.rs` | **Not / Deferred** (testnet limitation) | Hiding a deliberate Max payment needs input/amount privacy policy, not a Max arithmetic fix |

### 2. Payee (Bob)

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| B1 | B's chain view (tip, anchors, nullifiers) is real | S | A dishonest selected source can fabricate an anchor/frontier or hide a spend | High / High | Authenticated, pinned sync; rollback-aware scanning; reachable-lightwalletd comparisons | **Partial / Deferred** (source authenticity fixed; truth unproved) | Tasks 7–9 before mainnet |
| B2 | Pending exposure is bounded | S | Cross-payee double-spends, restored reorg debt and false chain state remain | High / new acceptance checked against 3 ZEC per payee | v0.7.2: 3 outstanding / 1 ZEC per signer and 3 ZEC total; serialized admission; expired debt counts, mined debt releases and a downgrade counts again | **Partial** (admission caps fixed; reorgs can restore debt above them) | Deferred: verified freshness and nullifier sharing; numerical caps remain provisional test-coin policy |
| B3 | `included`/`mined` are true | S | An authenticated gateway or lightwalletd can lie about confirmations | Medium / High | Signed newer results can downgrade after reorg; v0.7.2 removes cached-mined fallback when absent | **Partial** (F3 sticky state fixed; truth unproved) | Tasks 7–9 and independent gateways before mainnet |
| B4 | Statements come only from enrolled payers | S | A stranger claims pending acceptance under an enrolled identity | — | Signed envelopes/statements; fleet-only pending since v0.6.0 | **Enforced** (already fixed; open payments await inclusion) | — |
| B5 | Address → mesh-ID bindings are proved by the address owner | S+P | False bindings redirect receipt routing | High / High | v0.6.0 receiving-authority proof, first-use mesh-key pin, address-based payments | **Enforced** (already fixed, receiving authority only) | Spending-authority proof and independent crypto review remain deferred |
| B6 | B stores the tx before the receipt | S | Lost payment | — | fsync first `node/app.rs:672-699` | **Enforced** | — |
| B7 | Forwarding to the gateway hides who was paid | P | Submitter identity, duplicate txids and timing still correlate parties (F4) | High / Medium | v0.7.2 persisted request nonces replace party names; signed broadcast nonce/result binding | **Partial** (explicit notify pair fixed) | Tor/recipient hiding deferred; plaintext status/txid broadcast is P12's accepted testnet cost |
| B8 | B's address doesn't link payments | P | One static address per device (`core/fleet.rs:129-131`) | High / Low–Medium | Shielded on chain | **Not / Deferred** | Per-request addresses need diversified authority proofs, binding discovery and durable receipt routing; not a compatible address-string replacement |

### 3. Relays and other mesh devices

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| R1 | Relays forward and don't drop | S | A relay can still withhold traffic | Medium–High / Medium | Flooding, bounded 24 h store and app retries; payload/destination-authenticated acks since v0.6.0 | **Partial** (forged-ack fix already shipped) | Availability needs independent routes; signatures cannot force forwarding |
| R2 | Relays can't read payment details | P | Status/txid/nonce are now public P12 metadata, not amounts or payment statements | High / Medium | X25519 + ChaCha20-Poly1305 seals private bodies; nonce notices are deliberately plaintext | **Partial** (payment details sealed; broadcast metadata exposed) | Encrypt notices to a fleet key before mainnet |
| R3 | Past traffic stays private if a key leaks | P | Static key, no forward secrecy; relays keep sealed packets 24 h | Medium / High | — | **Not** | Key rotation or ratchet before mainnet |
| R4 | Relays can't impersonate a device | S | Forged private control/payment bodies or results | High / High | Every inner envelope and nonce notice is signed; strict Ed25519 already in v0.7.1 | **Enforced** (envelopes already fixed) | Outer routing headers remain spoofable; compromised keys remain trusted |
| R5 | Relays can't exhaust resources | S | Sybils, churn and I/O floods can still deny service | Medium / Medium | v0.7.2 global/per-sender stores, byte/ID/fragment/link/queue limits and five-minute future-skew cap (F8) | **Partial** (bounded resource admission fixed) | Headers do not prove sender identity; quotas do not guarantee availability |
| R6 | Relays don't learn the mesh's traffic | P | An authenticated stranger or an observer still learns routing metadata | High / Medium | v0.7.2 fresh key-possession challenge before sending/serving inventory | **Partial** (unauthenticated inventory fixed) | Membership authorization/link encryption deferred; old peers only live-forward |

### 4. Gateway device

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| G1 | The gateway submits what it receives | S | Withholds or delays; in open mode the nearest announcer wins | Medium / High | A and B both submit `node/app.rs:494-496, 702-704`, retries | **Partial** | Two or more gateways; detect withholding by inclusion proof |
| G2 | Chain reports are honest | S | Fake `included`/`mined` | Medium / High | Signature only `core/messages.rs:90-99` | **Designed only** | Tasks 7–9 before mainnet |
| G3 | Sync data is real | S | Poisoned compact blocks, wrong birthday frontier | Medium / High | Signed sync and reachable-server comparison | **Partial / Deferred** (F2) | As B1; chain proofs remain unbuilt |
| G4 | The gateway is legitimate | S+P | First-contact Sybil/eclipse or a dishonest pinned gateway | High / High | v0.6.0 fleet-role checks, signed presence and durable first-use key pin; changes blocked/warned | **Partial** (pinning already fixed) | Signed enrollment and independently verified chain views deferred |
| G5 | The gateway doesn't learn payer and payee | P | Sees submitters and can correlate identical txids/timing | High / Medium | v0.7.2 nonce-only notifications and signed nonce-addressed broadcast results | **Partial** (explicit notify pair fixed) | As B7; not anonymity |
| G6 | The gateway isn't a first spy and doesn't learn interest | P | Sees every tx first and each device's sync height (`node/app.rs:263-268`); v0.7 adds birthdays | High / Medium | — | **Not** (Tor, task 10, unbuilt) | Accept on testnet; Tor before mainnet |
| G7 | The honest path is correct | S | Unknown node wording must not become success | Medium / Medium | Exact code/message pairs; structured nullifier conflict; queued-download/full/behind-tip are retryable, not accepted | **Enforced** (v0.7 classifier; unknown wording rejects) | Update tested mappings deliberately if server versions change |
| G8 | The gateway holds no wallet | P | At v0.7 any internet device is a gateway and a wallet; lightwalletd first-spies its own txs with its IP | High / Medium (inferred) | — | **Not** | Separate gateway identity, or Tor |
| G9 | The gateway can't be flooded | S | Sybils and network withholding can still deny service | Medium / Medium | v0.7.2 admission before proofs: 1/s burst 4 per sender; 256 held/sender; 4,096 / 128 MiB global; strangers 1,024 / 32 MiB; bounded notify targets and workers; exact retries reuse verification | **Partial** (quotas fixed) | Global caps preserve held bytes and enrolled reservation, not availability; no Sybil-resistance claim |

### 5. lightwalletd server operator

testnet.zec.rocks; the Zaino backup returned HTTP 503 on 27 October 2026.

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| L1 | It returns the true chain | S | Fabricates compact actions, tree state, nullifiers and confirmations; compact headers are empty | Low–Medium / High | Compare reachable configured servers' hashes, frontier roots and transaction answers; withhold disagreement. v0.7.5 compact ranges come from the primary only | **Partial** (comparison implemented; only primary reachable in public check) | Tasks 7–9 before mainnet; agreement is not proof |
| L2 | It forwards `SendTransaction` | S | One or all servers censor while replying OK | Low–Medium / Medium | v0.7.2 submits unchanged bytes concurrently to every connected configured server, even if the first fails; accepting any means only submitted | **Partial** (multi-submit fixed) | Independent reachable operators and chain proofs still required |
| L3 | It doesn't deanonymize the fleet | P | Sees the gateway IP, every fleet tx with timing, birthdays, sync ranges, txid queries (inferred) | High / Medium | TLS on the path | **Not** | Tor per tx; status from blocks, not per-txid queries |
| L4 | It is available | S | All configured servers can fail; Zaino backup returns 503 | Medium / High | Configurable TLS list and connection failover; UI reports one source | **Partial** (fallback implemented; second public endpoint unavailable) | Add independent reachable endpoints when available |
| L5 | TLS authenticates the server | S | A mis-issued trusted certificate can still impersonate a server | Low / High | v0.7.0 requires HTTPS and WebPKI certificate/hostname validation; no insecure fallback | **Enforced** (strict TLS already fixed; CA trust remains) | SPKI pins deferred: operator-coordinated keys/rotation policy, not available stable pins |

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
| O1 | The organiser doesn't keep or misuse invited keys | S+P | Can steal/sign/decrypt for devices whose invites it generated (F6), not locally created invite-free wallets | Low / High | Gateway-only export by default since v0.7.6; invite-free keys stay on device | **Partial** (invited-key custody remains) | On-device invited keys and organiser-signed public enrollment |
| O2 | Invites stay confidential in transit | S | An intercepted invite gives every key | Medium / High | 0600 on disk | **Not** | As O1 |
| O3 | Fleet file and invites are authentic | S | A tampered `fleet.json` swaps payee addresses or gateway keys | Low–Medium / High | Consistency only `node/lib.rs:196-214` | **Not** | Organiser signature and fingerprint check |
| O4 | Policy values are sane | S | Bad floor, fee, branch or expiry (`core/fleet.rs:15-44`) | Low / Medium | — | **Partial** | Signed policy |
| O5 | Only trusted devices get the gateway role | S | Rogue gateway | Low / High | `node/app.rs:170-176, 813` | **Enforced** (fleet) | — |

### 8. Open-mode strangers on the LAN or BLE mesh

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| S1 | Strangers can't inject traffic | S | Open links still accept traffic under a stranger's own identity | High / High | Signed envelopes/acks since v0.6.0; v0.7.2 inventory proof and resource caps | **Partial** (impersonation already fixed; open admission intentional) | Membership authorization and Sybil resistance deferred |
| S2 | No Sybil | S | Unlimited identities: first-contact hijack or eclipse | High / High | Existing gateway/address pins and comparisons; v0.7.2 global/sender admission bounds | **Partial / Deferred** | Identity quotas alone cannot stop fresh identities; trusted enrollment/proofs needed |
| S3 | Names and addresses aren't impersonated | S | Self-chosen display names still collide | High / High | v0.6.0 address-authority proofs and first-use pins; routing uses mesh keys, not unknown display names | **Partial** (binding spoof already fixed) | Human-verifiable identity/enrollment remains deferred |
| S4 | Membership and topology stay hidden | P | Beacons expose a stable peer ID, IP and that the device runs Offline Zcash | High / Medium | v0.7.3 beacons a zero fleet tag and zero BLE hint, so fleet membership is no longer marked; no other hiding claim | **Not / Deferred** | Coordinated Android/Linux identity/outbox rotation is not contained or old-peer compatible |
| S5 | Remote strings can't run in the UI | S | A UI regression could introduce stored XSS | Medium / High | v0.6.0 text-only rendering and API token/Host checks; existing regression coverage | **Enforced** (already fixed) | Keep remote strings out of HTML construction |
| S6 | Pending is never offered to strangers | S | Pending would lack accountability | High / High | v0.6.0: open and unknown payers wait for gateway-reported inclusion | **Enforced** (already fixed) | — |

### 9. The faucet

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| FA1 | The public faucet doesn't link requesters | P | Learns requester IP and address | High / Low | — | **Not** | Request via the gateway or Tor, fresh address |
| FA2 | The faucet pays Ironwood addresses on NU7 testnet | S | No funds reach devices | Medium / Medium | Valar failed and zecpages timed out on 27 Oct; owner supplied TAZ for the later Linux run | **Faucet unverified** (funded app flow exercised separately) | Do not equate owner-funded testing with faucet reliability |
| FA3 | The gateway faucet can't be drained | S | Fresh identities evade the per-device throttle | Medium / test coins only | Existing v0.6.0 one request/device/60 s, ten notes and configured total cap; Regtest mining gateways only, disabled on testnet | **Partial** (limits/off-on-testnet already fixed) | Sybil-resistant enrollment deferred; never use this development faucet for real value |

### 10. Software distribution

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.2 | Fix and when |
|---|---|---|---|---|---|---|---|
| D1 | Catalogue and packages come from us | S | Catalogue on `main` points at mutable `releases/latest`; packages unsigned (`trustedSigners: []`); a compromised account ships a key-stealing module | Low / High | Content roots and SHA-256 (integrity only) | **Not** | Sign `.lgx`, pin versions before mainnet |
| D2 | The APK signing key is secret | S | Public test key (`android/test-signing.p12`): anyone can ship an update inheriting app data and keys | Medium / High | Documented warning | **Not** | Private release key and new package ID before real value |
| D3 | The CLI installer is authentic | S | Checksum from the same release (`scripts/install.sh:70-91`); macOS binary unsigned | Low / High | — | **Partial** | Signed releases |
| D4 | CI isn't compromised | S | A compromised runner/account or pinned dependency can still ship malicious code | Low / High | v0.7.2 all external actions use full commit SHAs; tagged release builds do not restore Rust/Gradle branch caches | **Partial** (mutable actions/cache exposure fixed) | Reproducible/signed releases and account controls remain deferred |
| D5 | The Basecamp host isolates modules | S | Other installed modules or a compromised host invoke spending IPC (inferred) | Low–Medium / High | No application isolation guarantee | **Not / Deferred** | SDK caller metadata needs newer host protocol and host-enforced origin binding; a name-only module allow-list is not sufficient and would break older Basecamp hosts |

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
| OS3 | Wallet and history files are private | P | Same-user malware/root still reads plaintext files | Medium / Medium | v0.7.2 atomic 0600 writes and migration of existing wallet/history/cache/trust/events/key files, including old temp files | **Enforced** (Unix mode fix; not encryption) | Windows ACLs rely on account configuration; keystore work deferred |
| OS4 | Only the user can call the local API | S | Same-user processes can read the token/keys; untrusted web origins must not gain access | Medium / High | v0.6.0 per-run token, loopback Host validation, loopback default bind | **Partial** (unauthenticated/rebinding access already fixed) | OS account isolation remains required |
| OS5 | The Android sandbox holds | S | Native sockets still carry plaintext mesh metadata; manifest does not enforce native TLS | Low / Low | v0.7.2 `usesCleartextTraffic=false`; backup disabled; only launcher exported; testnet Rust client requires TLS | **Partial** (platform cleartext opt-in fixed) | Native mesh TCP/BLE remains unencrypted at link layer; hardware run unverified |
| OS6 | The clock is roughly right | S | A badly wrong local clock affects expiry/presence; restart after losing sequence history can replay old times | Low / Medium | Existing durable increasing seq; v0.7.2 five-minute future-skew rejection and capped cache lifetime | **Partial** (remote future-packet attack fixed) | Trusted time/identity reset design deferred |
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
| A3/A6/A8, B1/B3, G1–G4, L1/L4, Z2 | PoW/MMR/inclusion proofs (tasks 7–9), independently operated reachable chain sources and verified freshness. Authentication, agreement and rollback-aware reports do not prove chain truth or finality. |
| A1/B2, P9/P10 | Nullifier sharing and co-signers require a cross-device protocol and adversarial review. Local pending caps bound one honest payee's exposure, not fleet-wide loss. |
| A9, B7, G5/G6/G8, L3, PO1/PO3/PO4 | Tor, recipient hiding, cover traffic, fleet-encrypted result broadcasts and separate gateway identity need transport/routing/deployment changes. Nonces remove explicit counterpart names only. |
| A5, O1–O4, G4, S2/S3 | On-device fleet keys and organiser-signed public enrollment change the invite/trust lifecycle. First-use pins are not membership authorization or Sybil resistance. |
| B8/A10/Z5 | Diversified receipt routes and amount/input-shape privacy need binding and wallet policy changes. A deliberate Max payment necessarily reveals the value being paid to its recipient. |
| PO2/S4, R3/OS7 | Coordinated BLE/beacon identity rotation and forward secrecy affect Android/Linux discovery, keys and durable outboxes. v0.7.3 removed the fleet tag/hint; stable IDs remain visible. |
| D1–D3/D4 | Signed packages, private APK signing and reproducible releases need owner-controlled keys and distribution policy. SHA-pinned actions and cache isolation do not authenticate packages. |
| D5 | Host-enforced Basecamp caller origin/authorization requires the upstream host protocol, not an unauthenticated module-name allow-list. |
| OS2/OS3 | Keystore/Keychain/Secure Enclave and Windows ACL integration need platform-specific storage and recovery design. Unix 0600 does not defeat same-user malware. |
| L5 | SPKI requires operator-agreed keys and rotation/recovery policy. Strict HTTPS/WebPKI is already mandatory; inventing an opportunistic leaf pin would create brittle trust-on-first-use. |
| R5/R6, G9, OS6, PO5 | Sybil-resistant membership, trusted time and independent routes are wider designs; finite quotas/key possession do not guarantee availability or radio privacy. |
| Z3/FA2 | The v0.7.2 unfunded gap is historical: owner-funded v0.7.5 Linux split/payment/restart/reconnect reached ten gateway-reported confirmations on 28 Oct. Independent chain proof, Android/current Mac/radio checks and faucet reliability remain unverified. |

### Top 10 residual risks (v0.7.2 baseline, amended through v0.7.7)

1. **Trusted chain reports** (L1–L4, B1/B3): proofs are unbuilt and only the primary public endpoint was reachable. A selected source still decides chain state.
2. **Organiser custody and unsigned invites** (O1–O3): compromise exposes generated invited-device keys; invite-free wallets keep their own keys.
3. **First-contact Sybil/eclipse** (G4, S2): pins prevent silent replacement, not dishonest initial selection.
4. **Pending double-spend** (A1, B2): capped per honest payee; no shared nullifier list or co-signer.
5. **Unsigned packages/public APK key** (D1–D3): pinned CI dependencies do not authenticate downloads.
6. **Plaintext keys and host trust** (OS2, D5): 0600 and API tokens do not isolate same-user malware or a compromised Basecamp host.
7. **Metadata and first-spy correlation** (A9, B7/G5/G6, PO1–PO4): nonce broadcasts trade the explicit party pair for public txid/status and do not hide timing.
8. **Censorship/resource denial** (R1/R5/R6, G1/G9): bounded stores and authenticated inventory are not Sybil resistance or independent routes.
9. **No forward secrecy** (R3, OS7): a later key seizure exposes captured private traffic.
10. **Public-testnet/hardware gaps** (Z2/Z3, FA2): cheap reorgs; funded Linux app flow is exercised but not independently chain-proved, and current Mac/phone/radio checks remain open.

### Statements earlier in this file that are out of date for v0.7.2

The earlier design/research sections use the original BitChat carrier. The implementation inventory and named amendments above win where they conflict:

1. P1 describes BitChat/Noise private messages. The transport is `ozm-mesh` with `ozm1` envelopes, whose headers expose sender and recipient.
2. P1 says the gateway gets tx bytes only; it also sees an authenticated submitter and nonce, and can correlate identical txids. The explicit `notify: [payer, payee]` list is gone from new submissions (F4).
3. P3 and the threat table assume PoW, MMR paths and two gateways (residual Medium). v0.7.2 authenticates sources and compares reachable servers, but does not verify those chain proofs; the residual is High.
4. P7 assumes Tor and our own node; Tor is unbuilt and lightwalletd is a third party.
5. P2, P11 and "payers known and accountable" assume a closed fleet. Open/unknown payers cannot receive pending acceptance; invited pending payments now have count/value caps. Durable sequences increase monotonically with a clock-derived floor.
6. P5 covers BitChat peer IDs, not UDP beacons, BLE advertisements or plaintext TCP links. Hello is still public; a fresh proof gates inventory and signed destination/payload-bound acks cannot be forged by relays.
7. P8 and the assumptions table have no public-testnet column (public lightwalletd, public faucet, minimum-difficulty reorgs).
8. P9/P10 (nullifier sharing/co-signers) remain unbuilt. P12's plaintext broadcast is now implemented for nonce-addressed signed gateway results; legacy results remain directed. Conflict bundles and mainnet fleet-key encryption are unbuilt.
9. The "Light client" leak row doesn't cover a gateway relaying compact actions with no header, the birthday-height leak or the unverifiable frontier.
10. Missing rows: organiser key custody and unsigned invites; local API, XSS and Basecamp isolation; unsigned supply chain and the public APK key; no forward secrecy and plaintext files; the faucet; address bindings; split-transaction fingerprints and fees; Max leaking notes.
11. The fleet-fingerprint section assumes uniform 2-action, 10,000-zat txs. Splits/Max have more actions and correctly pay the larger ZIP 317 fee (Z4); their shape still fingerprints them.
12. It still frames the harness as four Macs with BitChat. Open item 1 (packet capture) must now include the header leaks above.

## Open-mode controls (v0.6.0)

Moved from the Basecamp install guide; the controls date from v0.6.0. v0.7.3
changes only that invited devices trust their fleet's named gateway instead of
pinning the first one they hear.

All inner envelopes (including sync, faucet and submission requests/replies) are
signed by their sender and bound to the intended recipient. Mesh delivery acks
are signed by the original destination. Sealing alone is not authentication.
Gateway announcements and address bindings are individually sealed; only mesh
IDs and public routing keys are broadcast. Presence is immediate on link-up,
refreshes hourly on stable links and expires after three hours (the old v0.6.0
35-second lifetime no longer applies). Gateway loss/change may take an hour to
notice, or up to three hours to expire after missed presence.
Before first use, open devices choose fewest hops, then highest advertised tip,
then mesh ID. **First use pins the gateway's key** in `data/trust.json`, across
restarts. A nearer or newer gateway cannot silently replace it. An unavailable pin
produces a warning and blocks automatic switching. To change operator on purpose,
verify the new operator's mesh ID outside the app, then use Leave session in
Basecamp (v0.7.3), or stop the device and remove only `data/trust.json`. Never
delete `secret.json` or wallet data.

**First contact is still trust-on-first-use, not operator authentication.** A Sybil
gateway can win the initial choice. Hop counts and tips are not independently
proved; **sync, inclusion and mined results are gateway-reported**. Gateway
signatures do not prove chain facts.

Address bindings include a domain-separated Schnorr proof of knowledge of the
Ironwood receiver's incoming-viewing scalar, bound to the exact address and mesh
identity. No viewing or spending key is shared. This proves **receiving authority,
not spending authority**; a holder of a copied incoming viewing key can also prove
it. First receipt routing pins address → mesh key; changed or ambiguous bindings
disable receipt routing and never redirect funds. The typed address always decides
the transaction's destination. This experimental proof implementation has not had
an independent cryptographic review.

Open-mode and unknown payers **never receive pending acceptance**. A payee waits
for its pinned gateway to report inclusion before issuing an `Included` receipt.
That receipt is not an independently verified inclusion or finality proof.

The loopback web API requires a per-run random token and a loopback Host header;
the UI and CLI obtain the token from the local page. Remote strings are rendered
as text. Keep the API bound to `127.0.0.1`; `--api 0.0.0.0` is for isolated tests,
not a secure network API. There is no protection from programs running as your user.

The Regtest faucet allows one request per device per 60 seconds (persisted over
gateway restarts), at most 10 notes and the configured total cap (default 1 ZEC).
New identities can evade the limit: it is not Sybil protection. v0.5.x nodes
ignore unsupported v0.6.0 messages without crashing, but v0.6.0 and later reject
unsigned legacy envelopes and acks, so payment and sync endpoints must upgrade
together.
