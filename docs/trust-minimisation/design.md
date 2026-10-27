# Trust minimisation: target architecture

Status: research proposal, 27 October 2026, on branch `research/trust-minimisation`. Nothing here is built or decided; owner decisions are listed at the end. It answers every row of [Trust assumptions at v0.7.0](../privacy-and-threats.md#trust-assumptions-at-v070) and assumes the direct fixes queued for v0.7.0 and v0.7.2 land as briefed (they are not redone here). Method, sources and rejected options: [research-log.md](research-log.md). Build order: [implementation-plan.md](implementation-plan.md).

Labels: **[V]** read in the primary source at the cited revision; **[M]** measured; **[estimate]** arithmetic or modelling by us; **[unverified]** a claim we could not check. Byte costs are per device per day unless stated, at BLE planning rates of 3–100 KB/s per hop.

## The goal in one sentence

A device trusts only the people its user chose to deal with, and the only loss the design accepts without a technical fix is a chosen payer double-spending a pending payment. Everything else is either proved (chain facts), hidden (metadata), held by the user (keys) or verified by many independent parties (software). A short list of residuals that no design can remove is at the end, with the reason each is impossible.

## What changes, in seven parts

```diagram
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│ T4 Social identity: device-made keys, QR pinning, 1-hop introductions, optional         │
│    threshold fleet log. Credit (pending) only to pinned contacts                          │
├──────────────────────────────┬──────────────────────────────┬───────────────────────────┤
│ T5 Bounded credit            │ T1 Self-verifying chain      │ T7 Sealed device          │
│ pending caps, circle         │ release checkpoint, PoW tip  │ keystore-wrapped store,   │
│ nullifier gossip, chosen     │ ticks, FlyClient on conflict,│ optional hardware signer, │
│ escrow co-signers            │ work-priced confirmations,   │ IPC allow-list, chain     │
│ (the accepted residual)      │ MMR/txid/auth-data proofs    │ clock                     │
├──────────────────────────────┴──────────────┬───────────────┴───────────────────────────┤
│ T3 Quiet mesh: Noise-encrypted links, rotating BLE/LAN identifiers synced with the RPA,  │
│    no sender field, rotating recipient hints, preimage acks, fixed cells, jitter,        │
│    ratcheted receive keys (forward secrecy)                                              │
├─────────────────────────────────────────────┴───────────────────────────────────────────┤
│ T2 P2P light gateway: any device, no keys, no trust. Headers-first + whole blocks from   │
│    many Zcash P2P peers over Arti with per-request isolation; builds MMR leaves and      │
│    proofs itself; submits to several peers. lightwalletd becomes an optional extra source│
├─────────────────────────────────────────────────────────────────────────────────────────┤
│ T6 Verifiable distribution: reproducible builds, k rebuilders, t-of-n release signers,   │
│    attestations, signed .lgx, update metadata relayed over the mesh with a chain clock   │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

| Part | Replaces today | Removes trust in |
|---|---|---|
| T1 Self-verifying chain | Unsigned `SyncRes`, gateway-reported `included`/`mined`, the 2-gateway agreement rule | Gateways, lightwalletd, relays, the organiser's checkpoint |
| T2 P2P light gateway | gRPC to one public lightwalletd | lightwalletd operators (L1–L5) |
| T3 Quiet mesh | Cleartext sender/recipient headers, static peer IDs and BLE hint, static X25519 keys, plaintext TCP/GATT | Relays, passive observers, strangers (privacy rows) |
| T4 Social identity | Organiser-generated keys and raw-key invites, nearest-gateway selection | The organiser, strangers, Sybils |
| T5 Bounded credit | Floor-only pending | Nothing new: it bounds the accepted residual |
| T6 Verifiable distribution | Unsigned catalogue, public APK key, same-release checksums | Any single maintainer, CI runner or GitHub account |
| T7 Sealed device | Plaintext `secret.json`, device clock | Other apps, thieves, the clock |

## T1 Self-verifying chain

### Principle

Every chain fact a device acts on is checked against proof of work and Merkle commitments, starting from a checkpoint compiled into the release (so its trust is the supply chain's, T6). Sources are never counted or voted: a device keeps the **heaviest verified chain it has heard from any source**, so one honest reachable source is enough and any number of dishonest ones can only waste bandwidth. This replaces the decided rule "≥ 2 gateways must agree on the tip" ([gateway.md](../components/gateway.md)), which fails open under a Sybil swarm and fails closed with one honest gateway.

### What each fact is proved by

| Fact | Proof | Size (mainnet/testnet) | Notes |
|---|---|---|---|
| Header H is a real block at ≥ floor difficulty | Equihash (200,9) + `nBits` ≥ floor + DAA where predecessors are known | 1,487 B | 234 µs per Equihash verify on a 4-vCPU Xeon [M, research]; phone time [unverified] |
| H is an ancestor of tip T | MMR path for leaf H in the history tree committed by T's `hashBlockCommitments` (needs `authDataRoot(T)`, 32 B) | ≈ 5–7 KB at 2^17–2^21 leaves, 277–317 B per node after ZIP 258 [estimate] | Tree resets at each upgrade (ZIP 221 L150–154 [V]); proofs only since NU7 activation |
| Anchor A is a real Ironwood root at block N | History leaf N (`end_ironwood_root`) under T | ≈ 5–7 KB [estimate] | Already proved on Regtest ([`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md)) |
| Tx X (its effects: nullifiers, cmx, outputs, value, expiry) was mined in block N | Verified compact tx + txid branch to N's `hashMerkleRoot` + header N + ancestry under T | ≈ 0.6 KB (2 actions) + 32·⌈log₂ ntx⌉ + 1.5 KB + 5–7 KB [estimate] | v6 txid = BLAKE2b over component digests; the Ironwood digest has compact/memos/noncompact children (ZIP 229 L375–462, ZIP 244 T.4a–c [V]) |
| Which anchor and proofs X used | `auth_digest` path to `hashAuthDataRoot` (inside `hashBlockCommitments`) | ≈ 32·⌈log₂ ntx⌉ + 32 B | v6 txids **omit** the anchor (ZIP 229 [V]), so a txid branch alone does not prove the anchor |
| A note-commitment frontier at height h is correct (birthday) | Recompute the frontier root and compare with leaf h's `end_ironwood_root` | frontier ≤ 1 KB + 5–7 KB | Replaces trust in `GetTreeState` |
| A note I received is real and spendable | Its tx inclusion (above) + my witness root equals a proved anchor | as above | A wrong witness shows as a root mismatch, never as a silent loss |

Two facts **cannot** be proved from headers on Zcash today: that a nullifier has never been spent (there is no nullifier-set commitment, ZIP 221 L660–663 [V]), and that no heavier chain exists somewhere unseen. The design does not need the first (argued below) and bounds the second with work pricing.

### Verified compact transactions

ZIP 244's `orchard_actions_compact_digest` is defined as "the subset of Orchard Action information intended to be included in an updated version of the ZIP-307 CompactBlock format" — nullifier, cmx, ephemeralKey, `encCiphertext[..52]` [V]; Ironwood copies the structure with `ZTxIdIrnActCH_v6` / `ZTxIdIrnActMH_v6` / `ZTxIdIrnActNH_v6` (ZIP 229 [V]). So a device given the 148-byte compact form of **every** action of a tx, the memos and noncompact digests (64 B), `flagsIronwood` and `valueBalanceIronwood` (9 B) and the four other component digests (128 B) recomputes the txid and checks it against a header. Per tx ≈ 201 B + 148 B per action [estimate]. No ZIP, server or wallet implements this yet [V: lightwallet-protocol v0.6.0 `ec53075` says the CompactBlock `header` "should always be unset"; Zaino 0.10.1 builds it as `Vec::new()`; PR #21 open]. Our gateway produces it itself from whole blocks (T2), so we do not wait for upstream.

### Optimistic tip ticks, proofs on conflict

Shipping every header costs 1,487 B × 3,456 blocks ≈ 5.1 MB per day, too much for BLE. Shipping a full FlyClient proof every day costs ≈ 0.8–1.3 MB [estimate below]. The design avoids both on the common path:

1. **Tick.** A gateway broadcasts a signed-for-dedup but *untrusted* tick: the current header T plus the MMR ancestry path of the device's last accepted tick under T. A device accepts it if the header passes Equihash and the floor, and the previous accepted tick is an ancestor. 4–24 ticks a day cost ≈ 30–180 KB [estimate: 7.5 KB each]. Forging a tick costs one block of real work at ≥ floor difficulty.
2. **Conflict.** If a device holds two ticks that are not ancestor-related (two sources disagree), it asks both sources for FlyClient proofs since the last common ancestor and keeps the heavier ([Bünz, Kiffer, Luu, Zamani, S&P 2020](https://eprint.iacr.org/2019/226) [V]). Cost only on disagreement.
3. **Catch-up.** A device offline longer than one day, or with no accepted tick since its checkpoint, requests one FlyClient proof from any source. Upper bound at λ = 64 ≈ 1.2 MB for 1 day, 2.1 MB for 1 week, 2.8 MB for 21 days before deduplicating MMR paths; ≈ 1.8 MB for 21 days deduplicated [estimate, research]. This is smaller than the 8.55 MB modelled in [gateway.md](../components/gateway.md) because it relies on the difficulty fields in MMR nodes (`nEarliestTargetBits`, `nLatestTargetBits`, timestamps) instead of recomputing the DAA from 113 predecessor leaves per sample; recomputing it restores the larger figure. Proof sizes on Zcash with a real prover: 504–802 samples, ≈ 1.2 MiB gzipped at 3 M blocks ([Perazzo & Capecchi 2026, arXiv 2604.26736](https://arxiv.org/abs/2604.26736) [V excerpts]).
4. **Floor.** Difficulty below half of the difficulty at the device's last verified point is rejected (the decided rule in [pending-tier.md](../components/pending-tier.md)), but the floor now comes from the device's own verified chain and the release checkpoint, not from a fleet-owner signature.

FlyClient's security on a chain with per-block difficulty adjustment is heuristic. ZIP 221 says so directly: FlyClient "provides only heuristic security guarantees in that case … The use of these fields has not been analysed in the academic security literature" (ZIP 221 L784–800 [V]). Variable-difficulty NIPoPoWs now have a claimed proof ([Miller et al., CCS 2025](https://loicmiller.com/publication/mining-in-logarithmic-space-with-variable-difficulty/) [secondhand]) but no Zcash deployment. NU7 also changes the averaging window to 102 blocks (ZIP 218 [V]; the existing docs say 17). The design therefore never relies on FlyClient alone for value: every value decision is **work-priced**.

### Work-priced decisions

A device marks a payment `included` only when the inclusion block is buried under a contiguous suffix of full headers whose total work W satisfies W ≥ κ · value, linked by ancestry to its last verified point and checked against the floor and DAA. The forger must actually mine every suffix header, so a forgery costs at least the block rewards it forgoes. At NU7's 25 s target the block subsidy is ⅓ of the pre-NU7 1.5625 ZEC (ZIP 218 scaling [unverified exact value]); at half difficulty (the floor) each forged header costs roughly 0.2 ZEC of hash rental [estimate; assumes an efficient Equihash rental market, unverified]. With κ = 10, protecting a 3 ZEC pending ceiling needs ≈ 150 headers ≈ 220 KB, fetched once per window and shared by every payment in it [estimate]. `confirmed` keeps the ten-confirmation rule from [payee-verifier.md](../components/payee-verifier.md) as a minimum, raised by work pricing. On testnet, blocks mined under the 450 s minimum-difficulty rule (ZIP 218: "strictly greater" than 18 × 25 s [V]) count as zero work, so testnet decisions are functional tests, never security evidence (row Z2).

### Why nullifier completeness is not needed

The only party that can spend a note is its key holder. A gateway that hides "nullifier nf was already spent" can therefore only help a payer who spent nf elsewhere — a payer double-spend, which is the accepted residual. Every other completeness question is a liveness question for the party who wants the answer:

- **My payment was mined** — inclusion is provable by anyone who has the block; the payer and payee each want it and can fetch it from any source.
- **My incoming note exists** — hiding it is withholding (availability). The sender can push the inclusion proof directly over the mesh.
- **My note is unspent** — only I can spend it; if my own spends are hidden I learn it when my next spend fails, which costs only time.

This removes the strongest reason the old design needed two agreeing gateways. If Tachyon later adds a nullifier accumulator with untrusted non-membership proofs ([Bowe, 29 Sep 2026 design](https://seanbowe.com/blog/tachyaction-at-a-distance) [V]; not in NU7 per ZIP 259 [V]), the payee can also refuse already-spent notes up front; that narrows the accepted residual but is not required.

### Receiving without scanning

A payment over the mesh arrives as tx bytes; the payee decrypts its own output directly and needs only the inclusion proof later. Scanning the whole chain is needed only to discover payments sent from **outside** the mesh. At 116 B per action (cmx, epk, 52-byte ciphertext prefix; nullifiers are not needed because only the owner spends) a BLE device can scan ≈ 8,000 actions a day inside a 1 MB budget [estimate]. Above that, off-mesh discovery moves to Wi-Fi hubs or the next time the device is near a gateway; mesh payments are unaffected. The broadcast is identical for every device, so scanning reveals no interest ([ZIP 307](https://zips.z.cash/zip-0307): "it leaks no information about which transactions the light client is interested in" [V]). Requests for missed blocks name a fixed height bucket and carry no sender identity.

**Witnesses.** To spend a received note a payer needs its Merkle path. Above the scanning budget, gateways broadcast level-6 subtree roots (0.5 B per action) and serve whole 64-leaf buckets on anonymous request (2 KB); a gateway learns only that someone wants one of the bucket's 64 positions, and the device checks the rebuilt root against a proved anchor [estimate; bucket size is a tunable privacy/bandwidth trade].

### Budget for T1

| Item | Who receives it | Bytes per day [estimate] |
|---|---|---|
| Tip ticks | everyone | 30–180 KB |
| FlyClient catch-up | devices offline > 1 day, or on conflict | 0.8–1.3 MB per event |
| Work-priced suffix | payees with open payments | ≈ 220 KB per window |
| Inclusion + auth-data proof | payer and payee of each payment | ≈ 8–9 KB per payment |
| Anchor proof | payee per new anchor | ≈ 5–7 KB |
| Scanning stream (optional) | devices that want off-mesh discovery | 116 B per action |

A device that stays in touch with any gateway and holds a few payments uses ≈ 0.1–0.4 MB per day for chain trust; catch-up after a week offline is ≈ 2 MB once. Phone CPU: ≈ 30 Equihash verifies and a few hundred BLAKE2b hashes per day, well under a second [estimate].

## T2 P2P light gateway

The gateway is the only part with internet. It holds no keys, its word counts for nothing, and any device may run it. Instead of gRPC to one lightwalletd (whose compact blocks carry no header and whose `GetTreeState` returns only a frontier [V: lightwalletd v0.6.1 `909dee8` `parser/block.go` L110–117]), it talks to the Zcash P2P network directly:

- **Headers-first sync** with `getheaders` (≤ 160 headers per reply, ZIP 204 [V]) from many peers; Zebra answers `getheaders` and `getdata` from any peer [V: Zebra CHANGELOG, `zebrad::components::inbound`]. The gateway verifies every header itself (≈ 0.8 s of Equihash per day of headers at 234 µs [M]) and keeps the heaviest chain.
- **Whole blocks** with `getdata` for every block since its checkpoint. It appends commitments to its own Sapling, Orchard and Ironwood frontiers (no UTXO set, no proof verification, a few MB of state) and builds the ZIP 221/258 history leaves, which it checks against each header's `hashBlockCommitments`. A mismatch means a bad block or a bug, never a silent acceptance. This is the existing PoC's MMR rebuild ([`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md)) with a P2P source instead of node RPC.
- **Proofs** (ticks, FlyClient, suffixes, inclusion, anchors, verified compact txs) are computed locally from what it fetched. Zebra's draft prover RPCs (PR #9693: `gethistorynode`, `getauthdataroot`, `gettotalwork` [V]) can replace the local MMR later; they are not needed.
- **Submission** to at least three randomly chosen peers, each over a separately isolated Tor circuit (`arti-client` `IsolationToken` [V]; `zebra-network`'s `isolated` API already wraps Arti [V]); rebroadcast every 3 blocks until a verified inclusion or expiry. Inclusion is detected from blocks it already fetches, never by per-txid queries.
- **lightwalletd and Zaino** stay usable as extra block sources (more diversity), never as authorities. Mainnet has ≈ 18 servers from ≈ 10 operators; testnet has one operator (zec.rocks) [V: Hosh snapshot, about 26 September 2026], which is one more reason not to depend on them.

Internet-side cost: a full day of headers is 5.1 MB, blocks depend on chain volume; neither touches BLE. The P2P client is new code (the plan estimates it), not a full node: no mempool, no consensus validation beyond PoW, commitments and DAA, which is all its outputs claim.

**Uplink privacy.** Arti 2.7.0 (1 October 2026) is current, but its Android/iOS guides say it "does not have a stable Rust API yet … no proper bindings are provided" [V]; the gateway runs on laptops first. In real shutdowns Tor can fail entirely: in Iran in January 2026 Tor users "went almost to zero" and "all Tor pluggable transports were affected … obfs4, Snowflake, and WebTunnel" [V: Tor forum, Snowflake January 2026 update]. If Tor is unreachable the gateway must either refuse to submit or submit exposed; that is an owner decision (list at the end). Nym's mixnet is live for Zingo and Zkool through `nym-smolmix` ([Nym, 24 September 2026](https://nym.com/blog/nym-mixnet-zcash-wallets) [V]) and is a later option behind the same interface (librustzcash #2665 plans a pluggable privacy layer [secondhand]).

## T3 Quiet mesh

The mesh keeps `ozm-mesh`'s flooding, store-and-forward and 512-byte frames. What changes is what a relay or radio observer can see. Today it sees a cleartext 8-byte sender and recipient, `created_ms`, `hops`, a static BLE hint and static beacon IDs ([ozm-mesh README](../../crates/ozm-mesh/README.md)).

| Mechanism | Design | Source | Cost [estimate] |
|---|---|---|---|
| Link encryption | Noise `XX_25519_ChaChaPoly_BLAKE2s` on every TCP and GATT link; ephemeral keys per link, so links have forward secrecy; static key optional (contacts use it, strangers use a fresh one) | BitChat whitepaper L77 at `04995f6` [V]; Reticulum links (297 B setup) [V] | ≈ 192 B per handshake; 16 B tag per 512 B frame (3 %) |
| Rotating identifiers | BLE service data, LAN beacon tag and the Hello link ID = PRF(epoch secret, epoch), rotated every 10–20 min with a random offset, **and** links torn down at rotation so the advertising address, payload and connection all change together | GAEN v1.2.2: identifier and address "shall be changed synchronously" [V]; Becker et al. tracked devices through asynchronous payload changes ([PETS 2019](https://crysp.petsymposium.org/popets/2019/popets-2019-0036.pdf) [V]) | 0 bytes; one reconnect per epoch |
| Fleet mask | For a fleet, every frame header is XOR-masked with a key-derived stream and carries an 8-byte keyed tag so outsiders cannot parse or inject; open mode skips it | Reticulum IFAC, 8 B on LoRa (`RNodeInterface.py` L110 at `e40191b`) [V] | 8 B per frame |
| No sender | The packet header loses `sender`; `hops` is replaced by a randomised initial TTL (5–7); `created_ms` becomes a coarse 1-byte expiry bucket | Reticulum: "does not include source addresses on any packets" (whatis.rst L77) [V] | −15 B per packet |
| Recipient hint | Contacts: 4-byte hint = PRF(pair secret, epoch); strangers: hint 0, trial-decrypted. Hints come only from secrets (BitChat's day tag uses public keys and is computable by anyone, `PeerIDRotation.swift` L28–34 [V]) | Briar BTP tags rotate daily (`D + L`, D = 86,400 s) [V] | 4 B; ≈ 100 µs per trial decryption on Cortex-A53 [secondhand] |
| Preimage ack | The sender puts H(a) in the header and seals `a` inside; the recipient floods `a` (32 B); relays check the hash and delete stored copies. Nobody else can forge it, and it names nobody. The signed receipt still goes inside the sealed reply | Bitmessage embeds a ready-made ack the recipient rebroadcasts ([protocol spec](https://wiki.bitmessage.org/index.php/Protocol_specification) [V]) | 32 B ack versus 144 B today |
| Fixed sizes | Every frame is a full 512 B cell; envelopes use two classes: 1 cell (acks, ticks, small control) and 12 KiB (payments; 9,166 B tx fits) | BitChat pads only to 255 B (L49) [V] | Up to 25 % padding for small messages |
| Jitter | Relays wait 10–220 ms per forward; payments wait 0–30 s at origin | BitChat relay jitter 10–220 ms [V] | Latency only |
| Forward secrecy for sealed envelopes | Each device publishes a signed X25519 receive key per 12-hour epoch inside its presence message and deletes the private half two epochs later; envelopes seal to the current epoch key | Reticulum ratchets (30 min, 512 kept) [V]; Briar rotation deletes old keys (BTP L193–201) [V] | ≈ 100 B per epoch per contact, +4 B per envelope |
| First-hop cover | When a payment leaves a device, it also emits link-local decoy cells (TTL 1) in a payment-shaped burst at random times, ≤ 1 per hour | Loopix-style drop traffic, scaled down [V: Piotrowska et al. 2017] | ≤ 300 KB per day out |

**Quotas without sender IDs.** Per-sender quotas (v0.7.2) need a sender. Without one, quotas are per link and per recipient hint: a neighbour gets a fixed airtime and store share; contacts' hints get a reserved share; trial-decrypt (stranger) traffic gets a capped share. A flooder nearby can exhaust the stranger share, which is equivalent to jamming that share and no worse.

**What is rejected and why.** Sphinx onion routing: on a flood mesh every relay already sees every packet, and a mesh that forms by chance has no stable mixes; 3-hop headers (160–348 B) would cost 1.5–1.9× airtime for no unlinkability gain [V: Sphinx 2009; Nym `sphinx@fd64a73`]. Loopix-rate cover traffic: ≈ 1.5 MB per device per day before flood amplification [estimate]. MLS: needs a delivery service to order commits (RFC 9420 §14 [V]). OMR, FMD and PIR from devices: details in the research log.

**What cannot be hidden.** A device that transmits can be located by RSSI or angle of arrival and fingerprinted at the physical layer: "only needs 10 packets to accurately identify a device" and this "cannot be defended against by simple software/firmware update mechanisms" ([Givehchian et al., S&P 2022](https://cseweb.ucsd.edu/~nibhaska/papers/sp22_paper.pdf) [V]). Open-mode discovery also needs a fixed service UUID, so a scanner can tell that an ozm device is nearby; fleets avoid this with a keyed, rotating UUID.

## T4 Social identity

- **Keys are made on the device.** No invite carries a private key. `fleet-init` produces only public material.
- **People, not devices.** A person key signs that person's device keys (Matrix cross-signing pattern [V: Matrix spec v1.16]); losing a phone revokes one device, not the person.
- **Pinning in person.** Two devices scan each other's QR codes (Briar BQP: a 16-byte commitment to an ephemeral key, "sufficient for 128-bit security because the adversary cannot make use of the birthday paradox" [V: BQP.md at `623b8b29`]), then exchange keys over BLE and confirm. About 184 B. Without QR, a 6-digit SAS after a commitment (ZRTP: one chance in 2^20 for 20 bits; Matrix decimal SAS 39 bits [V]).
- **Introductions, one hop.** A pinned contact can introduce two of its contacts (Briar introduction client [V]); the result is labelled "introduced by X" until the two meet, because "the introducer may carry out a man-in-the-middle attack" [V]. No transitive web of trust; no public social graph (the PGP certificate-spamming lesson, CVE-2019-13050 [V]).
- **Names are local.** Each device shows its own petname for a key plus the key fingerprint. An address shown on the payee's own screen as a ZIP 321 QR is the only way to learn a stranger's address; address bindings over the mesh keep their v0.6 receiving-authority proof and pin.
- **Fleets are optional and threshold-held.** A group that wants shared membership runs a FROST DKG at a meetup (frost-core 3.0.0 `2016e44`: "two rounds … one round requiring a broadcast channel" [V]) with t > 2n/3, so two conflicting fleet logs need at least 2t − n colluding members. The group key signs ≈ 190-byte member certificates and heads of a hash-chained fleet log (≈ 112 B). Two signed heads with one sequence number are a 224-byte fork proof that floods the mesh and freezes new admissions. ROAST lets signers who come and go finish a signature with a coordinator that is "trusted merely for robustness but not for unforgeability" ([ePrint 2022/550](https://eprint.iacr.org/2022/550) [V]). Removing an authority member needs a new DKG: FROST refresh "does not 'restore full security'" against a removed member colluding with old shares [V: FROST book].
- **Membership grants nothing financial.** A fleet certificate says "the fleet admits this device", which a user may use for mesh priority, but pending credit is extended only to keys **this user** pinned or explicitly accepted (T5). So a dishonest fleet authority can at worst add Sybil members who get mesh bandwidth, not money.
- **Gateways need no admission.** Chain facts are self-verifying (T1) and submission goes to every reachable gateway, so a Sybil gateway can waste bandwidth or withhold, never mislead. Pinned gateways get queue priority only. This replaces nearest-gateway selection and the G4 certificate idea.
- **Rejected:** SybilGuard/SybilLimit/SybilRank (need the global graph and fast mixing; "much larger than anticipated" mixing times in real graphs, Mohaisen et al. IMC 2010 [V]); BrightID-style online graph analysis; global proof-of-personhood chains (Encointer, Idena) beyond borrowing their meetup and invitation-quota ideas.

## T5 Bounded credit (the accepted residual)

Pending acceptance stays: offline payments are the point of the project. It is limited so the residual is exactly "a payer the payee chose double-spends within a bounded amount":

1. **Pending only from pinned or explicitly accepted payers.** Strangers settle at work-priced inclusion (v0.6 already refuses pending to open/unknown payers; this generalises it to "the user's own choice", not fleet membership).
2. **Caps per payer, per payee and per escrow group** as decided in [pending-tier.md](../components/pending-tier.md), with the payee setting its own appetite.
3. **Circle nullifier gossip.** A payee shares pending nullifiers only with payees it pinned, sealed to their epoch keys (≈ 104 B per payment plus sealing), instead of a fleet-wide group. This keeps P9's leak inside the user's chosen circle.
4. **Escrow from chosen co-signers.** Above the floor, the payer + 3-of-4 FROST co-signer group ([double-spend.md](../components/double-spend.md)) is kept, with one change: co-signers are chosen by the payer from people the payee also accepts. Fraud then needs the payer plus two co-signers both parties chose.
5. **Evidence.** A conflict yields the signed statement, both txs and the verified inclusion of the winner: a portable debt claim against a person the payee knows.

Bonds were researched and rejected for the shielded pool: Ruffing–Kate–Schröder penalties need a deposit "time-locked … the depositor cannot withdraw them" ([CCS 2015](https://www.cs.purdue.edu/homes/akate/publications/Non-equivocationWithBitcoinPenalties.pdf) [V]); Zcash has CLTV but no CSV in transparent script (zcash_script `d587822` [V]) and a transparent bond is public and linkable. A co-signer-held bond note is possible but adds nothing the escrow does not already give.

## T6 Verifiable distribution

| Today | Design | Residual |
|---|---|---|
| Catalogue points at mutable `releases/latest`; `.lgx` unsigned | `lgx sign` every package; publish the signer DID out of band; users add it to their keyring. Logos `trustedSigners` "is a SELF-ASSERTION and establishes no anchor" (logos-package-manager `8e20ab6` `package_manager_lib.cpp` L377–394 [V]), so only the keyring counts | The keys in the user's keyring |
| `install.sh` checks a checksum from the same release | Verify a GitHub artifact attestation (SLSA Build L2, offline bundle with a pinned trust root [V: GitHub docs]) **and** a t-of-n signed `release.json` | t-of-n signers |
| Public APK test key | New `applicationId` and an offline-held key; anyone holding the public test key could forge a v3 rotation lineage, so a clean break is safer than rotation | The release key holders |
| Actions on tags, shared caches | SHA-pinned actions (v0.7.2), cache-free release builds, `publish` behind a reviewed environment, immutable releases | GitHub runners for provenance only |
| No reproducibility | `rust-toolchain.toml`, `SOURCE_DATE_EPOCH`, `--remap-path-prefix=all`, deterministic tar/gzip, `nix build --rebuild`, apksigcopier-style APK checks; k independent rebuilders publish matching hashes (Zebra 6.0.0 made its release images reproducible [V: Zebra CHANGELOG L485–489]) | k rebuilders you choose; the toolchain |
| Devices offline for days cannot check freshness | TUF-style metadata (root and targets t-of-n offline, timestamp online, TUF 1.0.37 [V]) relayed over the mesh by gateways (Uptane's online Primary pattern, Uptane 2.1.0 [V]). Expiry is measured in **verified block height** from T1, not device time, so a freeze is detected after N blocks | A device can be frozen on an old release for at most the metadata lifetime |

## T7 Sealed device

- **Storage.** One random database key encrypts `secret.json`, wallet, payments and the mesh store. The key is wrapped by Android Keystore (TEE; StrongBox where present), Apple Keychain `ThisDeviceOnly`, Linux Secret Service or `systemd-creds` for daemons. No platform keystore holds Pallas keys (KeyMint curves: P-224/256/384/521 and 25519; "STRONGBOX … do not support curve 25519" [V: IKeyMintDevice.aidl]; Secure Enclave "works only with NIST P-256" [V]), so the spending key is decrypted only while signing.
- **Optional hardware signer.** A payer can keep no spending key on the phone and sign PCZTs on a Keystone (Ironwood and batch PCZT over QR since firmware 3.0.2 [secondhand]) or a Ledger ("Your spending keys stay on your Ledger signer" [V: Ledger, 23 Sep 2026]). Compatibility with our Common 2.2.0/4.0 PCZT is [unverified].
- **Chain clock.** Mesh TTLs, gossip expiry, metadata freshness and `seq` use verified block height and median time past from T1, not the wall clock (OS6).
- **IPC allow-list.** The Logos module accepts `pay` only from the three bundled apps, and asks the user to confirm on screen; the caller identity Logos exposes is [unverified].
- **History.** Payments older than the dispute window are pruned to hashes; with T3's epoch keys deleted, a seized device cannot open old traffic.

## Residual trust, row by row

Columns: v0.7.0 status (from the original table) → residual after this design; mechanism (part); evidence; BLE cost [estimate]. "Avail." means the only residual is availability: one honest, reachable party is enough. "Hash majority" means Zcash's own honest-majority assumption (row Z1), which no light client can remove. Rows the v0.7.0/v0.7.2 threads already fix are marked "direct fix" and only extended here where architecture adds something.

### 1. Payer

| # | v0.7.0 | Residual after design | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| A1 | Partial | **Accepted residual**: a payer the payee chose double-spends, within caps; escrow needs payer + 2 of 4 co-signers both chose | T5 | Failure model in [double-spend.md](../components/double-spend.md); no offline scheme prevents it without trusted hardware or co-signers | 104 B per payment (gossip) |
| A2 | Not (open) | None: credit only to pinned payers; strangers settle at inclusion | T4, T5 | v0.6 rule generalised | 0 |
| A3 | Not | Hash majority | T1 anchor leaf under verified tip | Regtest PoC; ZIP 258 leaf roots [V] | 5–7 KB per new anchor |
| A4 | Enforced | None | unchanged | `node/app.rs` | 0 |
| A5 | Partial | The device while unlocked and signing, or the hardware signer's firmware | T7 | KeyMint/SE curve limits [V] | 0 |
| A6 | Not | Avail. (hidden notes); fake notes impossible | T1 verified compact tx + witness root check | ZIP 229/244 digest tree [V] | ≈ 8 KB per received note |
| A7 | Planned (v0.7.0 text) | None: a signed tx is payable until expiry by design; re-issue only from the same notes means a payer can never pay twice | T1 expiry from verified height; family rule | [expiry-reissue.md](../components/expiry-reissue.md) | 0 |
| A8 | v0.7.0 unlock | None: unlock at **verified** expiry; a hidden earlier inclusion only makes the next spend fail | T1 | as above | ticks |
| A9 | Not | Payee learns (by design); RF observer sees a burst (physical) | T3 no sender, hints, cells, jitter, decoys | Reticulum, Briar, BitChat [V] | ≈ 300 KB/day decoys out |
| A10 | Planned | Payee sees the amount, which is inherent to paying | Max uses denominated notes from "Prepare for offline" so it reveals no extra note structure | — | 0 |

### 2. Payee

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| B1 | Not | Hash majority + avail.; FlyClient heuristic only on conflict, never for value | T1 | ZIP 221 Security [V]; FlyClient [V] | 30–180 KB/day ticks; catch-up ≈ 1–2 MB |
| B2 | Partial | Part of the A1 residual | T5 | pending-tier caps | 0 |
| B3 | Not | Hash majority, priced: forging costs ≥ κ × value in mining | T1 work-priced suffix | ZIP 218 subsidy scaling [unverified exact] | ≈ 220 KB per window |
| B4 | Enforced | None | T4 pinning | — | 0 |
| B5 | v0.6 proof + pin | None: address from the payee's own screen or a proof-bearing pinned binding | T4 | `core/binding.rs` | 0 |
| B6 | Enforced | None | unchanged | — | 0 |
| B7 | Not (v0.7.2 nonce) | Gateway sees only the (public) tx and that it came from this mesh | T3 sealed submission, no sender; v0.7.2 nonce | — | 0 |
| B8 | Not | None | Fresh diversified address per invoice; one ivk still detects all ([ZIP 316](https://zips.z.cash/zip-0316)) | Orchard diversifiers | 0 |

### 3. Relays and other mesh devices

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| R1 | Partial (v0.6 signed acks) | Avail.: delivery needs one honest path | Flooding + store + preimage acks no relay can forge | Bitmessage acks [V] | 32 B per ack |
| R2 | Enforced | None | unchanged, plus link encryption | — | — |
| R3 | Not | Messages of the current 12 h epoch if a key leaks now | T3 epoch receive keys, Noise links | Briar/Reticulum ratchets [V] | ≈ 100 B per epoch per contact |
| R4 | v0.6 signed | None | unchanged | — | — |
| R5 | v0.7.2 quotas | A nearby flooder can use up the strangers' share (equivalent to jamming it) | Per-link and per-hint quotas | — | 0 |
| R6 | v0.7.2 | Neighbours see traffic volume only | Link encryption; rotating hints make harvested packets unlinkable | — | 3 % |

### 4. Gateway device

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| G1 | Partial | Avail.: one honest gateway eventually | Payer and payee submit to every reachable gateway; inclusion verified, so withholding is visible | — | 12 KiB per gateway reached |
| G2 | Designed only | None (hash majority) | T1: the gateway reports nothing trusted | — | — |
| G3 | Not | Avail. | T1 | — | — |
| G4 | Planned | None: legitimacy no longer matters for safety | T1, T4 | — | — |
| G5 | Not (v0.7.2) | None beyond the tx bytes | T3 | — | — |
| G6 | Not | The gateway knows a (public) tx entered the internet from its mesh at time t | Broadcast sync, bucketed requests, Tor uplink | ZIP 307 [V]; Zashi Tor [V] | 0 |
| G7 | v0.7.0 codes | None: status comes from proofs, not node errors | T1 | — | — |
| G8 | Not | None | Gateway identity separate from any wallet; P2P submission over isolated circuits | — | — |
| G9 | v0.7.2 quotas | Avail. | Quotas; proof checks bounded per link | — | — |

### 5. lightwalletd server operator

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| L1 | Not | **Removed from the trust path**: optional extra source, checked like any other | T2 P2P headers and blocks; T1 proofs | lightwalletd header empty [V] | 0 |
| L2 | Not | Removed: submission to ≥ 3 P2P peers on isolated circuits | T2 | ZIP 204 [V] | 0 |
| L3 | Not | Removed; P2P peers see a Tor exit and whole-block fetches | T2 | Arti isolation [V] | 0 |
| L4 | Not | Removed; Zcash P2P network availability remains | T2 | Zebra serves peers [V] | 0 |
| L5 | v0.7.2 pinning | Removed: data is self-verifying, TLS is not relied on | T1 | — | 0 |

### 6. Zcash network, miners and testnet

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| Z1 | Enforced | **Hash majority** (fundamental) | — | — | — |
| Z2 | Not | Hash majority, priced; testnet minimum-difficulty blocks count as zero work | T1 work pricing | ZIP 218 450 s rule [V] | — |
| Z3 | v0.7.0 smoke test | Not a trust row after testing | — | — | — |
| Z4 | v0.7.0 | Not a trust row | — | — | — |
| Z5 | Partial | The "delayed payment" class stays visible; blending with ZIP 318 buckets is [unverified] | Proposal: ZIP 318-style expiry buckets and 432-block anchor boundaries instead of the fleet-specific 72,576 delta ([prior-art.md](../prior-art.md)) | ZIP 318 [V via prior art] | 0 |
| Z6 | Not | Network liveness (fundamental) | — | — | — |

### 7. Organiser (becomes optional fleet authority)

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| O1 | Not | None: holds no device key | T4 on-device keys | — | 0 |
| O2 | Not | None: invites carry public keys only | T4 | — | — |
| O3 | Not | t-of-n members you chose (t > 2n/3) colluding can admit members; admission grants no credit; forks are provable | T4 threshold log, fork proofs | FROST RFC 9591 [V] | 190 B per certificate, 112 B per head |
| O4 | Partial | None: network constants come from the verified release; credit policy is each payee's own | T5, T6 | — | — |
| O5 | Enforced | None: gateway role no longer confers trust | T1 | — | — |

### 8. Open-mode strangers

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| S1 | v0.6 fixed | None beyond DoS | v0.6 + link encryption | — | — |
| S2 | Not | Airtime DoS only: nothing counts identities | T1 heaviest chain, T4 credit by choice, T2 any gateway | — | — |
| S3 | Planned | None | Petnames + fingerprints, QR/SAS | BQP [V] | — |
| S4 | Not | A scanner can tell an open-mode ozm device is near (fixed UUID); fleets hide even that | T3 rotation, keyed fleet UUID | GAEN [V] | 0 |
| S5 | v0.6 fixed | None | — | — | — |
| S6 | v0.6 rule | None | T5 | — | — |

### 9. The faucet

| # | v0.7.0 | Residual | Mechanism | BLE cost |
|---|---|---|---|---|
| FA1 | Not | The faucet learns that some fresh address got coins (inherent) | Request via the gateway over Tor with a new diversified address | 0 |
| FA2 | Planned | Not a trust row | Test | — |
| FA3 | Enforced | None | — | — |

### 10. Software distribution

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| D1 | Not | t-of-n release signers and k rebuilders the user chose; the source code itself unless reviewed | T6 | TUF 1.0.37, logos-package [V] | metadata ≈ few KB per release |
| D2 | Not | The holders of the new offline key | T6 new applicationId | Android docs [V] | — |
| D3 | Partial | As D1 | T6 attestation + threshold signature in `install.sh` | GitHub attestations [V] | — |
| D4 | Not (v0.7.2 SHA pins) | GitHub runners only for provenance; rebuilds catch tampering | T6 cache-free releases, rebuilders | Khan 2024 cache poisoning [V] | — |
| D5 | Not | Other Basecamp modules the user installed and allowed | T7 allow-list + on-screen confirmation | [unverified] Logos caller identity | — |

### 11. Passive network or radio observer

| # | v0.7.0 | Residual | Mechanism | Evidence | BLE cost |
|---|---|---|---|---|---|
| PO1 | Not | Burst timing and location of transmitters (physical) | T3 | — | as A9 |
| PO2 | Not | Physical-layer fingerprint (not fixable in software) | T3 RPA-synced rotation, Noise on GATT | Givehchian 2022 [V] | 0 |
| PO3 | Not | LAN presence | T3 Noise on TCP, rotating beacon tags | — | 3 % |
| PO4 | Not | Uplink metadata visible to the network operator **only if Tor is blocked and the owner chooses exposed submission** | T2 Arti | Tor in Iran 2026 [V] | 0 |
| PO5 | Not | Jamming and direction finding (physical) | — | — | — |

### 12. Device OS and platform

| # | v0.7.0 | Residual | Mechanism | BLE cost |
|---|---|---|---|---|
| OS1 | Enforced | The OS RNG (fundamental) | unchanged | — |
| OS2 | Partial | Malware on an unlocked device while signing; avoided entirely with a hardware signer | T7 | — |
| OS3 | v0.7.2 0600 | None at rest | T7 encrypted store | — |
| OS4 | v0.6 fixed | None | — | — |
| OS5 | v0.7.2 | None | — | — |
| OS6 | Partial | None: verified chain time replaces the clock | T7 | — |
| OS7 | Not | What the device still retains within the dispute window | T3 epoch keys, T7 pruning | — |

## What each party can learn, after the design

| Party | Amounts | Addresses | Sender ↔ receiver | Timing | Location / topology | Balance |
|---|---|---|---|---|---|---|
| Payer | Own | Payee's (given by the payee) | Own | Yes | Neighbourhood only | No |
| Payee | Paid to it | Payer's key (chosen contact) | Yes, by choice | Yes | No hop count | No |
| Relay | No (fixed cells) | No | **No** (no sender; rotating hints) | Coarse | Neighbours only | No |
| Gateway | No | No | **No** | When the tx enters the internet | That the tx came from its mesh | No |
| P2P peers / lightwalletd | No | No | No | Submission time | A Tor exit | No |
| Network / miners | No | No | Nullifier linkage on chain | Yes | No | No |
| Fleet authority (optional) | No | No | No | No | No | No |
| Open-mode stranger | No | Only addresses shown to it | No | Coarse | That an ozm device is near | No |
| Faucet | Its payout | A fresh address | No | Yes | A Tor exit | No |
| Software distribution | Everything if t-of-n signers **and** k rebuilders collude or the code is malicious | | | | | |
| Passive radio observer | No | No | Burst timing only | Yes | **RF location** (physical) | No |
| Device OS | Everything on the device while unlocked | | | | | |

## What each party can do, after the design

| Party | Steal | Double-spend | Censor | Delay | Lie about mined | Deny service | Impersonate |
|---|---|---|---|---|---|---|---|
| Payer | No | **Yes, bounded (accepted residual)** | — | Yes | No | Spam within quota | No |
| Payee | No | No | No (payer submits too) | No | No | — | No |
| Relay | No | No | Drop only; other paths deliver | Yes | No | Its own airtime | No |
| Gateway | No | No | Withhold; visible and routed around | Yes | **No** (proofs) | Yes, until another gateway | No |
| P2P peers / lightwalletd | No | No | One peer; others accept | Yes | Only with majority hash power | No | No |
| Network / miners | No | Majority hash only | Majority only | Yes | Majority only, and priced | Yes | No |
| Fleet authority | No | No | Refuse admissions | — | No | — | Add members (no credit) |
| Open-mode stranger | No | No credit offered | No | No | No | Airtime | No |
| Faucet | No | No | Withhold test coins | Yes | No | — | No |
| Software distribution | Only if t-of-n signers and k rebuilders fail together | | | | | | |
| Passive observer / jammer | No | No | Jamming | Jamming | No | Jamming | No |
| Device OS | Yes while unlocked (not with a hardware signer) | | | | | | |

## Accepted residuals and why each is unavoidable

1. **A chosen payer double-spends a pending payment.** Accepted by the owner. Without a co-signer or trusted hardware, a key holder can always sign a second spend of the same note while the first is still offline; consensus picks one. Bounded by caps and the user's choice of payers (T5).
2. **Honest-majority hash power (Z1, Z2, Z6).** A light client verifies work, not the absence of a heavier chain it has not seen; that is Zcash's own security assumption. Work pricing makes forgery cost more than it can win; ticks from any one honest source end an eclipse.
3. **Availability needs one honest, reachable path and source (R1, G1, G3, G9, L4).** No protocol can deliver data through a medium that every reachable node drops or jams. The design ensures one honest node anywhere on a path suffices.
4. **Physical radio facts (A9, PO1, PO2, PO5, S4 in open mode).** Transmission itself reveals presence and position, and PHY imperfections identify chips; Givehchian et al. show this is not fixable in firmware. Fixed cells, rotation and low duty cycle reduce what is learned above the PHY.
5. **The endpoint that signs (A5, OS2).** Some device must hold the spending authority at the moment of signing; malware there can sign. A hardware signer moves this trust to a smaller device; FROST escrow splits it.
6. **The software you run (D1–D5).** A user who does not read the code trusts whoever does. The design reduces this to t-of-n signers and k independent rebuilders the user chooses; it cannot remove it.
7. **The OS random number generator (OS1).** Every key depends on it.
8. **First entry into the internet (G6, PO4).** A transaction must enter the Zcash network from some machine, and that machine sees it first. Tor hides that machine from the internet; nothing hides the transaction from the machine that sends it. If Tor is blocked, the owner's policy decides between not sending and sending exposed.
9. **What a party you pay learns (A9, B4).** Dealing with someone by choice means they know you; the design keeps everyone else out.
10. **An optional fleet authority admitting members (O3).** Only if the user opts into a fleet, only t > 2n/3 colluding members, and admission confers no credit.

Rows not listed above have no residual beyond these.

## Owner decisions needed

1. **Chain-trust rule.** Replace "≥ 2 gateways must agree on the tip" with "heaviest verified chain from any source; FlyClient only on conflict or catch-up; value decisions priced in work". Choose κ (proposed 10) and keep the half-difficulty floor.
2. **Gateway source.** Replace gRPC lightwalletd with a Zcash P2P light client; lightwalletd stays as an optional extra source. This reverses the v0.7.0 choice of trusting TLS lightwalletd servers.
3. **Mesh wire v2.** A breaking change: no sender field, rotating identifiers, Noise links, preimage acks. Preimage acks conflict with the AGENTS.md rule "delivery acks authenticate their sender": they authenticate the recipient by knowledge of a secret, without naming anyone.
4. **Social trust model.** The organiser stops generating keys; fleets become optional and threshold-held; pending credit goes only to payers the payee pinned or accepted, regardless of fleet membership.
5. **Uplink when Tor is blocked.** Fail closed (hold transactions until Tor works or expiry) or submit exposed with a visible warning.
6. **On-chain blending.** Move from the 72,576-block fleet expiry to ZIP 318-style buckets and 432-block anchor boundaries, after measuring whether that actually blends with other wallets.
7. **Scanning on BLE.** Make off-mesh payment discovery a Wi-Fi/hub activity when chain volume exceeds the BLE budget; mesh payments need no scanning.
8. **Release signers.** Name the t-of-n release signers and at least two independent rebuilders; approve the new Android `applicationId`.
9. **Hardware signer priority.** Whether a Keystone/Ledger PCZT spike comes before or after the phone payer.
