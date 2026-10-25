# Threat Model and Mitigations: Moving Ironwood-Shielded Testnet ZEC Between Offline Basecamps over a Logos Messaging Mesh

Research date: 2026-10-24. Scope: privacy and security threat model only. Logos feature details, Zcash/Ironwood internals, radio link engineering, and offline-payment prior art are covered by other researchers. These notes refer to them only where a threat depends on them.

Reference architecture assumed throughout:
- **Sender basecamp:** syncs chain state, then builds, proves and signs an Ironwood (v6) transaction while offline.
- **Mesh relay:** the tx, plus any note or receipt data, crosses the Logos Messaging (Logos Delivery) mesh, possibly over a LoRa or other low-bandwidth RF link.
- **Gateway:** a node with intermittent uplink broadcasts the tx to Zcash testnet through Zebra, lightwalletd or Zaino.
- **Recipient basecamp:** detects the payment from the relayed tx or from chain data synced over the mesh.

Notation: "Cited Findings" are verified against the linked source. "Inferences" are my reasoning and are not independently sourced. Where a source is old or superseded, that is flagged inline.

---

## 1. Zcash-layer privacy: what an Ironwood transaction reveals, broadcast-origin linkage, light-client leakage, timing and amount correlation, and mitigations

### Takeaway
A fully shielded Ironwood transaction hides sender, recipient, amount and memo. It still publicly reveals a fixed set of metadata:
- nullifiers, note commitments and the number of actions;
- the anchor (which treestate the builder used), the expiry height, the fee/valueBalance and the exact size;
- any amount that crosses the Orchard→Ironwood turnstile or the transparent boundary.

The largest practical leaks for this architecture are network-level, not on-chain. They are the first node to broadcast (the gateway), what light-client servers learn, and the stale anchor/expiry and timing signatures that an offline, delayed transaction leaves on-chain.

### Cited Findings
**Ironwood status (current as of Oct 2026)**
- Ironwood is a new shielded pool activated by NU6.3 at mainnet height 3,428,143 (around 2026-07-28). It replaces Orchard after a soundness flaw in Orchard's circuit, found by Taylor Hornby, that could in theory have allowed undetectable counterfeiting. Orchard is now exit-only, through a turnstile. — [crypto.news](https://crypto.news/zcash-ironwood-upgrade-whats-changed-after-the-orchard-bug/); [The Block](https://www.theblock.co/post/409934/zcash-ironwood-upgrade-launching-new-shielded-pool-after-orchard-vulnerability)
- The NU6.3 **testnet** activation height is 4,134,000, with consensus branch ID 0x37A5165B. — [ZIP 258](https://zips.z.cash/zip-0258); [Zebra releases](https://github.com/ZcashFoundation/zebra/releases)
- Ironwood has its own note commitment tree, nullifier set and value pool. The Lean formal verification (more than 2,700 theorems) covers the proof system, the circuit and balance accounting. It **does not cover Ironwood's privacy guarantees**. zcashd does not support the NU6.3 rules, so Zebra is required. — [crypto.news](https://crypto.news/zcash-ironwood-upgrade-whats-changed-after-the-orchard-bug/)
- Ironwood uses Orchard's receiver structure, so existing addresses stay valid. "The amount transferred between pools is publicly visible on the blockchain," though sender and recipient stay private. — [Zodl Support: What is the Ironwood shielded pool?](https://support.zodl.com/article/56-what-is-the-ironwood-shielded-pool)

**v6 transaction format (ZIP 229, Draft, created 2026-06-13): what is on the wire**
- The header carries version, nVersionGroupId (0xD884B698), nConsensusBranchId, lock_time and nExpiryHeight. There is **no explicit fee field**. — [ZIP 229](https://zips.z.cash/zip-0229)
- The Ironwood bundle contains:
  - `nActionsIronwood`;
  - 820 bytes per action, using the same encoding as an Orchard action;
  - `flagsIronwood` (enableSpends, enableOutputs, enableCrossAddress);
  - `valueBalanceIronwood` (int64), `anchorIronwood` (32 B);
  - proofs of 2720 + 2272×n bytes, 64 B of spend-auth signature per action, and a 64 B binding signature. — [ZIP 229](https://zips.z.cash/zip-0229)
- The turnstile "reveals the amounts in each transaction that cross between pools." ZIP 229 recommends address rotation because of quantum privacy concerns (ZIP 2005). All Ironwood outputs use the ZIP 2005 quantum-recoverable note plaintext (lead byte 0x03). — [ZIP 229](https://zips.z.cash/zip-0229)
- The 820-byte action is made of:
  - cv (32 B, value commitment), nullifier (32 B), rk (32 B, randomized key) and cmx (32 B, note commitment);
  - ephemeralKey (32 B), encCiphertext (580 B, the encrypted note plaintext including the memo) and outCiphertext (80 B).
  - valueBalance is a plaintext int64 in the serialized tx. — [ZIP 225](https://zips.z.cash/zip-0225)
- Orchard-style actions pair one spend with one output, which hides the input/output arity that Sapling exposed. The number of actions is still a bundle field. — [Orchard book: Actions](https://zcash.github.io/orchard/design/actions.html); [ZIP 225](https://zips.z.cash/zip-0225)

**Fees, expiry, anchor and lock_time conventions (a fingerprinting surface)**
- ZIP 317 conventional fee = 5000 zats × max(2, logical actions). Ironwood actions count as logical actions (Revision 1, draft, enacted with NU6.3). "Non-standard fees may reveal particular users, wallets, or wallet versions." Padding is explicitly not penalized: default wallets pad to 2 inputs and 2 outputs per shielded pool. ZIP 317 was last updated 2026-06-26. — [ZIP 317](https://zips.z.cash/zip-0317)
- ZIP 203 expiry:
  - The default expiry is 40 blocks after the current height (about 50 minutes post-Blossom). ZIP 218 recommends 120 blocks under NU7, which has 25-second spacing and is not yet active.
  - Expired, unmined transactions are dropped from the mempool. nExpiryHeight=0 disables expiry. The maximum value is 499,999,999. — [ZIP 203](https://zips.z.cash/zip-0203)
- ZIP 315 (Draft) wallet conventions:
  - Choose the anchor at the trusted depth (the treestate of block H-3), use the 40-block default expiry, set lock_time to zero and use v6 transactions.
  - Recommended confirmations before spending are 10 for untrusted TXOs and 3 for trusted ones.
  - Get per-transaction opt-in for cross-pool transfers that reveal amounts.
  - Don't auto-shield from multiple transparent addresses in one tx. Prefer time-triggered rather than receipt-triggered auto-shielding, so that receiving a payment never triggers on-chain activity. — [ZIP 315](https://zips.z.cash/zip-0315)

**On-chain/graph heuristics (historical; pre-Sapling/Orchard era, flagged as dated)**
- Kappos, Yousaf, Maller and Meiklejohn (USENIX Security 2018) found most Zcash activity was transparent. Simple heuristics, such as founder and miner patterns and round-trip amounts, shrank the shielded anonymity set considerably. Fully shielded transfers had "no obvious deanonymisation techniques." The often-quoted 69.1% figure for transactions interacting with the pool comes from the authors' slides. — [USENIX](https://www.usenix.org/node/217538); [arXiv 1805.03180](https://www.arxiv.org/pdf/1805.03180); [slides](https://www.usenix.net/sites/default/files/conference/protected-files/security18_slides_kappos.pdf)
- Biryukov & Feher (IEEE CNS 2019) analyzed linkability in Zcash through mining/minting transactions. — [orbilu](https://orbilu.uni.lu/handle/10993/39728)

**Network-level (broadcast-origin) deanonymization**
- Biryukov & Tikhomirov (EuroS&P 2019): "timings of transaction messages leak information about their origin," which a well-connected adversarial node can exploit. They deanonymized their own Bitcoin and Zcash transactions "with a high level of accuracy." A full-scale Bitcoin attack would cost hundreds of USD. — [orbilu PDF](https://orbilu.uni.lu/bitstream/10993/39724/1/biryukov-tikhomirov-deanonymization-and-linkability)
- Biryukov & Tikhomirov (Pervasive & Mobile Computing 2019) found that a moderately resourceful attacker can correlate transactions issued from one mobile device with relatively high accuracy, including for Zcash wallets. — [orbilu](https://orbilu.uni.lu/handle/10993/39729)
- Biryukov & Pustogarov, "Bitcoin over Tor isn't a good idea" (IEEE S&P 2015): an attacker can abuse anti-DoS bans to push Tor users onto attacker exits, then fingerprint them with address cookies. This links transactions and controls which blocks and transactions the user sees. It is Bitcoin-specific, but the lesson applies to any full node run over Tor. — [IEEE PDF](https://www.ieee-security.org/TC/SP2015/papers-archived/6949a122.pdf); [arXiv 1410.6079](https://arxiv.org/abs/1410.6079)
- Tramèr, Boneh and Paterson (USENIX Security 2020) showed remote timing and traffic-analysis attacks (PING/REJECT) on Zcash nodes:
  - They let an active attacker identify the payee of a transaction by measuring how the payee's P2P node reacts.
  - Proof-generation time depends on secret data, including the amount.
  - The attacks could remotely crash a node whose payment address is known, and could mount timing attacks on ECDH involving the viewing key.
  - The issues were disclosed and patched. This is historical, but the lesson stands: don't let a wallet's own node answer P2P requests. — [USENIX](https://www.usenix.org/conference/usenixsecurity20/presentation/tramer); [ePrint 2020/220](https://eprint.iacr.org/2020/220)
- Kappos & Piotrowska ("Extending the Anonymity of Zcash", 2019) argue that a global passive network adversary can link users to transactions, and that Tor alone is not a full defence. — [arXiv 1902.07337](https://arxiv.org/pdf/1902.07337)
- Stolon is a Zebra RFC for Dandelion-inspired diffusion. Its stem phase runs over ephemeral, isolated Tor connections and its fluff phase broadcasts on the clearnet. Its deployment status is **unconfirmed**: the RFC page returned 404 and only a search summary was available. — [Zebra docs search result](https://docsearch.algolia.com/mcp/docs/repo/zcashfoundation/zebra); referenced in [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)
- Dandelion++ (Fanti et al. 2018) is the reference design for hiding broadcast origin from "first-spy" estimators. It is cited by ECC research as a candidate fix, and a Tor transaction relay network is proposed in zips PR #391. — [arXiv 1805.11060](https://arxiv.org/abs/1805.11060); [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)
- Bitcoin Core is reported to add private broadcast over Tor/I2P in v31. This is a non-Zcash reference point. — [Bitcoin Optech topic](https://bitcoinops.org/en/topics/transaction-origin-privacy/)

**Light-client (lightwalletd/Zaino) leakage**
- ZIP 307 (Draft):
  - Compact blocks are broadcast-style, so syncing itself does not reveal which transactions interest the client.
  - Block-range requests reveal the client's last-synced height. Bucketing is proposed but not implemented.
  - Fetching a full transaction (needed for memos and outCiphertext) reveals interest in it. Clients should fetch decoys and every tx in that block.
  - The model assumes Tor or similar for network privacy. The server is "honest but curious." Header validation is only partly implemented (only prevHash is checked). — [ZIP 307](https://zips.z.cash/zip-0307)
- Hornby, "Fixing Privacy Problems in the Zcash Light Wallet Protocol" (ECC; undated, circa 2020–21, partly superseded):
  1. Wallets don't validate headers or PoW, so a malicious lightwalletd can show fake payments and enable double-spend fraud.
  2. There is no network anonymity: lightwalletd learns users' IPs.
  3. Passive monitoring of lightwalletd's connection reveals the transaction graph. The proposed fix is a constant-bandwidth protocol (lightwalletd issue #307).
  4. lightwalletd broadcasts submitted txs to P2P in plaintext, so its host or ISP learns which txs its users sent.
  5. Dust attacks reveal when a wallet spends.
  6. Memo fetches reveal which txs belong to the wallet, so lightwalletd learns the whole graph. Options listed are out-of-band delivery ("liberated payments"), mixnets, PIR and download-all-memos. — [defuse.ca PDF](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)
- Zodl (formerly Zashi) "Tor Protection" (beta) routes exchange-rate requests, **transaction sending, transaction data retrieval** and third-party API calls through Tor. It slows sync. — [Zodl Support: Enabling Tor protection](https://support.zodl.com/article/17-enabling-tor-protection)
- Zaino (Zingo Labs) is the Rust indexer replacing lightwalletd. It separates light-client serving from the full node. I found no 2026 source on Zaino-native Tor support. — [ZecHub: Zaino](https://zechub.wiki/zcash-tech/zaino)

**Viewing keys**
- ZIP 316 (UFVK vs UIVK):
  - An FVK is meant to show all transactions to and from the address. An IVK shows only incoming payments, and the UIVK excludes change.
  - A UFVK includes the ovk components used to decrypt outgoing ciphertexts (destination, amount, memo). The Sapling FVK includes nk (needed for spend detection), and the IVK does not.
  - A transparent component exposes transparent activity too. — [ZIP 316](https://zips.z.cash/zip-0316)

### Inferences
- **Offline/mesh-delayed txs carry a distinctive on-chain signature.** Standard wallets set the anchor to H-3 and expiry to H+40 at build time ([ZIP 315](https://zips.z.cash/zip-0315)). An observer comparing a tx's anchor height and expiry height with the block it was mined in can estimate how long it sat in transit. A tx built against an anchor hundreds of blocks old, or with an unusually large expiry delta, stands out as "built offline," which shrinks the anonymity set to mesh users. Mitigations:
  - Keep the anchor as fresh as mesh sync allows, for example by streaming compact blocks over the mesh.
  - Use one expiry policy across all basecamps so that they at least share an anonymity set.
  - Accept that the anonymity set is "all offline-mesh users," not "all Zcash users."
- **The 40-block (about 50 min) default expiry is incompatible with intermittent uplinks.** Options:
  - A larger, fixed expiry delta used uniformly by every basecamp. This trades fingerprinting for a longer window of gateway power, discussed below.
  - nExpiryHeight=0 (no expiry). This is worst for gateway time-shifting, because a withheld tx could be broadcast arbitrarily later.
  - Re-proving on expiry.
  - A cancellation path: the sender re-spends the same note to invalidate the old tx through nullifier conflict. Because of this path, the recipient cannot treat an unmined tx as final.
- **Size and arity leak.** A minimal Ironwood-only v6 tx is roughly 2,854 + 3,156×n bytes. That is about 9.2 KB for n=2 and about 15.5 KB for n=4, computed from the ZIP 229 field sizes plus empty transparent/Sapling/Orchard counts; this is my approximation. Always padding to a fixed action count (for example, 2 or 4) makes txs indistinguishable by size on both the chain and the mesh. The ZIP 317 fee is then a function of the padded count only.
- **Gateway = first-spy.** Whatever node first injects the tx into Zcash P2P, or submits it to a lightwalletd/Zaino server, becomes its apparent origin ([Biryukov & Tikhomirov](https://orbilu.uni.lu/bitstream/10993/39724/1/biryukov-tikhomirov-deanonymization-and-linkability)). If one known gateway, at a known physical location with a known uplink IP, broadcasts every mesh payment, an observer learns "this tx came from the basecamp mesh." Mitigations:
  - Submit over Tor with a fresh circuit per tx, the way Zodl does.
  - Rotate among multiple gateways.
  - Add randomized broadcast delays and batch several mesh txs per uplink window, mixed with dummy/self-transfers.
  - Never run the gateway's Zebra node as a wallet. Separate node and wallet, per the Tramèr et al. lessons.
- **Bitcoin-over-Tor lesson.** If the gateway runs a full Zebra node over Tor, it is exposed to Biryukov–Pustogarov-style exit-banning and fingerprinting. A one-shot submission (gRPC SendTransaction over a fresh Tor circuit to a lightwalletd/Zaino) avoids long-lived P2P identity. The server still learns submission timing, and the tx itself is public anyway.
- **Light-client sync over the mesh should be broadcast, not query.** If the gateway publishes all compact blocks to a single shared content topic and basecamps only listen, then neither the gateway nor the mesh learns which basecamp is interested in which tx. This is the ZIP 307 property preserved at the radio layer. Full-tx fetches for memos should be avoided. The sender already delivers the full tx to the recipient out-of-band over the mesh, which is in effect a "liberated payment."
- **Amount/timing correlation.** Testnet faucet funds usually arrive transparent, and Orchard→Ironwood migration reveals amounts. Shielding a unique amount and then making a payment soon after, which appears on-chain right after a gateway's uplink window, enables round-trip and timing heuristics of the Kappos type. Mitigations:
  - Shield well in advance, in round or common denominations.
  - Never de-shield the same amount.
  - Spread on-chain appearance times across uplink windows.
- **Memo.** The memo is encrypted inside encCiphertext and is readable only with the recipient's IVK/FVK or the sender's OVK. Memo content is not public, but anything in it (basecamp names, invoice IDs) becomes exposed if a viewing key leaks. Keep memos empty or use an encrypted reference.
- **Never share a UFVK with a gateway, lightwalletd or Zaino operator.** A UFVK reveals incoming, outgoing and change history ([ZIP 316](https://zips.z.cash/zip-0316)). Use a UIVK if watch-only detection must be delegated.

### Gaps
- No authoritative published list of "public fields" specific to Ironwood beyond the ZIP 229 structure. The semantics of the new `enableCrossAddress` flag and any privacy effect are not explained in the ZIP text I retrieved.
- Deployment status of Stolon or Dandelion in Zebra as of Oct 2026 is unconfirmed (the RFC page returned 404). No 2026 source found on Zebra/Zaino native Tor transaction submission.
- No official z.cash or ECC "privacy best practices" page was found. ZIP 315 is the closest official guidance and it is still Draft.
- zcashd's near-expiry relay threshold (TX_EXPIRING_SOON) behaviour and whether Zebra refuses to relay near-expiry txs is not documented in ZIP 203. It needs a code check.
- I found no academic network-deanonymization study specific to Zebra or post-NU5 Zcash. The Biryukov work predates Zebra.

---

## 2. Messaging-layer privacy: what Logos Messaging/Waku leaks, Store logging, RLN deanonymization, libp2p Mix status, cover traffic, padding and ephemeral identities

### Takeaway
Logos Messaging Relay (GossipSub-based) gives k-anonymity for subscribers and only weak sender anonymity, against a single-node attacker. It leaks:
- PeerIDs and IP addresses to direct peers;
- first-hop timing, which reveals the originator to its neighbours;
- message size and content topics to Store, Filter and Lightpush service nodes.

RLN adds spam control but creates a new failure mode: exceeding the rate limit reveals the member's secret, which enables slashing and linkage. A stale membership root can also reveal a member's tree position. The libp2p Mix protocol is integrated into Logos Delivery as of late 2026 but its spec is still "raw":
- fixed 4608-byte Sphinx packets, at least 3 hops, exponential delays;
- loop cover traffic is optional;
- no Sybil or DoS protection by default, and the exit sees the plaintext payload.

### Cited Findings
**Relay anonymity analysis (Vac/Logos research)**
- The Waku Privacy and Anonymity Analysis Part I (2022-07-22) defines its attacker models:
  - internal attackers: single-node, multi-node (Sybil/botnet) and linearly-scaling;
  - external attackers: local, AS and global on-net, each passive or active.

  It finds:
  - **receiver anonymity** (subscriber–topic unlinkability) as k-anonymity against a global adversary;
  - only **weak sender anonymity, in the single-node model only**.

  It notes these leaks:
  - participation is visible at the GossipSub layer;
  - attackers can GRAFT into a victim's mesh if they know its multiaddress;
  - neighbours receive a victim's own messages faster than relayed ones (latency and hop-count signals);
  - AS or global observers can correlate traffic with origin.

  The post does **not** analyze PeerIDs or message size. It flags STORE, FILTER, LIGHTPUSH and FTSTORE as leaking receiver interest or timing. StrictNoSign (no publisher signatures) is named as the main enabler of weak sender anonymity. Roadmap mitigations are Dandelion++, onion routing/Tor and mixnets/Nym. *Dated: it predates RLN mainnet and Mix.* — [Logos Research rlog](https://research.logos.co/rlog/wakuv2-relay-anon)
- 11/WAKU2-RELAY notes that a publisher's direct connections can infer which messages it published through traffic analysis, and that the risk is higher if the payload is unencrypted. It recommends a single pubsub topic for k-anonymity. — [rfc.vac.dev 11/RELAY](https://rfc.vac.dev/waku/standards/core/11/relay) (via search summary)
- Vac forum discussion flagged sender timestamps and static PeerIDs as tracking vectors. This is discussion, not final analysis. — [forum.vac.dev](https://forum.vac.dev/t/on-the-anonymity-of-waku-relay/135)

**Official Logos docs (2026)**
- Logos Delivery uses libp2p PeerIDs as pseudonyms. This is "not complete anonymity," because actions under one PeerID can be linked.
  - Store and Filter need direct connections, so a Store node links a querier's PeerID to the topics queried, and a Filter node links a light node's PeerID to its filter.
  - Some features may not be on the current testnet. — [Logos docs: Security features](https://docs.logos.co/messaging/concepts/security-features)
- Filter, Store and Light Push disclose content topics to peers. Broad, shared topics increase k-anonymity. Store is short-term and has no availability guarantee. Chat provides E2EE, with de-MLS for groups. — [Logos docs: Messaging](https://docs.logos.co/messaging)
- **Docs inconsistency:** the Messaging overview lists "RLN Relay" as a Delivery protocol, while the Security-features page describes RLN as "ongoing research." — [Messaging](https://docs.logos.co/messaging) vs [Security features](https://docs.logos.co/messaging/concepts/security-features)

**Store (13/WAKU2-STORE, Draft, rev. 2026-06-16)**
- A querier must disclose its content filters to the store node.
- Messages with `ephemeral=true` SHOULD NOT be stored.
- A store node MAY reject messages whose timestamp is more than 20 s from its clock.
- The adversary model is passive only, so it does not analyze store nodes omitting messages.
- Future-timestamp attacks are acknowledged.
- Anonymous queries via proxy/Tor and PSI are listed as future work.
- The spec says nothing about query logging or retention periods. — [13/WAKU2-STORE](https://lip.logos.co/messaging/core/draft/13/store.html)

**RLN (17/WAKU2-RLN-RELAY Draft rev. 2026-06-16; 32/RLN-V1 Draft; RLN-V2 Raw)**
- Membership:
  - Publishing requires an RLN membership, registered with a stake through a smart contract, optionally on Ethereum.
  - Each message carries a RateLimitProof containing the zk proof, merkle_root, epoch, nullifier and Shamir share (share_x, share_y).
  - The base rule is one message per epoch per member. — [17/RLN-RELAY](https://lip.logos.co/messaging/core/draft/17/rln-relay.html)
- **Double-signal deanonymization:**
  - Two different messages in the same epoch share a nullifier, and any routing peer can reconstruct the sender's secret key from the two shares.
  - The key MAY be used to remove the member and claim the stake.
  - No single slashing authority is defined, and slashing can be spread off-chain over pubsub. — [17/RLN-RELAY](https://lip.logos.co/messaging/core/draft/17/rln-relay.html); [32/RLN-V1](https://lip.logos.co/anoncomms/draft/32/rln-v1.html)
- **Stale-root leak:** peers must use the latest Merkle root, because an old root can reveal the position of the peer's key in the tree and break message unlinkability. A `max_epoch_gap` tolerance covers network delay and clock skew. — [17/RLN-RELAY](https://lip.logos.co/messaging/core/draft/17/rln-relay.html)
- RLN-V1:
  - The internal nullifier is a per-epoch, per-app fingerprint, so a user's messages within one epoch are linkable.
  - Users SHOULD NOT reuse the RLN secret across protocols.
  - The circuits are unaudited, with no MPC ceremony.
  - Security is estimated at about 160 bits. — [32/RLN-V1](https://lip.logos.co/anoncomms/draft/32/rln-v1.html)
- RLN-V2 adds `message_id` and `user_message_limit` (multiple messages per epoch). A larger limit "helps users stay anonymous." — [RLN-V2](https://lip.logos.co/anoncomms/raw/rln-v2.html)

**libp2p Mix (LIP-99 "LIBP2P-MIX", Raw, latest rev. 2026-08-24)**
- Design:
  - Mix provides sender anonymity by stateless routing through at least 3 mix nodes.
  - Packets are fixed-size Sphinx packets of 4608 B total, with δ = 3984 B of payload, and padding and fragmentation keep packets indistinguishable.
  - Mixing is continuous-time, with an exponential per-hop delay chosen by the sender.
  - Cover traffic is optional loop messages only.
  - Replay protection uses per-node tag tables. — [LIBP2P-MIX spec](https://lip.logos.co/anoncomms/raw/mix.html)
- Security section:
  - Sender anonymity holds "even if multiple mix nodes are colluding," with no explicit tolerated fraction. A **global passive adversary is not addressed**.
  - There is **no built-in Sybil resistance** and **no DoS protection by default**.
  - The **exit sees the decrypted payload** and can alter, drop or fabricate messages undetected.
  - Delivery is probabilistic, with no ACK or retransmission.
  - A destination may run its own Mix final hop to remove the exit trust assumption. — [LIBP2P-MIX spec](https://lip.logos.co/anoncomms/raw/mix.html)
- Related raw specs: Mix Cover Traffic, Mix DoS Protection, RLN DoS Protection for Mixnet, and Stake-Weighted Mix RLN DoS Protection. — [lip.logos.co index](https://lip.logos.co/)
- Status timeline:
  - Mix was integrated into lightpush in Oct 2025.
  - In April 2026 the cover-traffic spec and implementation opened, and Mix was integrated with Logos Delivery and tested in local simulation.
  - The deliverable "Integrate mix into the Logos Delivery module" was reportedly closed 2026-10-12. Treat this date with caution, because the issue's date metadata is inconsistent.
  - A September 2026 PR says it does not claim full Mix spec conformance, and automatic mix peer discovery is out of scope. — [roadmap: mix](https://roadmap.logos.co/anoncomms/furps/mix); [Logos weekly 2026-04-13](https://roadmap.logos.co/reports/weekly/2026-04-13); [anoncomms-pm #53](https://github.com/logos-co/anoncomms-pm/issues/53); [logos-delivery #4356](https://github.com/logos-messaging/logos-delivery/issues/4356)

### Inferences
- **The first-hop peer is the messaging-layer first-spy.** Over a sparse mesh where each basecamp has one or two radio neighbours, Relay's neighbour-timing leak becomes near-certain origin attribution. Lightpush is worse: the service node knows the light client originated the message. Mix, or at least a Dandelion-style stem, is needed for any sender anonymity, and Mix's ≥3-hop requirement may be unachievable on a small RF mesh with few nodes.
- **Mix packet size versus payload.** A roughly 9.2 KB tx needs at least 3 Mix packets (13.8 KB on the wire). A burst of 3 fixed-size packets from the same node at the same time is linkable unless cover traffic is on. Mix's 4608 B packets are also very large for LoRa. If Mix runs only on IP-backhauled segments, the RF hop remains unprotected.
- **RLN in an intermittently connected mesh is hazardous:**
  1. Basecamps that can't refresh the membership Merkle root will publish with stale roots, which leaks tree position. Root updates must be synced over the mesh before publishing.
  2. Splitting a tx into many messages can exceed the per-epoch limit. Retransmissions, or the same RLN credential used on two devices (for example, a basecamp's primary and backup), can produce two different messages in one epoch. Either case leaks the secret, gets the member slashed and links the messages. The client must enforce the limit hard, and each device needs its own credential.
  3. Epochs and the ±20 s store timestamp window assume synchronized clocks, so offline basecamps need a disciplined clock such as GPS. Clock skew both drops messages and fingerprints nodes.
  4. RLN registration on-chain links the funding Ethereum account to a membership, though not to individual messages. Register with an unlinked account.
- **Store is a logging point.** Any store node retains the relayed bundle (encrypted payload, content topic, timestamp, size) for an unspecified period, and its operator can log querier PeerIDs and IPs. For payment bundles:
  - E2E-encrypt the payload.
  - Use a broad shared content topic.
  - Prefer `ephemeral=true` plus application-level store-and-forward between trusted basecamps, or accept store exposure in exchange for offline delivery.
  - Query store over Mix/Tor where possible.
- **Content topics should be generic and shared across all basecamps,** for example a single "/basecamp/1/payments/proto". Per-recipient or per-session topics reveal who is talking to whom. Noise-session topic rotation helps (§6).
- **PeerIDs should be ephemeral,** rotated per session or epoch, and never reused across the internet-facing gateway role and the mesh role.

### Gaps
- No published 2025–2026 quantitative anonymity analysis of Logos Delivery with Mix enabled was found, including anonymity-set size, latency and intersection-attack resistance.
- Not found: whether Logos Delivery mainnet enforces RLN today, which chain hosts the membership contract, and its epoch length and message limit. The docs conflict on this.
- No data on store-node default retention or logging in Logos Delivery implementations.
- The Mix cover-traffic spec contents (rates, whether senders must send constant-rate traffic) were not retrieved.

---

## 3. RF/mesh-layer threats: direction finding and triangulation, traffic analysis on low-bandwidth links, device fingerprinting, jamming, node capture

### Takeaway
At the radio layer, encryption hides content but not existence, timing, size, location or device identity. LoRa-class transmitters can be:
- localized (RSSI/TDOA);
- uniquely fingerprinted by hardware imperfections (deep-learning RFFI on commodity SDRs);
- selectively jammed by decoding just the header.

On a quiet low-bandwidth link, a roughly 9 KB burst at an unusual time is itself a strong "payment happened now" signal. Common mesh firmware has also had concrete key-management and header-privacy failures.

### Cited Findings
- **Device fingerprinting:**
  - Shen, Zhang et al. (IEEE JSAC 2021) built LoRa RF fingerprint identification from spectrograms with CNNs.
  - Their IEEE TIFS 2022 follow-up is scalable and channel-robust, identifying previously unseen devices with deep metric learning. It was evaluated with 60 COTS LoRa devices and a USRP N210. — [Liverpool repo (JSAC)](https://livrepository.liverpool.ac.uk/3112081/); [Liverpool repo (TIFS)](https://livrepository.liverpool.ac.uk/3148161/); [IEEE SPS webinar](https://signalprocessingsociety.org/blog/towards-scalable-and-channel-robust-radio-frequency-fingerprint-identification-lora)
- **Localization:** RSSI-based ML ranging and trilateration has been demonstrated in LoRa networks, indoors and outdoors. — [arXiv 2006.01278](https://arxiv.org/pdf/2006.01278). Patent literature cites TDOA multilateration accuracy of about 20–200 m depending on gateway density; this comes from a search summary of a patent and needs verification. — [USPTO 11782120](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/11782120)
- **Jamming:**
  - Selective jamming of LoRaWAN on commodity hardware decodes enough of a packet to identify the sender, then jams it. It cut the victim's packet reception rate to 1.3%, and triggered jamming cut it to 0.5%. — [Aras et al., arXiv 1712.02141](https://arxiv.org/pdf/1712.02141)
  - LoRa's deterministic, known preamble makes reactive jamming "feasible and straightforward." — [Dossa, Burg, Amhoud, arXiv 2501.18339](https://arxiv.org/html/2501.18339v1)
- **Meshtastic threat model (DEF CON 34 Ham Radio Village, 2026-08-08):**
  - The shared channel key gives confidentiality only. It gives no integrity, sender identity, header privacy or availability, and the default key ships on every node.
  - Cleartext headers expose who is talking, when, and the network shape.
  - Licensed mode transmits the callsign in the clear, and callsigns map to names and addresses.
  - Live demos showed forgery (AES-CTR ciphertext forgery, source-ID spoofing), targeted dropping and hop-limit rewriting, and node-DB exhaustion of about 100 slots with ghost nodes.
  - Recommendations: Reticulum for cryptographic identity and authenticated encryption by default, change default keys, verify out of band. — [DEF CON 34 HRV talk](https://cfp.hamvillage.org/dc34/talk/CTMAYM/)
- **Supply-chain key failure: CVE-2025-52464 (Meshtastic 2.5.0–2.6.10).** Vendor flashing produced **duplicate key pairs across devices**, and some platforms had low-entropy RNG. Holders of the compromised keys could decrypt DMs. Fixed in 2.6.11 and 2.6.12, which auto-wipes known-bad keys. The workaround is a full device wipe. — [NVD](https://nvd.nist.gov/vuln/detail/CVE-2025-52464); [OSV](https://osv.dev/vulnerability/CVE-2025-52464)
- A Hackfest 2025 talk lists physical attacks, privacy leaks, poor key management and jamming susceptibility as Meshtastic risks. — [Hackfest 2025](https://cfp.hackfest.ca/hf2025/talk/RUNCYK/)

### Inferences
- **Traffic analysis is the dominant RF privacy threat.** On a link that is otherwise idle or carries only beacons, a multi-packet ~9 KB transmission reveals that a payment-sized event happened and when, plus which node sent it (by fingerprint and direction finding) and roughly where. It may also reveal to whom, by watching which node later transmits, for example the gateway uplinking. Mitigations:
  - Constant-rate or scheduled transmission slots with fixed-size frames and dummy traffic.
  - Store-and-forward batching, so payments ride in regular sync bursts with compact blocks.
  - Uniform tx padding (§1).
  - No unique on-air identifiers (callsign-free modes where legal).
- **Airtime and legal constraints (assumption-dependent; verify with the physical-link researcher).** At LoRa SF7/125 kHz (assumed about 5.5 kbps), a 9.2 KB tx needs more than 13 s of raw airtime. At SF12 it needs minutes. Duty-cycle limits, such as the EU sub-band 1% limits, may force spreading over a long time, which increases exposure to direction finding and jamming. This argues for higher-bandwidth links (Wi-Fi HaLow, 802.11 long-range) for the tx payload, with LoRa only for small control or receipt messages.
- **Direction finding and fingerprinting can't be defeated cryptographically.** Operational mitigations:
  - Minimize transmit power and duration.
  - Use directional antennas toward known peers.
  - Relay through unattended repeater nodes placed away from basecamps, so a fixed relay gets located rather than people.
  - Rotate hardware.
  - Assume a determined local adversary can map every transmitter.
- **Jamming is cheap and selective.** Header-decoding jammers can target a specific node or the gateway. Mitigations include frequency or band diversity, multiple independent paths (RF plus sneakernet or QR), and the fact that the payment protocol tolerates delays up to the expiry window (§4).
- **Node capture.** A captured relay or repeater exposes its link keys, PeerID, RLN credential, routing tables and any cached store messages. With cleartext headers, as in Meshtastic, it can also enable forgery of other nodes' traffic. Use per-node credentials, link-layer authenticated encryption, and E2E payload encryption so a captured relay learns only metadata.

### Gaps
- No published work found that performs RFFI or direction finding specifically against Meshtastic, Reticulum or Logos-over-LoRa nodes. The closest is the generic LoRa literature.
- No published traffic-analysis study of payment-sized bursts on LoRa meshes was found.
- Exact LoRa data rates and regional duty-cycle figures were not verified here; they belong to the physical-link researcher.

---

## 4. Security threats: double-spend in the offline window, replay, censorship by relays, gateway withholding and front-running, Sybil/eclipse, DoS on constrained links, key compromise, supply chain, seizure/coercion, expiry in transit

### Takeaway
Zcash consensus prevents on-chain double-spends through the global nullifier set, and signatures bind the tx so relays can't alter it. Until the tx is mined, though:
- the recipient has **no finality**;
- the sender (or anyone who learns the spend key) can invalidate it by spending the same note elsewhere;
- any relay or gateway can drop, delay or time-shift it, up to its expiry.

Mesh-layer Sybil, eclipse and DoS attacks are under-protected: Mix has no Sybil or DoS defence by default, and RLN needs synchronized membership. Endpoints are the softest target, through key theft, cloned firmware keys and physical coercion.

### Cited Findings
- **Nullifiers and double-spend:** each Ironwood action reveals the nullifier of the spent note, and Ironwood keeps its own nullifier set. — [ZIP 229](https://zips.z.cash/zip-0229); [crypto.news](https://crypto.news/zcash-ironwood-upgrade-whats-changed-after-the-orchard-bug/)
- **Expiry:** a tx with expiry height N must be mined by block N. Blocks containing expired txs are invalid, and expired txs are dropped from the mempool. The default expiry is 40 blocks (about 50 min). — [ZIP 203](https://zips.z.cash/zip-0203)
- **Re-spending invalidated notes:** ZIP 315 proposes that if a tx is invalidated after revealing nullifiers, those notes be used in a note-management tx. This addresses linkage when a tx that already revealed nullifiers is abandoned. — [ZIP 315](https://zips.z.cash/zip-0315)
- **Server lies:** a malicious lightwalletd can show fake transactions ("double-spending can be just as bad"). The fix is to validate headers, PoW and the note commitment tree root. — [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf). ZIP 307 header validation checks only prevHash. — [ZIP 307](https://zips.z.cash/zip-0307)
- **Relay censorship:** Waku Relay censorship resistance is guaranteed only against a single-node attacker. Dropping by a linearly scaling botnet, or by a local, AS or global attacker, can DoS the network. — [Logos Research rlog](https://research.logos.co/rlog/wakuv2-relay-anon)
- **Mix:** delivery is probabilistic. Misbehaving nodes that delay or drop are indistinguishable from benign failure. There is no Sybil resistance and no default DoS protection. The exit can tamper. — [LIBP2P-MIX](https://lip.logos.co/anoncomms/raw/mix.html)
- **Store:** misbehaviour such as omitting messages is not analyzed (the model is passive), and future-timestamp injection is acknowledged. — [13/WAKU2-STORE](https://lip.logos.co/messaging/core/draft/13/store.html)
- **Eclipse:** attackers can GRAFT into a victim's GossipSub mesh if they know its multiaddress. The proposed countermeasure is accepting grafts only from distinct IPs or subnets. — [Logos Research rlog](https://research.logos.co/rlog/wakuv2-relay-anon)
- **Bitcoin-over-Tor:** the attacker controls which blocks and transactions the victim sees. — [Biryukov & Pustogarov](https://arxiv.org/abs/1410.6079)
- **Spam control:** RLN rate-limits publishers and slashes double-signalers. — [17/RLN-RELAY](https://lip.logos.co/messaging/core/draft/17/rln-relay.html)
- **Mesh firmware DoS and forgery:** demonstrated node-DB exhaustion, targeted packet dropping, hop-limit rewriting and forgery on Meshtastic. — [DEF CON 34 HRV](https://cfp.hamvillage.org/dc34/talk/CTMAYM/)
- **Supply chain:** vendor-flashed duplicate keys (CVE-2025-52464). — [NVD](https://nvd.nist.gov/vuln/detail/CVE-2025-52464)
- **Remote node attacks:** a Zcash node could be crashed remotely given a payment address, and payees could be identified via P2P timing (historical, patched). — [Tramèr et al.](https://eprint.iacr.org/2020/220)

### Inferences
- **Offline double-spend is the core security limitation, and it can't be fixed at the mesh layer.** The recipient can verify offline:
  - that the tx decrypts to a note for them;
  - that the proof and signatures verify against a known anchor;
  - that the nullifiers are not in its locally synced nullifier set.

  It can't know whether the sender broadcast a conflicting spend of the same note via another path. Options:
  - **(a)** Treat payments as final only after N confirmations (ZIP 315 suggests 10 for untrusted), learned via mesh-synced chain data.
  - **(b)** Use bonded or reputational trust between basecamps, which the offline-payment prior-art researcher should cover.
  - **(c)** Use FROST-custodied treasuries (§5), where a threshold of basecamps must co-sign, so a single basecamp can't double-spend unilaterally.
  - **(d)** Keep the offline window short by making gateways frequent.
- **Expiry trade-off.** A short expiry limits how long a malicious gateway can time-shift a withheld tx, and lets the sender safely re-issue after expiry. It also risks expiry in transit. A long expiry or no expiry lets a gateway hold the tx and release it at a moment of its choosing, which enables timing correlation or griefing, and leaves the sender unsure whether to re-send. A sender re-sending a fresh tx that spends the same notes is safe: only one can be mined, because of the nullifier conflict. Re-sending one that spends *different* notes risks paying twice if the first later lands.
- **Front-running.** The gateway can't modify a signed v6 tx, because spend-auth and binding signatures commit to the tx digest. Its power is limited to withholding, delaying, ordering and censoring, and to learning timing and origin. There is no meaningful MEV for a plain shielded transfer. *(Inference; ZIP 244 sighash not re-fetched here: [ZIP 244](https://zips.z.cash/zip-0244).)*
- **Replay.**
  - Zcash-level replay is impossible: a mined tx's nullifiers block re-inclusion.
  - Cross-network replay (testnet to mainnet) fails because the anchor must be a valid treestate root on that chain. Address encodings and network parameters also differ.
  - Messaging-level replay is handled by the seen-cache, RLN duplicate rejection and Mix tag tables. Application messages should still carry unique IDs so receipts and deliveries are idempotent.
- **Censorship and gateway withholding mitigations:**
  - Send through multiple independent gateways. The same signed tx is idempotent, so redundancy has no cost beyond extra origin exposure.
  - Require signed end-to-end receipts from the gateway that include the txid and broadcast time, and later confirm inclusion via mesh-synced blocks.
  - Monitor for txs that never appear on-chain.
- **Sybil and eclipse on a small mesh.** With few nodes, an attacker adding radio nodes can become every basecamp's neighbour. That makes it first-spy for all traffic, lets it feed false chain data (fake compact blocks, so recipients see fake payments) and lets it censor. Mitigations:
  - An allowlist of mesh peers (pre-shared node identities).
  - Header and PoW validation of synced chain data, which ZIP 307 and Hornby both call for.
  - Getting chain data from at least two independent gateways.
  - RLN or a permissioned membership for publishing.
- **DoS on constrained links.** One spammer can saturate a LoRa link's duty cycle. RLN only helps if all relays enforce it and membership is synchronized. Add link-layer admission control, per-peer quotas and priority for payment and receipt classes.
- **Expiry in transit.** This is a direct consequence of a 40-block default with intermittent uplinks. Choose the expiry from the expected gateway cadence, uniformly across basecamps (§1), and alert the sender if no inclusion receipt arrives before expiry.

### Gaps
- No published Zebra policy found on relaying txs close to expiry.
- No published analysis of eclipse attacks on Logos Delivery specifically, beyond the 2022 GRAFT note.
- No source found quantifying Sybil cost on the Logos Delivery network or for RLN membership pricing.

---

## 5. Key management and endpoint hardening: hardware wallets with shielded support, FROST threshold custody, spending/viewing key separation, PCZT air-gapped signing, secure elements, FDE, duress

### Takeaway
As of October 2026, both Keystone (air-gapped QR, via Zodl/Zashi) and Ledger (PCZT-based Orchard signing per Ledger's developer docs, with Ironwood support reported) can keep Zcash shielded spend-authorization keys off the online host. Ironwood specifics rest partly on low-quality sources.

PCZT (ZIP 374 draft, `pczt` crate) splits construction, proving and signing across roles. FROST spend-authorization (ZIP 312 draft, ZF frost-client/frostd) can give basecamp treasuries t-of-n custody, but its coordinator and signers are trusted with privacy. Viewing keys should be split by need: a UIVK for detection, a UFVK only for auditors.

### Cited Findings
- **Ledger.** Ledger's Zcash Signer Kit (official developer docs) works as follows:
  - The host builds a PCZT and streams the header, transparent inputs/outputs and Orchard actions to the device.
  - The device returns RedPallas spend-auth signatures per Orchard action and transparent signatures.
  - **The binding signature and final assembly happen on the host.**
  - The device can export a UFVK or raw Orchard FVK. Addresses and transactions are confirmed on-device.
  - Ironwood is not mentioned on the page. — [Ledger developer docs](https://developers.ledger.com/docs/device-interaction/dmk-ts/references/signers/zcash)
  - A September 2026 report says Ledger gained shielded ZEC signing support (weak, news-only). — [Crypto Briefing](https://cryptobriefing.com/zcash-ledger-shielded-transactions-support/)
  - Affiliate guides claim Ledger Wallet Desktop 4.21+ with Zcash app 3.9.4+ supports Ironwood but not Sapling or Orchard balances, and not mobile. These are low-quality sources and need verification. — [zeczcash.com](https://zeczcash.com/blog/zcash-ledger-wallet)
- **Keystone.**
  - Keystone calls itself "the first hardware wallet with native shielded Zcash support" with air-gapped keys. — [Keystone on X](https://x.com/KeystoneWallet/status/2053460042423349529)
  - The z.cash ecosystem page notes Keystone co-developed the Zashi integration for shielded ZEC and lists Zodl (aka Zashi). — [z.cash: Keystone](https://z.cash/ecosystem/keystone/)
  - Claims of Ironwood support and batch PCZT signing in firmware 3.0.2+ come from community or affiliate sources only. — [Coin Bureau](https://coinbureau.com/analysis/best-zcash-wallets) (*outdated: lists Ledger as transparent-only*); [Blofin](https://blofin.com/en/academy/education/zcash/zcash-hardware-wallet-support)
- **PCZT.** The PCZT API is defined in a draft of ZIP 374 and implemented by the `pczt` Rust crate. Its roles roughly mirror BIP 174/370, with Zcash-specific roles added. The crate's signer role signs internally, which has complicated external-signer integrations. — [nuttycom HackMD](https://hackmd.io/@nuttycom/rJm5Oac1bg); [docs.rs pczt](https://docs.rs/crate/pczt/latest)
- **FROST (ZIP 312, Draft):**
  - FROST is re-randomized so signatures verify as Sapling or Orchard spend-auth signatures. The randomizer must go over a **confidential** authenticated channel.
  - **The Coordinator is trusted with tx privacy and unlinkability.** It can't sign without the threshold. Rogue share holders can break privacy.
  - Signers must check that the SIGHASH matches the transaction, or compute it themselves.
  - Coordinator removal and network privacy are non-goals.
  - Key generation (trusted dealer or DKG) is out of scope. An independently generated `ask` allows DKG but **can't be recovered from a seed phrase**.
  - Ironwood is not mentioned. — [ZIP 312](https://zips.z.cash/zip-0312)
- ZF has shipped frost-core and ciphersuite crates, frostd (a server) and frost-client (a CLI and reference for wallet integration). FROST v3.0.0-rc.0 (2026) makes cheater detection the default and adds stronger zeroization. This is a release candidate. — [Zcash Shielded News Vol.32](https://zechub.substack.com/p/zcash-shielded-news-vol32); [Zcash Shielded News Vol.4](https://zechub.substack.com/p/zcash-shielded-news-vol4-0a5) *(I took these facts from search summaries, and which issue says which is not confirmed. Check the ZF FROST repo.)*
- **Viewing keys:** a UFVK reveals incoming, outgoing (via ovk) and change activity, while a UIVK reveals incoming payments only. — [ZIP 316](https://zips.z.cash/zip-0316)
- ZIP 315:
  - Generate a 24-word BIP 39 seed or a 33-word SLIP 39 share.
  - Auto-shielding needs ambient spending authority, so it is unsuitable for hardware-key wallets.
  - Do not expose internal (change) receivers. — [ZIP 315](https://zips.z.cash/zip-0315)
- **Duress and plausible deniability (generic):** passphrase "hidden wallets" derive independent wallets from one seed, so a decoy passphrase can be revealed under coercion. Some devices have dedicated duress PINs, which are not standard. Decoys can fail against informed attackers. — [Trezor blog](https://blog.trezor.io/passphrase-the-ultimate-protection-for-your-accounts-3a311990925b); [BitBox blog](https://blog.bitbox.swiss/en/optional-passphrases-benefits-and-risks/)
- **Supply-chain RNG and key cloning** at flashing time is a real failure mode (CVE-2025-52464). — [NVD](https://nvd.nist.gov/vuln/detail/CVE-2025-52464)

### Inferences
- **Recommended basecamp key architecture:**
  1. **Spend authority** (ask) lives on a hardware signer (Keystone air-gapped QR, or Ledger) or is FROST-shared across basecamps.
  2. **The online or mesh-connected builder** holds only the UFVK needed to construct PCZTs and generate proofs. The FVK/ovk still gives full history, so this host must be hardened and disk-encrypted.
  3. **Gateways and sync servers** get nothing, or at most a UIVK if delegated detection is unavoidable.
  4. **The recipient's detection device** holds an IVK.

  Proof generation happens on the host. Ledger's flow keeps the binding signature on the host, so the host learns all amounts. Tramèr et al. showed proving time can leak amounts, so build and prove offline, not on the gateway.
- **FROST for treasuries.** The ZIP 312 coordinator role conflicts with an offline mesh: rounds need several message exchanges among signers over a slow mesh. Commitments can be pre-generated in batches to cut rounds; this is standard FROST practice and needs verification for the ZF tools. The coordinator learns full tx contents, so pick a coordinator basecamp that already holds the FVK. Signers must independently verify the sighash, which means each needs the PCZT and enough chain state to check the anchor. DKG-generated keys can't be seed-recovered, so key-share backups (and FROST refresh/repair) need a plan. Ironwood applicability is plausible because Ironwood uses Orchard actions, RedPallas and rk, but this is **unconfirmed**.
- **Endpoint hardening checklist** (generic best practice, not a source-backed Zcash requirement):
  - Full-disk encryption with a strong passphrase. Tails or other amnesic OSes for the builder.
  - Secure-element signer.
  - Reproducible or verified firmware builds, and re-keying after receiving any vendor-flashed radio (per CVE-2025-52464).
  - Duress passphrases where the hardware wallet supports them for Zcash.
  - Minimal on-device history: wallets store decrypted notes and memos, so a seized builder exposes the full shielded history.
- **Physical seizure and coercion.**
  - A seized UFVK holder reveals history. A seized hardware signer with its PIN reveals spend authority.
  - FROST t-of-n means seizing one basecamp is not enough to spend.
  - Duress decoys are of uncertain value against informed adversaries.

### Gaps
- No official Keystone or Ledger changelog was retrieved confirming Ironwood support versions. Vendor duress or hidden-wallet support for Zcash shielded accounts is unverified.
- The ZIP 374 text and status were not retrieved. ZIP 312 status (still Draft, created "2022-08-dd") and Ironwood applicability are unconfirmed.
- No published review of FROST UX or latency over delay-tolerant or mesh networks.

---

## 6. App-level payload protection over Logos Messaging: Noise, X3DH/double ratchet, MLS/de-MLS, age; authenticated E2E receipts; binding a tx/note bundle to the recipient so relays learn nothing

### Takeaway
Logos provides Noise-based secure channels (WAKU2-NOISE and WAKU2-NOISE-SESSIONS, raw), X3DH and double-ratchet specs (draft/raw), and de-MLS-based group chat (raw). For basecamp-to-basecamp payments:
- a pre-established Noise or double-ratchet session, keyed from out-of-band-verified basecamp identities;
- session-ID-derived, rotating content topics;
- fixed-size padded payloads;
- signed, encrypted receipts.

Together these give relays nothing except size and timing. age is unsuitable as the only layer, because it provides no sender authentication and exposes the recipient count.

### Cited Findings
- **Available Logos specs:**
  - WAKU2-NOISE (raw) and WAKU2-NOISE-SESSIONS (raw);
  - 53/WAKU2-X3DH and 54/WAKU2-X3DH-SESSIONS (draft);
  - NOISE-X3DH-DOUBLE-RATCHET (raw);
  - DECENTRALIZED-MLS-OFFCHAIN-CONSENSUS (raw), the de-MLS basis;
  - MESSAGING-API, CHAT-FRAMEWORK and RELIABLE-CHANNEL-API (raw). — [lip.logos.co index](https://lip.logos.co/)
- Logos Chat provides E2EE for one-to-one and group chats, using de-MLS for groups. Confidentiality, integrity and authenticity are available through the Waku Message format and Noise. — [Logos docs: Messaging](https://docs.logos.co/messaging)
- **WAKU2-NOISE-SESSIONS (raw, rev. 2026-06-16):**
  - The session ID is derived from the handshake hash through HKDF.
  - Session traffic uses content topic `/{app}/{ver}/wakunoise/1/sessions/{ct-id}/proto`, where ct-id is a double hash of the session ID.
  - Session IDs are rotated with HKDF to change topics over time (metadata reduction). Sessions are marked stale privately or publicly.
  - Multi-device modes are N11M (shared state, with the risk that a compromised propagation message exposes the session) and NM (pairwise per device, Sesame-like). — [WAKU2-NOISE-SESSIONS](https://lip.logos.co/messaging/application/raw/noise-sessions.html)
- **age:**
  - No sender authentication: the X25519 stanza uses only an ephemeral key and the recipient key, and the header MAC is keyed from the file key.
  - The number of recipients is visible from the plaintext stanza lines.
  - Tagged recipient types sacrifice "ciphertext anonymity and unlinkability."
  - The payload is ChaCha20-Poly1305 STREAM with truncation detection. — [age spec (C2SP)](https://github.com/C2SP/C2SP/blob/main/age.md)
- The Mix exit sees the decrypted payload, so app-level E2E encryption is required on top of Mix. A destination may run its own final hop. — [LIBP2P-MIX](https://lip.logos.co/anoncomms/raw/mix.html)
- Zcash in-band secret distribution already encrypts each note to the recipient inside the tx (encCiphertext, 580 B). The tx itself carries the recipient's note. — [ZIP 225](https://zips.z.cash/zip-0225)
- "Liberated payments" and out-of-band tx delivery are identified by ECC research as a way to deliver txs privately through existing channels. Ideally, receiving such a payment would be indistinguishable from receiving a normal message. — [Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)

### Inferences
- **The raw tx is public data once broadcast, but before broadcast it is sensitive.** Its existence, timing, size and destination basecamp are metadata, and a relay holding it can broadcast it early, from a location the relay chooses. Wrap the signed tx in an E2E-encrypted envelope addressed to (a) the gateway and/or (b) the recipient basecamp. Relays then see only a fixed-size ciphertext on a shared topic.
- **Suggested envelope:**
  - Noise session (for example, a K/KK-style pattern with pre-shared static keys exchanged in person when basecamps are set up), or an X3DH-initialized double ratchet for forward secrecy and post-compromise security.
  - Payload: signed tx bytes, the txid, the intended expiry, an application message ID (for replay and idempotency) and an optional memo reference.
  - Pad to a fixed bucket, for example a multiple of the Mix δ=3984 B, or a fixed 12 KB for all payment messages, with dummy messages of the same size sent on a schedule.
  - Publish on a shared, generic content topic, or on Noise-session-derived rotating topics, and never on per-recipient static topics.
- **Recipient binding.**
  - The note is already cryptographically bound to the recipient by in-band encryption. Only the IVK holder can decrypt it, and the commitment binds value and recipient.
  - The E2E envelope additionally hides from relays which recipient the payment is for.
  - For the gateway path, encrypt to the gateway's key. The gateway necessarily learns the tx, which is public later anyway, and the time of submission.
  - Optionally deliver to the recipient and the gateway separately so that neither relay path learns both endpoints.
- **Authenticated end-to-end receipts.**
  - (1) The gateway returns a signed "broadcast at height H, txid X" receipt over the same session.
  - (2) The recipient returns a signed "received, decrypted, value V" receipt, keyed to the session.
  - (3) Final confirmation comes from mesh-synced blocks showing the txid mined.
  - Receipts must be padded the same as other traffic, or their size gives the flow away.
- **de-MLS** suits basecamp groups, such as a treasury signer group running FROST rounds. It is raw-spec maturity, so treat it as experimental.
- **age** may be acceptable for at-rest backup of PCZTs or keys. For transport it must be combined with a signature, for example sign-then-encrypt, and it leaks recipient count.

### Gaps
- No public security audit was found for the Logos Noise, X3DH or de-MLS implementations. All are raw or draft specs.
- The WAKU2-NOISE handshake patterns and any message-nametag mechanism were not described in the sessions spec retrieved. The base WAKU2-NOISE spec was not fetched.
- No Logos-native "receipt" or delivery-acknowledgement primitive with end-to-end authentication was confirmed. RELIABLE-CHANNEL-API exists (raw) but was not reviewed.

---

## 7. Consolidated threat model: assets, adversaries, threat table

### Takeaway
The highest residual risks are:
1. Offline double-spend or non-finality for recipients.
2. Origin and location attribution via RF direction finding and fingerprinting, and via the gateway as first-spy.
3. Mesh-layer Sybil, eclipse and DoS on a small, under-protected network.
4. Endpoint compromise or seizure of FVK-holding builders.

Cryptographic confidentiality of amounts and recipients is strong (Ironwood plus E2E envelope). Metadata privacy (who, when, where) is weak and only partially mitigable on low-bandwidth RF.

### Cited Findings
(All sources are as cited in sections 1–6. The table's mitigation and residual-risk judgments are synthesis; see Inferences.)

### Inferences

**Assets**
- A1 Spend authority (ask / spending key / FROST shares).
- A2 Viewing capability (UFVK incl. ovk; UIVK).
- A3 Unbroadcast signed txs and PCZTs.
- A4 Basecamp identities and locations (people).
- A5 Payment metadata: who paid whom, when, how much.
- A6 Mesh credentials (PeerIDs, Noise static keys, RLN secrets and stake).
- A7 Availability: timely delivery before expiry.
- A8 Integrity of chain data synced over the mesh (compact blocks, headers, anchors).

**Adversaries**
- ADV-L: local RF observer (direction finding, SDR fingerprinting, jamming).
- ADV-N: malicious mesh node or relay (single or Sybil swarm).
- ADV-G: malicious or compromised gateway.
- ADV-S: lightwalletd/Zaino/store-node operator and its hosting provider.
- ADV-P: Zcash P2P observer (well-connected spy nodes).
- ADV-GPA: global passive network adversary (internet backbone).
- ADV-E: endpoint attacker (malware, supply chain).
- ADV-C: physical captor or coercer.
- ADV-SND: dishonest sender (double-spend).

**Threat table**

| # | Threat | Adversary | Layer | Impact | Mitigation | Residual risk |
|---|---|---|---|---|---|---|
| T1 | Sender double-spends the same note via another path during the offline window | ADV-SND | Zcash/security | Recipient loses funds; goods released on an unconfirmed tx | Treat as final only after N confirmations (ZIP 315: 10 untrusted) seen via validated, mesh-synced headers; FROST t-of-n treasuries; inter-basecamp bonds/reputation; frequent uplinks | **High.** Inherent to offline UTXO/note payments; can only be bounded by trust or waiting |
| T2 | Gateway is the "first-spy" broadcaster, linking txs to the mesh and its location | ADV-P, ADV-S, ADV-GPA | Zcash network | Every mesh tx attributed to the basecamp network; small anonymity set | Submit over Tor (fresh circuit per tx; Zodl does this); rotate multiple gateways; randomized broadcast delay; batch with dummy/self-transfers; Dandelion/Stolon if available | **Medium.** Tor helps but a GPA defeats it (Kappos & Piotrowska); Bitcoin-over-Tor style attacks on long-lived nodes |
| T3 | Stale anchor and unusual expiry delta fingerprint offline-built txs | ADV-P (chain analyst) | Zcash on-chain | Txs identified as "offline mesh," shrinking the anonymity set | Freshest feasible anchor via streamed compact blocks; one uniform expiry policy across basecamps; lock_time 0; v6 | **Medium.** Delay is intrinsic; best case is blending with other offline users |
| T4 | Tx size, action count and fee reveal tx shape | ADV-P, ADV-L | Zcash + RF | Distinguishes payment types and wallets; size-matches RF bursts to on-chain txs | Pad all txs to a fixed action count; ZIP 317 standard fee only; pad mesh envelopes to fixed buckets | **Low–Medium.** The padded count is still visible but uniform |
| T5 | Amount and timing correlation (shielding/turnstile/de-shield amounts; on-chain appearance right after an uplink window) | ADV-P | Zcash on-chain | Links a basecamp's funding to its payments | Shield early in common denominations; avoid unique amounts; never de-shield the same amount; per-tx opt-in for cross-pool (ZIP 315); spread broadcasts | **Medium.** Turnstile and transparent faucet amounts are inherently public |
| T6 | Light-client server learns IP, sync height and full-tx fetches; may lie about chain | ADV-S | Zcash light client | Wallet linkage, tx graph, fake payments | Gateway broadcasts compact blocks to a shared topic (no per-basecamp queries); Tor for gateway–server; no memo fetches (sender delivers full tx); validate headers and PoW; ≥2 independent servers | **Low–Medium.** Gateway–server link still leaks gateway activity |
| T7 | Viewing-key exposure (UFVK given to a server or gateway, or seized builder) | ADV-S, ADV-E, ADV-C | Key management | Full incoming, outgoing and change history incl. memos | Never share a UFVK; delegate a UIVK only; FDE; amnesic builder OS; minimal stored history | **Medium.** A seized, unlocked builder still reveals history |
| T8 | Spend-key theft from a basecamp device | ADV-E, ADV-C | Endpoint | Total loss of funds | Hardware signer (Keystone air-gapped / Ledger PCZT); FROST t-of-n; verified firmware; PIN/passphrase | **Low–Medium.** Ironwood HW support partly unverified; host still sees amounts (Ledger binding sig on host) |
| T9 | Supply-chain compromise of radio or wallet firmware (cloned keys, weak RNG, backdoor) | ADV-E | Endpoint/RF | Decryption of link traffic; impersonation; key theft | Re-key after purchase; verify builds; prefer open-source firmware; independent E2E layer over link encryption | **Medium.** Hard to verify hardware; CVE-2025-52464 precedent |
| T10 | Neighbour timing reveals message originator at the messaging layer | ADV-N | Messaging | Sender attribution (basecamp X sent a payment) | libp2p Mix (≥3 hops, delays); Dandelion-style stem; generic shared topics; ephemeral PeerIDs | **Medium–High** on small meshes. Mix needs enough mix nodes; no Sybil resistance; GPA not addressed |
| T11 | Store/Filter/Lightpush service nodes learn PeerID, content topics and timing; store logs bundles | ADV-S, ADV-N | Messaging | Interest linkage; retained ciphertext for later analysis | E2E envelope; broad topics; ephemeral=true where possible; query over Mix/Tor; trusted store nodes at basecamps | **Medium.** Retention and logging policy unspecified |
| T12 | RLN double-signal (split bundles, retransmits, shared credential on two devices) leaks the secret | Self-inflicted, observed by ADV-N | Messaging | Slashing, removal, linkage of messages in the epoch | Strict client-side rate limiting; one credential per device; RLN-V2 with an adequate user_message_limit; chunk scheduling across epochs | **Low** if engineered carefully; **High** if ignored |
| T13 | RLN stale Merkle root reveals member position; clock skew breaks epochs | ADV-N | Messaging | Deanonymization of an offline basecamp; message drops | Sync membership root over the mesh before publishing; GPS-disciplined clocks; tune max_epoch_gap | **Medium.** Intermittent connectivity makes stale roots likely |
| T14 | Mix exit or relay reads, alters or drops payload | ADV-N | Messaging | Payload exposure if not E2E; censorship | App-level E2E plus signatures; recipient runs its own final hop; redundant paths | **Low** for content; **Medium** for censorship |
| T15 | Malicious relays drop, delay or reorder (censorship) | ADV-N | Messaging/mesh | Payment delayed past expiry; DoS | Multi-path or multi-gateway sends (idempotent tx); E2E receipts with timeouts; peer allowlists | **Medium.** Misbehaviour is indistinguishable from failure (Mix spec) |
| T16 | Gateway withholds, delays or time-shifts the tx | ADV-G | Gateway | Timing correlation; griefing; sender uncertainty | Moderate uniform expiry; multiple gateways; signed broadcast receipts; sender may cancel by re-spending the same notes | **Medium** |
| T17 | Gateway or relay modifies or "front-runs" the tx | ADV-G | Zcash | None beyond ordering (signatures bind the tx) | None needed beyond verification; no MEV in plain shielded transfers | **Low** |
| T18 | Replay of txs or messages | ADV-N | Zcash/messaging | Duplicate processing | Nullifiers (chain); anchor binds the chain; seen-cache, RLN duplicate rejection, Mix tags; app message IDs | **Low** |
| T19 | Sybil/eclipse: attacker becomes all of a basecamp's peers and feeds fake chain data | ADV-N | Mesh/messaging | Fake payments shown; censorship; total traffic visibility | Pre-shared peer allowlist; header and PoW validation; ≥2 independent chain-data sources; GRAFT IP/subnet diversity | **Medium–High** on small RF meshes |
| T20 | DoS/spam on constrained links (duty cycle exhaustion, node-DB exhaustion) | ADV-N, ADV-L | Mesh/RF | Payments cannot propagate before expiry | RLN everywhere; per-peer quotas; priority classes; band diversity; out-of-band fallback (QR/sneakernet) | **Medium–High** |
| T21 | Radio direction finding and triangulation of transmitters | ADV-L | RF | Physical location of basecamps and people | Low power, short bursts, directional antennas, remote unattended repeaters, scheduled transmissions | **High** against a determined local adversary |
| T22 | RF fingerprinting of devices (RFFI) | ADV-L | RF | Persistent device tracking despite ID rotation | Hardware rotation; shared relay nodes; accept the risk | **High.** No cryptographic fix |
| T23 | Traffic analysis of payment-sized bursts on low-bandwidth links | ADV-L | RF | Reveals that and when a payment occurred, and between which nodes | Constant-rate or scheduled slots; fixed-size frames; dummy traffic; batch with block sync | **Medium–High.** Bandwidth cost of cover traffic on LoRa is severe |
| T24 | Selective or reactive jamming of a node or gateway | ADV-L | RF | Censorship; expiry in transit | Frequency or band diversity; multiple paths; delay-tolerant expiry; physical sneakernet fallback | **Medium–High** |
| T25 | Node capture (relay or repeater seized or tampered) | ADV-C, ADV-N | RF/mesh | Link keys, RLN credential, cached store messages; forgery if headers are unauthenticated | Per-node credentials; authenticated link encryption; E2E payloads; remote revocation; tamper-evident enclosures | **Medium** |
| T26 | Physical seizure or coercion of operators | ADV-C | Endpoint | Spend or view keys disclosed | FROST t-of-n across basecamps; duress passphrase wallets (if supported); minimal stored history | **Medium–High.** Duress features unverified for Zcash shielded |
| T27 | Tx expires while in transit | Environment, ADV-G, ADV-N | Zcash/ops | Payment fails; sender confusion | Expiry delta sized to the gateway cadence, uniform across basecamps; receipts with alarms; re-issue with the same notes | **Low–Medium** |
| T28 | Proving-time and P2P side channels (Tramèr et al.) | ADV-P | Zcash implementation | Amount or payee leakage | Prove offline, not on internet-exposed hosts; don't run a wallet inside a P2P-exposed node | **Low** (patched, but the architecture lesson stands) |
| T29 | Mesh firmware forgery and cleartext headers (Meshtastic-class) | ADV-N, ADV-L | RF/mesh | Impersonation; network-shape and location leakage | Authenticated-encryption link layer (Reticulum-style); don't rely on channel PSKs; E2E above | **Medium** (depends on chosen radio stack) |
| T30 | Protocol immaturity: Ironwood privacy unverified formally; Logos Mix, Noise and de-MLS raw specs; RLN circuits unaudited | All | Cross-layer | Unknown bugs | Testnet-only use (as planned); defence in depth; track audits | **Medium** |

**Outdated or superseded material flagged:**
- Kappos et al. 2018 (Sprout era).
- The Biryukov Zcash network work (2019, zcashd era; Zebra is now mandatory).
- The Hornby light-wallet deck (circa 2020–21; Zodl now has Tor, and Zaino is replacing lightwalletd).
- Tramèr et al. 2020 (patched).
- The Waku relay anonymity rlog 2022 (pre-RLN mainnet, pre-Mix).
- Coin Bureau's hardware-wallet list (says Ledger is transparent-only; contradicted by newer sources).
- The Logos docs conflict on RLN status.

### Gaps
- Residual-risk ratings are qualitative judgments. No published quantitative risk model exists for this specific architecture.
- No published end-to-end analysis was found that combines Zcash with a Waku/Logos mesh or LoRa, so the cross-layer correlation risks (T4, T23, T2) are inferred rather than measured.
