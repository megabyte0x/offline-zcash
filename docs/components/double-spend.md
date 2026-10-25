# Double-spend controls and acceptance tiers

No offline check proves a note is unspent, so a "pending" payment is an IOU from a known payer and becomes final only when mined.
MVP v1 bounds the loss with card-network policy kept on the payee. Prevention arrives next as a bilateral 2-of-2 FROST escrow between payer and payee.
Decided 25 October 2026 from two design reviews ([reconciliation](../../archive/reports/Design%20review%20reconciliation.md)). These decisions override older archive text (corrections listed at the end).

Related: [payee-verifier.md](payee-verifier.md) (offline checks, receipts, states), [transport-bitchat.md](transport-bitchat.md), [gateway.md](gateway.md), [../privacy-and-threats.md](../privacy-and-threats.md).

## Decided

| Tier | Payee holds | Stops payer double-spend? | Status |
|---|---|---|---|
| Pending | Transaction that passes offline checks at height H | No. Bounded by floor limit and per-payer counter | MVP v1 [decided] |
| Bilateral escrow | Transaction the payee itself co-signed (2-of-2 FROST) | Yes, unless the payer's share leaks | Next phase [decided], untested |
| Quorum-certified | Payer + any 2f+1 of 3f+1 FROST signers | Yes, if ≤ f signers misbehave | Out of the 3-device MVP [decided] |
| Final | Mined transaction, inclusion proof and headers | Yes (consensus) | Every payment, eventually [decided] |

- **Pending is an IOU from a known payer.** No offline check proves inputs unspent. No published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. [decided]
- **MVP v1:** pending tier only. The payee keeps a floor limit and a per-payer counter. Payment details and the payer's accountability signature go only to the payee; relays and the gateway get transaction bytes only. Conflicts are detected from chain data. Accept small amounts from known payers. Final only when mined. [decided]
- **Layer 1, FROST quorum: out of the MVP.** It needs at least 4 signers plus the payer, and the demo has 3 devices. FROST is a signing tool, not anti-double-spend by itself: two colluding authorities plus a malicious payer can sign conflicting spends when quorums do not intersect in an honest member. A safe deployment needs authenticated DKG, signer enrollment, an atomic nullifier lock recorded before any share is released, no unlock on timeout, and no nonce reuse. frost-tools' confirm step just prints hex and asks y/n. [decided]
- **Layer 2, bonded accountable intents: dropped as written.** Zakura 1.6.0 ignores `lock_time` on fully shielded transactions (`zakura-chain/src/transaction.rs` L620–644 at `ebda15e`, pinned by ths v0.4.1; Zebra main has the same logic), so a bond has no refund path. A committee-held bond is custody. Counters cannot cap total loss across partitions: a k=1 credential shown to 100 disconnected payees exposes 100× the limit (k-times anonymous authentication only detects the (k+1)-th use afterwards; Teranishi, Furukawa, Sako, ASIACRYPT 2004; Camenisch et al., "How to Win the Clone Wars", CCS 2006). Counters are local loss controls only. [decided]
- **Layer 3, conflict detection: kept.** A nullifier mined in a different txid is conflict evidence, not fraud: an honest re-issue, a restored seed on a second device and key theft look the same. Hot lists hold nullifiers only, never payer keys. Nothing is hot-listed automatically. [decided]
- **Next phase: bilateral 2-of-2 FROST escrow, payer + payee.** The only prevention available with three devices. Details below. [decided]
- **Later:** 4-signer quorum; basecamp committees in the Logos phase. [decided]

## Findings

### Why prevention needs trust or waiting

- Three independent reasons no offline payee can know a note is unspent: a nullifier snapshot says nothing about spends after it (CAP; [Grothoff & Dold 2021](https://git.gnunet.org/taler/marketing/file/papers/2021-offline/offline.tex.html)); headers commit to note-commitment roots but not to a nullifier set ([ZIP 221](https://zips.z.cash/zip-0221), L660–663); the spend-key holder can sign a second spend at any time, and consensus deduplicates only at mining.
- "Preventing double-spending is literally the only problem that blockchains solve" (nuttycom, [Zcash forum](https://forum.zcashcommunity.com/t/how-to-pay-offline-with-zcash-in-any-situation/50167)). IBM calls offline double-spending "trivial" without connectivity ([ePrint 2024/1746](https://eprint.iacr.org/2024/1746)).
- The open question is stated and unanswered in the Delving Bitcoin thread "Is the pre-finality double-spend race intrinsic to single-owner coins?" (Oct 2026).
- Rerandomized spend-auth signatures do not identify the signer, so the nullifier match alone is the evidence.

### Card-network offline acceptance: what transfers

Card networks do not prevent offline losses; they bound and assign them. Offline EMV approval proves the card is genuine, never the balance ([US Payments Forum 2016](https://www.emv-connection.com/downloads/2016/04/Merchant-Processing-during-Communication-Disruption-FINAL-April-2016.pdf)). Source note: [card_network_offline_acceptance.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/card_network_offline_acceptance.md).

| Card control | Source | Here |
|---|---|---|
| Floor limit: above it, go online | [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf) | Transfers: pending only below a fleet amount [decided] |
| Consecutive-offline counters (tags 9F14, 9F23) enforced in the card's secure element | [OpenSCDP EMV](https://www.openscdp.org/scripts/tutorial/emv/terminalriskmanagement.html) | Partly: payee counts per payer locally; fleet exposure = payees × limit; no tamper-resistant counter |
| Random online processing below the floor | Visa Core Rules | Needs a reachable co-signer; not in MVP v1 [unverified value] |
| Pre-departure hot list | [American Banker / GuestLogix](https://www.americanbanker.com/payments/news/guestlogix-rolls-out-travel-industry-hot-list-to-block-fraudulent-cards) | Transfers as a nullifier list in chain bundles, never payer keys |
| Named loss owner: "the airline absorbs that loss"; merchant "fully liable" | American Banker; [Adyen-based guide](https://support.ventrata.com/en/articles/13384674-how-to-activate-offline-payments-for-adyen-terminals) | Transfers: the payee accepting at pending absorbs a failure |
| Sync deadline (24 h; 4 days transit) | Visa Rules 5.7.3.3 | Maps to the fleet expiry delta |
| Issuer collects a failed charge as debt | — | Does not transfer: ZEC is a bearer asset; a double-spender keeps the coins. Recourse is an off-chain agreement among known payers |

The Ironwood payee's offline check is stronger than a card's (the proof shows the notes existed at the stated anchor), but only once the anchor is checked against known roots ([payee-verifier.md](payee-verifier.md)).

### Prior art: what transfers and what doesn't

Source note: [offline_payment_prior_art.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/offline_payment_prior_art.md), [decentralized_double_spend.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/decentralized_double_spend.md).

| Approach | Source | Transfers? |
|---|---|---|
| Chaum ecash: one spend anonymous, a double-spend reveals identity | Chaum, Fiat, Naor, CRYPTO '88; Compact E-Cash, EUROCRYPT 2005 | Detect-and-deter needs an issuer and identity; Zcash conflicts reveal no identity |
| Consensusless quorums: 2f+1 of 3f+1, intersecting quorums | FastPay, AFT 2020, [arXiv 2003.11506](https://arxiv.org/abs/2003.11506); Guerraoui et al., PODC 2019, [arXiv 1906.05574](https://arxiv.org/abs/1906.05574) | Yes, as layer 1, but needs ≥4 signers; equivocating payer locks the note |
| Collateral/statekeepers, penalty deposits | Snappy, NDSS 2020; Brick, FC 2021; "Liar, Liar, Coins on Fire", CCS 2015 | No: need scripts or timelocks the shielded pool lacks |
| Trusted-hardware wallets (CBDC secure elements) | BIS Polaris 2023; IBM ePrint 2024/1746; Brands, CRYPTO '93 | No Zcash spend-once applet; one break allows unlimited double-spends; seed backup defeats it |
| Store-and-forward relays (txTenna, Meshtastic bridges) | [txTenna](https://github.com/remyers/txTenna) | Yes, as transport; none solves double-spend |
| Custodial ecash over mesh (Cashu/Fedimint over BitChat) | [Cashu NUT-11](https://cashubtc.github.io/nuts/11/) | Custodial, not Zcash; offline sender can still double-spend |
| Bearer URIs (ZIP 324) | [ZIP 324](https://zips.z.cash/zip-0324) | No: Draft, Sapling-only, sender can reclaim, interception equals theft |
| Lightning-style channels | — | No: no shielded scripts or timelocks |

### Bilateral escrow feasibility (next phase)

- [ZcashFoundation/frost-tools](https://github.com/ZcashFoundation/frost-tools) at `53df99c` supports Ironwood v6 (commit `06c0dbd`), but on upstream crates (`orchard` 0.15.5 via the conradoplg fork, `zcash_primitives` 0.29) that cannot link with Zakura 2.2.0. `frost-client` itself exchanges only bytes (sighash, α, signature). README: demo only.
- zakura-orchard 2.2.0 has the `unstable-frost` feature, `SpendValidatingKey::from_bytes` (`keys.rs` L193, L205–221) and `FullViewingKey::from_bytes` (L440–453). zakura-pczt 0.1.0-rc4 has `sign_ironwood_with`.
- Use the PCZT path: the builder's `SigningParts` has no public accessor for α.
- Estimated glue: about 300–500 lines porting `zcash-sign` to zakura-orchard and zakura-pczt. [unverified]
- Costs: escrow FVKs are non-standard and not quantum-recoverable (no ZIP 2005 `use_qsk`; frost-tools uses `from_sk_ak_incompatible_with_quantum_recoverability_and_will_be_removed`); FROST shares are not seed-restorable; the payee sees the escrow FVK; the payer can strand the payee's re-issue after expiry or a branch-ID change, because re-signing needs the payer's share; funds must be escrowed while online.

## Open

1. [unverified] frost-tools sighash, α and signature bytes interoperate with a Zakura 2.2.0 Ironwood transaction on the local NU7 chain (design open check 8).
2. [unverified] The ~300–500-line glue estimate; a 2-of-2 escrow note can be funded, co-signed via `sign_ironwood_with` and mined.
3. Test: deliberate conflicting spend ends as "conflicted" at the payee, not "final" (reconciliation §6 test 4).
4. [unverified] Floor limit and counter values: local chain coins are worthless, so simulate with notional amounts.
5. Open design: payee registry rules for honest re-issue (same payee and amount only), nonce-reuse protection across crashes, share backup.
6. Open design: how a payee learns earlier pending payments by the same payer to other payees. Probably impossible without a co-signer.
7. [unverified] How often a 4-signer quorum would be reachable on a real mesh.

### Corrections to archive text

- Report and research note: "intent plus chain fraud proof" and "attributable evidence" → conflict evidence only.
- Research note §5: "k × floor limit bounds total liability across partitions" → false; local control only.
- Card note §3.4 and report: hot list of payer basecamp keys → nullifiers only, never automatic.
- Research note §10.4 and report: `lock_time` refund path open → resolved, not enforced; layer 2 dropped.
- Research note §9 staging (signed intents gossiped mesh-wide in MVP v1) → details to the payee only.
