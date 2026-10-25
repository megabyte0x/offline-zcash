# Privacy and threat model

Amounts and recipients are well protected: Ironwood hides them on chain, and BitChat private messages hide the payment details from relays.
Metadata is the weak point: who sent, when and from where leaks through the gateway, mesh peer IDs, radio and fleet-specific transaction shapes.
With three devices the anonymity set is tiny. MVP v1 is a closed fleet on a local chain; several of these risks only become real in a shutdown on mainnet.

Related: [design.md](design.md), [components/double-spend.md](components/double-spend.md), [components/transport-bitchat.md](components/transport-bitchat.md), [components/gateway.md](components/gateway.md), [components/payee-verifier.md](components/payee-verifier.md). Deep sources: [threat_model.md](../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/threat_model.md), [report §Metadata](../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md), [reconciliation](../archive/reports/Design%20review%20reconciliation.md).

## Decided

Agreed in the 25 October 2026 reviews ([reconciliation](../archive/reports/Design%20review%20reconciliation.md) G1, G2, G4, G6).

| # | Finding | Decision |
|---|---|---|
| P1 | The old step-3 public broadcast of a signed intent (nullifiers, txid, payee, amount, expiry, payer key) was a plaintext payment record; BitChat public payloads are signed, not encrypted | **Fixed:** relays and the gateway get transaction bytes only; details and accountability signature go to the payee over a Noise-encrypted private message [decided] |
| P2 | Identity-keyed controls (per-payer counters, accountability signatures) link every payment to a stable identity | Closed, known fleet only; never with strangers. Hot lists hold nullifiers, not payer keys [decided] |
| P3 | The gateway is the first broadcaster (first-spy) and a trust point: it can withhold, delay, forge chain state and sees every transaction first | Accept in MVP v1; it holds no spending or viewing key. Mitigations in [gateway.md](components/gateway.md) [decided] |
| P4 | Fleet transactions are fingerprintable: stale anchor, long expiry delta, uniform padding, bursts at each uplink | Accept: anonymity set is "fleet users", not "all Zcash users". Uniform policy so the fleet at least blends with itself [decided] |
| P5 | BitChat stable peer IDs and neighbour announcements leak topology; with 3 devices the anonymity set is tiny | Accept in the demo [decided] |
| P6 | Committees and FROST participants hold the escrow FVK, so they see amounts and recipients | Escrow payee sees the escrow FVK by design [decided] |
| P7 | lightwalletd sees the submitter's IP and timing | Gateway over Tor for mainnet (MVP v2) [decided] |
| P8 | ths dev accounts derive from all-zero entropy (`schema.rs` L42), so their keys are public | Fine for Regtest; never hold real value with them [decided] |

## Findings

### What leaks, by layer

| Layer | Leak | Source |
|---|---|---|
| On chain | Nullifiers, note commitments, action count, anchor, expiry height, `valueBalance`, exact size; no fee field. 820 B per action | [ZIP 229](https://zips.z.cash/zip-0229), [ZIP 225](https://zips.z.cash/zip-0225) |
| On chain | Ironwood formal verification covers soundness and balance, not privacy | [crypto.news](https://crypto.news/zcash-ironwood-upgrade-whats-changed-after-the-orchard-bug/) |
| On chain | Standard wallets use anchor H−3 and expiry H+40; a stale anchor or long delta marks "built offline". Non-standard fees reveal wallets | [ZIP 315](https://zips.z.cash/zip-0315), [ZIP 317](https://zips.z.cash/zip-0317) |
| Broadcast | The first node to broadcast is the apparent origin; Zcash txs deanonymized from timing "with a high level of accuracy" (pre-Zebra) | [Biryukov & Tikhomirov 2019](https://orbilu.uni.lu/bitstream/10993/39724/1/biryukov-tikhomirov-deanonymization-and-linkability) |
| Broadcast | Long-lived nodes over Tor face exit-banning and fingerprinting; a global passive adversary defeats Tor | [Biryukov & Pustogarov 2015](https://arxiv.org/abs/1410.6079), [Kappos & Piotrowska 2019](https://arxiv.org/pdf/1902.07337) |
| Light client | lightwalletd learns IP, sync height and full-tx fetches; may show fake payments if headers are not validated | [ZIP 307](https://zips.z.cash/zip-0307), [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf) |
| Keys | A UFVK reveals incoming, outgoing and change history; a UIVK only incoming. FROST participants share the FVK | [ZIP 316](https://zips.z.cash/zip-0316), [ZF FROST book](https://frost.zfnd.org/zcash/technical-details.html) |
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
| Payer double-spends while offline | Pending tier, floor limit, final only when mined; escrow next | High at pending |
| Gateway first-spy and trust point | Tor, fresh circuit per tx (mainnet); several gateways; PoW-validated headers from ≥2 sources | Medium |
| Fleet fingerprint (anchor, expiry, padding, uplink bursts) | Freshest anchor; one fleet expiry; standard 10,000-zat fee; 2-action padding; spread broadcasts | Medium |
| Mesh topology and origin via stable peer IDs and announcements | Private messages only; same-size traffic | Medium–High (tiny set) |
| Sybil/eclipse feeding fake chain data | Known peers; header validation; ≥2 gateways | Medium–High |
| RF direction finding, fingerprinting, jamming | Low power, short bursts, multiple paths | High against a determined local adversary |
| Seed restored on a second device | One spending device per account | Looks exactly like a double-spend; conflict evidence cannot tell |
| Device compromise or seizure | Full-disk encryption; minimal history; no keys on relays or gateway | Medium |
| Viewing-key exposure | Never share a UFVK; UIVK only for watch-only | Medium |
| Operator legal exposure | Commodity hardware, anyone can be a gateway | Medium–High in hostile jurisdictions |

### Assumptions: MVP v1 versus a real shutdown

| Assumption | MVP v1 (local Regtest, 3 Macs) | Real shutdown on mainnet |
|---|---|---|
| Payers are known and accountable | Holds (closed fleet) | Holds only inside a fleet; fails with strangers |
| Gateway is honest | Holds (own laptop) | Fails: must assume ADV-G |
| Gateway can reach the network | Holds (local chain) | Intermittent; Tor is often blocked in shutdowns, so the gateway may be exposed or absent |
| Uplink comes before expiry | Holds | Often fails: a 3-day expiry is short against real outages (G8); re-issue needs the payer |
| No radio adversary | Holds (BLE, one room) | Fails: ADV-L can locate and jam transmitters |
| Anonymity set | Irrelevant (worthless coins) | Fleet users only; small |
| Keys | Public dev keys (all-zero entropy) | Must be real seeds, hardware or FROST-held |
| Legal exposure | None | Operators may face money-transmission or radio charges |

## Open

1. Test: packet capture shows no plaintext amounts, payees or keys on the mesh (reconciliation §6 test 5).
2. Test: a transaction against a fabricated anchor is rejected by the payee ([payee-verifier.md](components/payee-verifier.md)).
3. [unverified] Whether BitChat private messages leak recipient identity to relays beyond the routing header; peer-ID rotation cost for the outbox.
4. [unverified] Tor reachability from the gateway in a real shutdown; fallback transports (bridges) not evaluated. Source for "often blocked" was the review, not an archive note.
5. [unverified] `schema.rs` L42 all-zero entropy: from the reviews; ths file path and commit not re-checked here.
6. [unverified] Size of the fingerprint: how distinct a fleet expiry delta and anchor age are on mainnet.
7. [unverified] Duress or hidden-wallet support for shielded Zcash on any hardware signer.

### Corrections to archive text

- Report flow step 5 (intent fan-out to gateways) and research note §9 (signed intents gossiped mesh-wide) → payee only (P1).
- Threat note "rotate peer IDs per session" vs report "stable `--nodekey`": both written for Logos; the BitChat demo keeps stable peer IDs and accepts the linkage (P5).
- Report and card note: hot list of payer keys → nullifiers only (P2).
- Threat note §1 says NU7 "not yet active": NU7 is active from height 1 on the local chain and from 3,543,405 on mainnet.
