# Privacy and threat model

Amounts and recipients are well protected: Ironwood hides them on chain, and BitChat private messages hide the payment details from relays.
Metadata is the weak point: who sent, when and from where leaks through the gateway, mesh peer IDs, radio and fleet-specific transaction shapes.
With four devices the anonymity set is tiny. The design target is mainnet in a shutdown; the local chain and the four Macs are a harness, so no mitigation below is dropped because the harness cannot exercise it. Where the harness cannot test a control, the assumptions table says so and the control is tested with mainnet vectors.

**v0.7.0 trust assumptions by party, with what each party can learn and do and the top risks, are in [Trust assumptions at v0.7.0](#trust-assumptions-at-v070). Where older statements here conflict with that section, it wins.**

Related: [design.md](design.md), [components/double-spend.md](components/double-spend.md), [components/mesh.md](components/mesh.md), [components/gateway.md](components/gateway.md), [components/payee-verifier.md](components/payee-verifier.md). Deep sources: [threat_model.md](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), [report §Metadata](../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md), [reconciliation](../archive/reports/Design%20review%20reconciliation.md).

## Decided

Agreed in the 25 October 2026 reviews ([reconciliation](../archive/reports/Design%20review%20reconciliation.md) G1, G2, G4, G6).

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
| On chain | ZIP 315 recommends anchor H−3 for its wallet policy; expiry defaults are version/height dependent: pinned Common 2.2.0 uses target+40, NU7 upstream default target+120 (~50 min). Long expiry/old anchors distinguish classes, not proof a tx was offline; fees also fingerprint | [ZIP 315](https://zips.z.cash/zip-0315), [ZIP 218](https://zips.z.cash/zip-0218), [ZIP 317](https://zips.z.cash/zip-0317), [prior art](prior-art.md) |
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

Ratings are qualitative (from [threat_model.md §7](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), updated for the BitChat design).

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

The design is written for the right-hand column. The left-hand column says only what the harness can exercise.

| Assumption | Harness (local Regtest, 4 Macs) | Mainnet (design target) |
|---|---|---|
| Payers are known and accountable | Closed fleet | Holds only inside a fleet; fails with strangers |
| Gateway is honest | The design does not assume it, here or on mainnet. The harness gateway is a Mac we control, so lying-gateway tests inject forged bundles deliberately | Assume ADV-G: every bundle fact verified; ≥ 2 gateways |
| Proof of work on headers | Cannot be exercised: Regtest has `with_disable_pow(true)` (issue log #8). Checked with mainnet header vectors under the `regtest-no-pow` flag ([chain.md](components/chain.md)) | Equihash and difficulty against a pinned floor on every header the payee accepts |
| Gateway can reach the network | Loopback to `ths` | Intermittent; Tor is often blocked in shutdowns, so the gateway may be exposed or absent |
| Uplink comes before expiry | Explicit mining exercises block age, not real outage availability | Profile not selected: proposed 3-day courier assumes ≤1-day uplink; proposed 21-day shutdown assumes ≤7 days. Neither covers arbitrary shutdowns. Re-issue needs A even under 2-of-2; debt remains ([expiry/re-issue](components/expiry-reissue.md)) |
| No radio adversary | BLE, one room | Fails: ADV-L can locate and jam transmitters |
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
- Threat note §1 says NU7 "not yet active": NU7 is active from height 1 on the local chain and from 3,543,405 on mainnet.
- This file, 25 October morning, P6 "FROST participants hold the escrow FVK": not required (R1, R2); the co-signer works from the sighash alone.

## Trust assumptions at v0.7.0

**v0.6.0 implementation update:** the review below is preserved at its stated
v0.5.1 source revision. v0.6.0 signs every inner envelope and mesh ack (R1/R4/S1),
adds loopback API tokens/Host checks and text-only web rendering (S5/OS4), proves
address receiving authority and pins bindings (B5), removes pending acceptance for
open/unknown payers (A2/S6), and pins the first used gateway with change warnings
(G4). [Exact controls and remaining limits](basecamp-install.md#open-mode-trust-and-faucet-limits).
First-contact Sybils, dishonest pinned gateways and unverified chain views remain
risks. Receiving authority is not spending authority, and the new proof code is
experimental and not independently audited.

Written 27 October 2026 from a read of `main` at `3395bd1` (v0.5.1). v0.6.0 (open LAN join, pay any address, Max) and v0.7.0 (public testnet, light-client gateway against public lightwalletd, `included`/`mined` gateway-reported, compact birthday sync, "Prepare for offline" split) were not on `main` then, so their rows are **planned**. A row worked out from the planned design rather than read in code is **inferred**. Line numbers are at `3395bd1`.

Paths: `core/` = `crates/zcash-offline-core/src/`, `mesh/` = `crates/ozm-mesh/src/`, `node/` = `crates/ozm-node/src/`, `kt/` = `android/app/src/main/java/org/zcash/offline/`.

Status: **Enforced** (in code, holds at v0.7.0), **Partial**, **Not** (no control in code), **Designed only** (docs, no code), **Planned** (a v0.6.0/v0.7.0 feature).

### Code facts behind most rows

- **F1. The sender is not authenticated on the mesh.** The link hello sends a claimed peer ID (`mesh/link.rs:157-174`) and the listener accepts any inbound TCP connection (`mesh/link.rs:52-69`); the fleet tag only decides whom a node dials (`mesh/discovery.rs:607`). The packet `sender` is cleartext and unsigned (`mesh/packet.rs:41-70`). The envelope's `from`/`from_key` are claimed (`core/envelope.rs:24-31`) and checked only against that claimed peer ID (`node/app.rs:531-536`). Only `Statement`, `Receipt` and `GatewayResult` are signed; `Sub`, `SyncReq`, `SyncRes`, `FundReq`, `FundRes` and mesh acks are not (`core/messages.rs:16-32`, `mesh/node.rs:366-381`).
- **F2. Offline chain views are unverified.** `SyncRes` blocks are applied after a role check only (`node/app.rs:311-336`, `core/wallet.rs:140-214, 242-270`). That view feeds anchors, `chain_nullifiers` and balances (`core/verify.rs:121-136`).
- **F3. `mined` is one gateway's word, and it sticks.** One signed `GatewayResult` sets `mined` and nothing moves it back (`node/app.rs:811-829`). Any node error containing "already" counts as success (`node/app.rs:908`).
- **F4. The gateway learns payer and payee.** Every `Sub` carries `notify: [payer, payee]` (`node/app.rs:495, 703, 855`).
- **F5. A refused payment is still submitted.** A sends `Sub` together with `Pay` (`node/app.rs:487-505`), so a refused payment can still mine; the app treats `refused` as final (`node/app.rs:68-70, 749-755`).
- **F6. The organiser holds every key.** `fleet-init` generates every device's spending and Ed25519 keys in the organiser's folder (`node/lib.rs:142-175`, `core/fleet.rs:210-217`); invites carry raw private keys (`node/lib.rs:109-113`); fleet file and invites are unsigned (`node/lib.rs:196-214`).
- **F7. Remote text reaches `innerHTML`.** Device names, refusal reasons and gateway rejection reasons are rendered with `innerHTML` (`node/ui.html:94, 98-102`; `node/app.rs:744-757, 816-819`). The HTTP API on `127.0.0.1:7780` has no authentication (`node/lib.rs:62, 288-336`).
- **F8. The mesh store has no limits.** Unbounded, kept 24 h, rewritten on every packet (`mesh/node.rs:284-303`, `mesh/lib.rs:92`); every new link receives every stored packet (`mesh/node.rs:208-214, 400-416`).
- **F9. Locked notes never unlock on expiry,** only when seen spent (`core/wallet.rs:207-212`).
- **F10. Keys and wallet data are plain files.** `secret.json` is plaintext, mode 0600 (`node/lib.rs:185-193`); `wallet.json`, `payments.json` and the mesh store use default permissions (`core/wallet.rs:279-287`). The X25519 key derives from the static Ed25519 key, so no forward secrecy (`core/fleet.rs:125-128`). Signatures use `verify`, not `verify_strict` (`core/fleet.rs:162-166`).

### 1. Payer (Alice)

| # | Assumption | S/P | What breaks if violated | Likelihood / impact | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| A1 | The payer does not double-spend a pending payment | S | Same note to B and C (no nullifier sharing), or an online sweep after B's receipt; editing `wallet.json` clears `locked_by` | High / ≤0.5 ZEC per payment, unlimited count | Note lock `core/wallet.rs:39-41`, `core/builder.rs:146-148` (honest app only); same-payee conflict `node/app.rs:651-655`; floor `core/verify.rs:137-139`, `core/fleet.rs:39` | **Partial** (floor only) | Tasks 5–6 before pending outside a closed fleet; co-signers in v1.1 |
| A2 | The payer is known and accountable (P2) | S | Open-mode statement keys are throwaway: no debt claim. **Regression** | High / High | Statement signature `core/messages.rs:49-62`, fleet-only signer `core/messages.rs:135-139` | **Not** for open mode (planned, inferred) | v0.6: no pending for unknown payers; wait for `included` |
| A3 | The payer's anchor is a real chain root | S | Colluding payer and view source (forged `SyncRes`) get a fabricated note accepted | Medium (High in open mode) / ≤ floor | Anchor must be in the payee's view `core/verify.rs:121-127`, but the view is unverified | **Not** (tasks 7–8 deferred) | Sign `syncr` now; MMR paths and PoW before mainnet |
| A4 | The statement matches the transaction | S | Amount, payee or txid mismatch | — | `node/app.rs:630-644` | **Enforced** | — |
| A5 | The payer's device protects its spending key | S | Theft from `secret.json`, the organiser's copy (F6) or the invite in transit | Medium / High | 0600 `node/lib.rs:185-193`; Android backup off | **Partial** | Keystore/Keychain and on-device keys before mainnet |
| A6 | Synced notes and balance are true | S | Lying gateway or lightwalletd shows fake notes or hides real ones | Medium / Medium | None (F2) | **Not** | As B1 |
| A7 | A refusal means the payment won't settle | S | Payee refuses but A already submitted (F5); A pays again, pays twice | Medium / Medium | Locks persist after refusal | **Not** | Show "refused, may still mine until expiry"; or submit after receipt |
| A8 | Withheld transactions release their notes | S | Gateway or lightwalletd swallows the tx; notes locked for good (F9), fleet expiry ~21 days | Medium / Medium | Retries `node/app.rs:836-868` | **Not** | Unlock after verified expiry |
| A9 | Paying B doesn't reveal that A paid B | P | Packet header shows sender and recipient; the 12 KiB bucket marks `pay` (`mesh/packet.rs:41-70`, `core/envelope.rs:20, 50`) | High / Medium | Sealed payload | **Not** | Fixed size, recipient hiding, cover traffic before mainnet |
| A10 | Max doesn't leak the payer's state | P | Max spends ≤2 notes minus fee (`core/builder.rs:48-71`), so B learns note inventory | Low / Low | — | **Planned** | Accept or warn |

### 2. Payee (Bob)

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| B1 | B's chain view (tip, anchors, nullifiers) is real | S | Forged `SyncRes` or lying lightwalletd: fake-anchor or spent payments accepted; `reset=true` wipes the view (`node/app.rs:318-320`); the birthday frontier can't be checked (planned) | High in open mode / High | Role check only `node/app.rs:312` | **Not** (tasks 7–9 deferred) | v0.6: sign `syncr`; two-source agreement; tasks 7–8 before mainnet |
| B2 | Pending exposure is bounded | S | No per-payer count/value cap, payee total, freshness or sharing | High / ≤0.5 ZEC × N | Floor; expiry margin, expiry 0 refused `core/verify.rs:128-139` | **Partial** | Task 5 (caps), task 6 (sharing) |
| B3 | `included`/`mined` are true | S | Gateway or lightwalletd claims confirmations it never saw; one result sticks (F3) | Medium / High | Fleet-gateway signature `node/app.rs:811-813`; labelled gateway-reported | **Not** (gateway-reported by design) | Tasks 7–9 and a second gateway before mainnet |
| B4 | Statements come only from enrolled payers | S | Forged statement | — | `core/messages.rs:135-139`, `node/app.rs:632` | **Enforced** (fleet); open mode **planned** | — |
| B5 | Address → mesh-ID bindings are proved by the address owner | S+P | A stranger binds the victim's address to its own mesh ID, receives the sealed `pay` and sends fake receipts; a name→address lookup redirects funds. **Regression** | High / High (inferred) | None yet | **Planned** | Address-owner proof, pin on first use, show the address not just a name. v0.6 blocker |
| B6 | B stores the tx before the receipt | S | Lost payment | — | fsync first `node/app.rs:672-699` | **Enforced** | — |
| B7 | Forwarding to the gateway hides who was paid | P | `notify` names payer and payee (F4) | High / Medium | — | **Not** (contradicts P1) | `request_nonce` design (`components/gateway.md`) |
| B8 | B's address doesn't link payments | P | One static address per device (`core/fleet.rs:129-131`) | High / Low–Medium | Shielded on chain | **Not** | Diversified address per request |

### 3. Relays and other mesh devices

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| R1 | Relays forward and don't drop | S | Black-hole relay; a forged ack (needs only the cleartext `msg_id`) deletes a message mesh-wide (`mesh/node.rs:347, 366-381`) and marks a payment delivered (`node/app.rs:568-581`) | Medium–High / Medium | Flooding, 24 h store, app retries | **Partial** | Authenticated acks. v0.6 |
| R2 | Relays can't read payloads | P | — | — | X25519 + ChaCha20-Poly1305, 4 KiB padding `core/envelope.rs:47-95` | **Enforced** | — |
| R3 | Past traffic stays private if a key leaks | P | Static key, no forward secrecy; relays keep sealed packets 24 h | Medium / High | — | **Not** | Key rotation or ratchet before mainnet |
| R4 | Relays can't impersonate a device | S | Spoof the gateway's `SyncRes`/`FundRes` or forge `Sub` (F1) with public keys only; anyone in open mode | High / High | Payment, receipt and result are signed | **Not** for unsigned bodies | Sign every inner envelope. v0.6 blocker |
| R5 | Relays can't exhaust resources | S | Floods, far-future `created_ms`, store rewritten per packet (F8) | Medium / Medium | — | **Not** (quotas designed only) | Per-sender quotas, skew cap. v0.6 |
| R6 | Relays don't learn the mesh's traffic | P | Every new link gets every stored packet and seen ID (F8) | High / Medium | — | **Not** | Inventory sync only with authenticated peers |

### 4. Gateway device

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| G1 | The gateway submits what it receives | S | Withholds or delays; in open mode the nearest announcer wins | Medium / High | A and B both submit `node/app.rs:494-496, 702-704`, retries | **Partial** | Two or more gateways; detect withholding by inclusion proof |
| G2 | Chain reports are honest | S | Fake `included`/`mined` | Medium / High | Signature only `core/messages.rs:90-99` | **Designed only** | Tasks 7–9 before mainnet |
| G3 | Sync data is real | S | Poisoned compact blocks, wrong birthday frontier | Medium / High | — | **Not** (F2) | As B1 |
| G4 | The gateway is legitimate | S+P | v0.5.1: organiser-granted role (`node/lib.rs:222-224`, `node/app.rs:813`). v0.6: self-signed announcement, nearest wins: Sybil or eclipse. **Regression** | High / High (inferred) | Fleet role | **Planned** | Fleet-signed gateway certificates; open mode needs two agreeing gateways |
| G5 | The gateway doesn't learn payer and payee | P | Learns both (F4) | High / Medium | — | **Not** | Nonce-addressed acks |
| G6 | The gateway isn't a first spy and doesn't learn interest | P | Sees every tx first and each device's sync height (`node/app.rs:263-268`); v0.7 adds birthdays | High / Medium | — | **Not** (Tor, task 10, unbuilt) | Accept on testnet; Tor before mainnet |
| G7 | The honest path is correct | S | The "already" match can show a conflicted tx as `in-mempool` forever (F3, inferred) | Medium / Medium | — | **Not** | Exact error codes. v0.7 |
| G8 | The gateway holds no wallet | P | At v0.7 any internet device is a gateway and a wallet; lightwalletd first-spies its own txs with its IP | High / Medium (inferred) | — | **Not** | Separate gateway identity, or Tor |
| G9 | The gateway can't be flooded | S | Unlimited `Sub` proof checks `node/app.rs:763-807` | Medium / Low | — | **Designed only** | Quotas |

### 5. lightwalletd server operator

testnet.zec.rocks; the Zaino backup returned HTTP 503 on 27 October 2026.

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| L1 | It returns the true chain | S | Fabricates compact actions, tree state, nullifiers and confirmations for the whole fleet; compact blocks carry no header and GetTreeState only a frontier, so no PoW check. **Regression** from an own node | Low–Medium / High | — | **Not** | A second independent server compared on every answer; own node plus tasks 7–8 before mainnet |
| L2 | It forwards `SendTransaction` | S | Censors while replying OK | Low–Medium / Medium | — | **Not** | Send to two or more servers; confirm via another source |
| L3 | It doesn't deanonymize the fleet | P | Sees the gateway IP, every fleet tx with timing, birthdays, sync ranges, txid queries (inferred) | High / Medium | TLS on the path | **Not** | Tor per tx; status from blocks, not per-txid queries |
| L4 | It is available | S | Single point of failure; the backup returns 503 | Medium / High | — | **Not** | Multiple endpoints |
| L5 | TLS authenticates the server | S | Mis-issued certificate or DNS hijack | Low / High | WebPKI (planned) | **Planned** | SPKI pinning |

### 6. Zcash network, miners and testnet

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| Z1 | Consensus mines at most one conflicting spend | S | — | — | Duplicate nullifiers rejected by nodes | **Enforced** (network) | — |
| Z2 | 10 confirmations mean final | S | Testnet minimum-difficulty blocks after 450 s gaps make cheap reorgs feasible | Medium / Low (test coins) | — | **Not** | Testnet is a functional test only |
| Z3 | Common 4.0 output is valid on public testnet | S | All payments rejected | Medium / Medium | Accepted on Regtest only | **Planned** | Mined testnet smoke test before tagging v0.7.0 |
| Z4 | The fleet fee fits every transaction shape | S | Split txs with more than 2 actions need more than 10,000 zat (ZIP 317), but the gateway requires exactly the fleet fee (`core/verify.rs:75-78`) | High / Medium (inferred) | — | **Not** | Fee policy per action count |
| Z5 | On-chain privacy | P | Fingerprints: 72,576-block expiry (`core/fleet.rs:37`), anchor age, 2-action shape, split outputs | High / Medium | Accepted (P4) | **Partial** | Accept on testnet |
| Z6 | Testnet stays live through the expiry window | S | A reset or upgrade kills unmined txs | Low / Medium | — | **Not** | — |

### 7. Organiser and invite issuer (fleet mode)

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| O1 | The organiser doesn't keep or misuse device keys | S+P | Holds every key (F6): steals all funds, signs as anyone, decrypts every envelope | Low / High | A doc warning only | **Not** | Keys generated on the device; organiser signs public enrollment only |
| O2 | Invites stay confidential in transit | S | An intercepted invite gives every key | Medium / High | 0600 on disk | **Not** | As O1 |
| O3 | Fleet file and invites are authentic | S | A tampered `fleet.json` swaps payee addresses or gateway keys | Low–Medium / High | Consistency only `node/lib.rs:196-214` | **Not** | Organiser signature and fingerprint check |
| O4 | Policy values are sane | S | Bad floor, fee, branch or expiry (`core/fleet.rs:15-44`) | Low / Medium | — | **Partial** | Signed policy |
| O5 | Only trusted devices get the gateway role | S | Rogue gateway | Low / High | `node/app.rs:170-176, 813` | **Enforced** (fleet) | — |

### 8. Open-mode strangers on the LAN or BLE mesh

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| S1 | Strangers can't inject traffic | S | Already possible at v0.5.1 (inbound 7777, store harvest); with public identities they spoof the gateway and acks. **Regression** | High / High | — | **Not** | Authenticated links and envelopes. v0.6 blocker |
| S2 | No Sybil | S | Unlimited identities: nearest-gateway hijack, eclipse | High / High | — | **Planned / Not** | Pinned or fleet gateways, two-source agreement, rate limits |
| S3 | Names and addresses aren't impersonated | S | Self-chosen "Bob", binding spoof (B5) | High / High (inferred) | — | **Planned** | Key fingerprints, pin on first use |
| S4 | Membership and topology stay hidden | P | Beacons send tag and peer ID every 2 s (`mesh/discovery.rs:338-340, 503-533`), plus the BLE hint and IPs | High / Medium | — | **Not** | Rotating IDs (P5) |
| S5 | Remote strings can't run in the UI | S | Stored XSS (F7) → `fetch('/api/pay')` drains the wallet to any address. **Regression** | Medium / High | — | **Not** | `textContent` everywhere, API token. v0.6 blocker |
| S6 | Pending is never offered to strangers | S | A1 with no accountability | Depends on v0.6 / High | Fleet-only statements | **Planned** | Rule: no pending for unknown keys |

### 9. The faucet

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| FA1 | The public faucet doesn't link requesters | P | Learns requester IP and address | High / Low | — | **Not** | Request via the gateway or Tor, fresh address |
| FA2 | The faucet pays Ironwood addresses on NU7 testnet | S | No funds reach devices | Medium / Medium | Untested | **Planned** | Test before v0.7.0 |
| FA3 | The gateway faucet can't be drained | S | Per-request cap only (`node/app.rs:362`) | — | Off unless `mine` (`node/app.rs:359`), so off on testnet | **Enforced** (off) | — |

### 10. Software distribution

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| D1 | Catalogue and packages come from us | S | Catalogue on `main` points at mutable `releases/latest`; packages unsigned (`trustedSigners: []`); a compromised account ships a key-stealing module | Low / High | Content roots and SHA-256 (integrity only) | **Not** | Sign `.lgx`, pin versions before mainnet |
| D2 | The APK signing key is secret | S | Public test key (`android/test-signing.p12`): anyone can ship an update inheriting app data and keys | Medium / High | Documented warning | **Not** | Private release key and new package ID before real value |
| D3 | The CLI installer is authentic | S | Checksum from the same release (`scripts/install.sh:70-91`); macOS binary unsigned | Low / High | — | **Partial** | Signed releases |
| D4 | CI isn't compromised | S | Actions pinned to tags, `@stable` toolchain, shared cache (`.github/workflows/release.yml:64-67`) | Low / High | — | **Not** | Pin SHAs, isolate release cache |
| D5 | The Basecamp host isolates modules | S | Other modules call `pay` over Logos IPC (inferred) | Low–Medium / High | — | **Not** (inferred) | Caller allow-list |

### 11. Passive network or radio observer

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| PO1 | The mesh hides who pays whom | P | Cleartext sender/recipient, size and time; `sub` timing links the txid to A and B | High / Medium | Sealed payload | **Not** | Uniform size, cover traffic |
| PO2 | BLE doesn't track devices | P | Static 4-byte fleet hint (`kt/BleLink.kt:214`, `crates/ozm-ble/src/linux.rs:176`), unencrypted GATT (`kt/BleLink.kt:242`), stable peer ID | High / Medium | No device name | **Not** | Rotating hint and ID |
| PO3 | The LAN doesn't leak | P | Beacons and plaintext TCP | High / Medium | — | **Not** | Link encryption |
| PO4 | The uplink doesn't leak | P | Gateway IP and tx timing at lightwalletd | High / Medium | TLS | **Not** | Tor |
| PO5 | No active radio adversary | S+P | Jamming, direction finding | Low–Medium / High | Accepted | **Not** | — |

### 12. Device OS and platform

| # | Assumption | S/P | What breaks if violated | L / I | Existing mitigation | Status v0.7.0 | Fix and when |
|---|---|---|---|---|---|---|---|
| OS1 | The OS RNG is sound | S | — | — | `core/fleet.rs:91-108` | **Enforced** (OS) | — |
| OS2 | Key storage resists malware and seizure | S | Plaintext `secret.json` | Medium / High | 0600; Android backup off | **Partial** | Keystore, Keychain, Secure Enclave |
| OS3 | Wallet and history files are private | P | Default permissions (F10) | Medium / Medium | — | **Not** | 0600 |
| OS4 | Only the user can call the local API | S | Any local process or DNS-rebinding page calls `/api/pay` (inferred; no Host/Origin check); with pay-any-address it pays anyone. **Regression** | Medium / High | Loopback bind `node/lib.rs:62` | **Not** | Random token and Host check. v0.6 blocker |
| OS5 | The Android sandbox holds | S | `usesCleartextTraffic=true` | Low / Low | `allowBackup=false`, only the launcher exported | **Partial** | — |
| OS6 | The clock is roughly right | S | Time-based `seq` (`node/app.rs:427`), mesh expiry (`mesh/node.rs:275-310`) | Low / Medium | — | **Partial** | — |
| OS7 | A seized device doesn't expose past traffic | P | No forward secrecy | Medium / High | — | **Not** | Key rotation |

### What each party can learn

| Party | Amounts | Addresses | Sender ↔ receiver | Timing | Location / topology | Balance |
|---|---|---|---|---|---|---|
| Payer | Own | Payee's | Own | Yes | Payee's hop count | No |
| Payee | Paid to it | Payer's (fleet file) | Yes | Yes | Payer's hop count | Partly, via Max (planned) |
| Relay | Size bucket only | No | **Yes** (header IDs) | Yes | **Yes** | No |
| Gateway | No | Via `notify` and the fleet file | **Yes** (F4) | Yes, first spy | Partly | No |
| lightwalletd | No | No | No, but sees the fleet's whole tx set | Yes | **Gateway IP** | No |
| Network / miners | No | No | Nullifier linkage on chain | Yes | No | No |
| Organiser | **Yes** | **Yes** | **Yes** | With captures | If on the mesh | **Yes** (holds keys) |
| Open-mode stranger | Size only | Bindings (planned) | **Yes** | Yes | **Yes**, plus IPs | No |
| Faucet | Its payout | Requester's | No | Yes | Requester IP | No |
| Software distribution | Everything, via a malicious build | | | | | |
| Passive observer | Size only | No | **Yes** | Yes | **Yes**, RF location | No |
| Device OS | Everything on the device | | | | | |

### What each party can do

| Party | Steal | Double-spend | Censor | Delay | Lie about mined | Deny service | Impersonate |
|---|---|---|---|---|---|---|---|
| Payer | No | **Yes** (pending) | — | Yes | No | Spam | Throwaway IDs in open mode |
| Payee | No | No | Partly (A also submits) | Yes | No (can falsely refuse) | — | No |
| Relay | No | No | **Yes** (drop, forged acks) | Yes | Fake view via `syncr` | Yes | **Yes** (unsigned bodies) |
| Gateway | No | Can hide conflicts | **Yes** | **Yes** | **Yes** | Yes | Open-mode announcement |
| lightwalletd | No | Can hide conflicts | **Yes** | Yes | **Yes**, fleet-wide | **Yes** | It is the chain |
| Network / miners | No | Enables it (testnet reorgs) | Yes | Yes | Reorgs undo confirmations | Yes | No |
| Organiser | **Yes** | **Yes** | Yes | Yes | **Yes** | Yes | **Yes** |
| Open-mode stranger | **Yes**, via XSS or the API | If offered pending | Yes | Yes | If picked as gateway | Yes | **Yes** |
| Faucet | No | No | Withholds funds | Yes | No | — | No |
| Software distribution | **Yes** | **Yes** | Yes | Yes | Yes | Yes | **Yes** |
| Passive observer / jammer | No | No | Jamming only | Jamming only | No | Jamming only | No |
| Device OS | **Yes** | **Yes** | Yes | Yes | Yes | Yes | **Yes** |

### Top 10 risks at v0.7.0

1. **One public lightwalletd is the whole chain** (L1–L4, B1, B3): no headers, no PoW, no second source; it decides balances, anchors, nullifiers and `included`/`mined` for every device, sees every tx and the gateway IP, and is a single point of failure. **Regression.**
2. **Unauthenticated mesh sender and unsigned sync** (F1, F2, R1, R4, S1): forged `syncr` gets fabricated-anchor payments accepted; forged acks delete messages mesh-wide. Any fleet member at v0.5.1; **anyone** in open mode.
3. **Identity and binding spoofing in open mode and pay-any-address** (G4, B5, S2, S3). **Regression**, planned.
4. **The local UI and API can be driven remotely** (F7, OS4, S5): `innerHTML` plus an unauthenticated API means direct theft with pay-any-address. **Regression.**
5. **Pending double-spend has no caps or accountability** (A1, A2, B2). **Regression** with open mode.
6. **The organiser holds every key; invites are raw and unsigned** (O1–O3).
7. **Gateway lies, censorship and stuck funds** (G1–G3, G7, A7, A8): sticky `mined`, the "already" match, refused-but-submitted payments, locks that never expire. **Regression**: any internet device can be a gateway.
8. **Metadata leaks** (A9, B7, G5, G6, R6, PO1–PO4).
9. **Unsigned supply chain** (D1–D4); fix before any real value.
10. **Testnet chain and v0.7 mechanics** (Z2–Z4, FA2): cheap reorgs, untested Common 4.0 on testnet, split txs breaking the exact-fee rule, faucet support unverified.

### Statements earlier in this file that are out of date for v0.7.0

The sections above were written for the BitChat-fork design. Until they are rewritten, this section wins where they conflict:

1. The intro and P1 describe BitChat/Noise private messages. The transport is `ozm-mesh` with `ozm1` envelopes, whose headers expose sender and recipient.
2. P1 says the gateway gets tx bytes only; the code also sends `notify: [payer, payee]` and the claimed `from` (F4).
3. P3 and the threat table assume PoW, MMR paths and two gateways (residual Medium). At v0.7.0 nothing is verified; the residual is High.
4. P7 assumes Tor and our own node; Tor is unbuilt and lightwalletd is a third party.
5. P2, P11 and "payers known and accountable" assume a closed fleet; open join and pay-any-address break this. `seq` is time-based (`node/app.rs:427`), not a count.
6. P5 covers BitChat peer IDs, not UDP beacons, the static BLE hint, plaintext TCP links, inventory harvesting or unauthenticated hello and acks.
7. P8 and the assumptions table have no public-testnet column (public lightwalletd, public faucet, minimum-difficulty reorgs).
8. P9, P10 and P12 (nullifier sharing, co-signers, broadcast acks) are not built; gateway results are directed messages.
9. The "Light client" leak row doesn't cover a gateway relaying compact actions with no header, the birthday-height leak or the unverifiable frontier.
10. Missing rows: organiser key custody and unsigned invites; local API, XSS and Basecamp isolation; unsigned supply chain and the public APK key; no forward secrecy and plaintext files; the faucet; address bindings; split-transaction fingerprints and fees; Max leaking notes.
11. The fleet-fingerprint section assumes uniform 2-action, 10,000-zat txs; split txs break that (Z4).
12. It still frames the harness as four Macs with BitChat. Open item 1 (packet capture) must now include the header leaks above.
