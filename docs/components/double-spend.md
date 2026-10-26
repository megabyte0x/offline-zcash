# Double-spend controls and acceptance tiers

No offline check proves a note is unspent, so a "pending" payment is an IOU from a known payer and becomes final only when mined.
MVP v1 bounds the loss with card-network policy kept on the payee, plus two detection aids: payees share pending nullifiers inside a mesh partition, and the gateway returns a mempool receipt. Prevention arrives in v1.1 as one **nullifier-lock co-signer**: a 2-of-2 FROST share-holder that signs at most one distinct spend per nullifier and needs no viewing key. Recovery from a lost co-signer is v1.2.
Decided 25 October 2026 from two design reviews ([reconciliation](../../archive/reports/Design%20review%20reconciliation.md)) and three research threads the same day: R1 topologies, R2 FROST in source, R3 pending tier. These decisions override older archive text and the first version of this file (corrections listed at the end).

Related: [pending-tier.md](pending-tier.md) (statement, receipt, nullifier sharing, policy), [payee-verifier.md](payee-verifier.md) (offline checks, states), [offline-builder.md](offline-builder.md), [gateway.md](gateway.md), [../privacy-and-threats.md](../privacy-and-threats.md).

## Decided

| Tier | Payee holds | Stops payer double-spend? | Status |
|---|---|---|---|
| Pending | Transaction that passes offline checks at height H, plus the payer's signed PaymentStatement | No. Bounded by floor limit and per-payer caps. A second spend of the same note inside the same mesh partition is refused at acceptance (nullifier sharing) | MVP v1 [decided] |
| Escrow (co-signed) | Transaction whose spends come from a 2-of-2 FROST escrow key and were co-signed by the nullifier-lock co-signer | Yes, while the co-signer is honest and its lock store is intact | v1.1 [decided] |
| Quorum-certified | Payer + any 2f+1 of 3f+1 FROST signers | Yes, if ≤ f signers misbehave | Logos phase [decided] |
| Final | Mined transaction, inclusion proof and headers | Yes (consensus) | Every payment, eventually [decided] |

### Pending tier (MVP v1)

- **Pending is an IOU from a known payer.** No offline check proves inputs unspent. No published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. [decided]
- The payee keeps a floor limit and per-payer count and value caps. The payer's 276-byte **PaymentStatement**, signed by a device-bound Ed25519 payment key, and the payee's 260-byte **Receipt** go only between payer and payee; relays and the gateway get transaction bytes only. Formats, policy defaults and persistence are in [pending-tier.md](pending-tier.md). [decided]
- **Fee check:** `valueBalanceIronwood` must equal the fleet fee for the action count, which blocks a transaction built never to be mined. [decided]
- **Nullifier sharing:** payees post (txid, expiry, nullifiers) for each pending payment to a fleet-payee BitChat group; a match from another payee with a different txid is refused. Coverage is one partition, after propagation. [decided]
- **Gateway mempool receipt:** the gateway acknowledges acceptance into the node mempool; a payee that can reach the gateway waits for it above a soft threshold. [decided]
- Conflicts are detected from chain data (and, inside a partition, from shared nullifiers). A nullifier mined in a different txid is **conflict evidence, not fraud**: an honest re-issue, a restored seed and key theft look the same. With a PaymentStatement it becomes a debt claim against the payer's device key under the fleet agreement, still not proof of fraud. Hot lists hold nullifiers only, never payer keys; nothing is hot-listed automatically. [decided]
- Counters are local loss controls. Across partitions one payer can expose payees × limit; sharing lowers "payees" to "partitions". [decided]

### Escrow tier (v1.1): the nullifier-lock co-signer

- **One component, two deployments.** A 2-of-2 FROST key (RedPallas, `zakura-reddsa` `frost` feature) between payer A and a co-signer. Deploy first as A + a **mesh co-signer** (demo: Mac C, offline); the **bilateral** form with the payee as co-signer is the same component in a second configuration and a real deployment mode for recurring payer–payee pairs, where it gives the payee a transaction it co-signed itself and allows cumulative-state payments from one note. [decided]
- **The co-signer holds one share and no viewing key.** It receives the redacted PCZT, recomputes the sighash itself, reads the action nullifiers, checks `rk = ak.randomize(α)` against the certified group key, checks that the outputs pay the stated payee (cmx), and signs. [decided]
- **Atomic lock, then release.** Before releasing a share the co-signer writes, in one fsynced transaction: nullifier → digest of the actions, and the consumed nonce. A crash between the write and the release must never yield two releases. [decided]
- **One distinct spend per nullifier.** Exclusive locking remains decided. Correction: identical actions alone do not fix flags/value balance/pool/format. Complete effecting-payload locks, named expiry/supported-branch changes and retained canonical effects are **proposed, awaiting owner decision** ([expiry-reissue.md #3](expiry-reissue.md#proposed--awaiting-owner-decision)). Compatible anchor/proof changes are authorizing data and need no new signing session. Same-input versions exclude each other on one valid chain, not across orphaned histories.
- **Expiry cap.** The fleet-cap / no-zero direction remains, but a rolling relative cap is **not** a recovery fence: renewals could extend it forever. An immutable absolute key-epoch fence fixed at lock creation, never advanced by renewal, is **proposed, awaiting owner decision** (#3); enforcement at A/B/C is proposal #9. Do not claim v1.2 recovery safety from the relative cap alone.
- **Two-tier wallet.** Escrow notes are double-spend-proof but unusable when the co-signer is unreachable; ordinary notes fall back to the pending tier. The PaymentStatement flags carry the tier, so the receipt does too. [decided]
- **Payee check.** The payee verifies that each spend's `rk` derives from the certified group key Y using α, which A sends privately. [decided]
- **Hostage accepted for v1.1.** In 2-of-2 either side can strand the other, and a lost share cannot be repaired. Keep escrow balances small, like cash in a wallet. [decided]
- **A's share may be backed up.** It is useless for a double-spend without the co-signer. The co-signer's share and lock store are the sensitive state. [decided]
- Costs that remain: the escrow FVK is non-standard (ZIP 2005 `use_qsk` derivation done by us; not quantum-recoverable until checked against vectors); FROST shares are not seed-restorable; the co-signer learns which txids are A's and holds partial spending authority (P10 in [privacy-and-threats.md](../privacy-and-threats.md)); funds must be escrowed while online. [decided]

### Recovery (v1.2)

- Choose between **2-of-3 {A, hot co-signer, cold recovery share R, fenced}**, where R signs only a sweep back to A and only after the chain passes the co-signer's expiry cap, and **timelock-encrypted release** of the co-signer's share to A (tlock/drand or an RSW puzzle, with on-chain rotation each epoch). [decided]
- Never 2-of-3 with a cold share held by A (A signs alone) or with the payee as the third holder (A plus any share-holding payee bypasses the lock; payee plus co-signer can steal). [decided]

### Layers kept, dropped, deferred

- **FROST quorum (2f+1 of 3f+1): Logos phase.** Needs ≥ 4 signers plus the payer. Two colluding authorities plus a malicious payer can sign conflicting spends when quorums do not intersect in an honest member. A safe deployment needs authenticated DKG, signer enrollment, the atomic nullifier lock above, no unlock on timeout and no nonce reuse. The v1.1 component is that lock for one signer. [decided]
- **Bonded accountable intents: dropped.** Zakura 1.6.0 ignores `lock_time` on fully shielded transactions (`zakura-chain/src/transaction.rs` L620–644 at `ebda15e`, pinned by ths v0.4.1; Zebra main has the same logic), so a bond has no refund path. A committee-held bond is custody. Counters cannot cap total loss across partitions (k-times anonymous authentication only detects the (k+1)-th use afterwards; Teranishi, Furukawa, Sako, ASIACRYPT 2004; Camenisch et al., CCS 2006). [decided]
- **Conflict detection: kept**, with the debt-claim reading above. [decided]

## Findings

### Why prevention needs trust or waiting

- Three independent reasons no offline payee can know a note is unspent: a nullifier snapshot says nothing about spends after it (CAP; [Grothoff & Dold 2021](https://git.gnunet.org/taler/marketing/file/papers/2021-offline/offline.tex.html)); headers commit to note-commitment roots but not to a nullifier set ([ZIP 221](https://zips.z.cash/zip-0221), L660–663); the spend-key holder can sign a second spend at any time, and consensus deduplicates only at mining.
- "Preventing double-spending is literally the only problem that blockchains solve" (nuttycom, [Zcash forum](https://forum.zcashcommunity.com/t/how-to-pay-offline-with-zcash-in-any-situation/50167)). IBM calls offline double-spending "trivial" without connectivity ([ePrint 2024/1746](https://eprint.iacr.org/2024/1746)).
- On a shielded note with no timelocks, any spend path that skips the lock holder is a double-spend path unless time gates it. So no topology avoids both the hostage problem and the double-spend without trust (R1).
- Rerandomized spend-auth signatures do not identify the signer, so the nullifier match alone is the evidence.

### Co-signer facts verified in source (R1, R2)

Crates at Zakura Common 2.2.0 (`zakura-core/common@b6cf4ce`, byte-identical to crates.io), zakura-pczt 0.1.0-rc4, frost-tools `53df99c`.

- **No viewing key needed.** The nullifier and `rk` are in the v6 txid and sighash digest (zakura-orchard `bundle/commitments.rs` L148, L156), and the circuit binds `rk = ak + [α]G` and `nf` to the note. A co-signer therefore knows exactly which notes any transaction it signs spends, and a fake nullifier gives an unprovable transaction. Protocol reasoning, not run against a hostile builder [unverified in that sense].
- **The co-signer recomputes the sighash.** zakura-pczt `Signer::new` (`roles/signer/mod.rs` L134–171) and `shielded_sighash()` (L181) work from effects data; for v6 the anchor is not required before authorization (`common.rs` L61–75), because the v6 anchor is auth data (`commitments.rs` L98–107). Nullifiers: `pczt.ironwood().actions()[i].spend().nullifier()`. α: `orchard::pczt::Spend::alpha()` (pczt.rs L168, L236). `rk` check: `SpendValidatingKey::randomize(&self, &pallas::Scalar) -> VerificationKey<SpendAuth>` (keys.rs L187). The Redactor can clear value, recipient, rseed, fvk, witness, rcv and alpha (`roles/redactor/orchard.rs`).
- **α is chosen by the PCZT constructor, the payer**: `pallas::Scalar::random(&mut rng)` in zakura-orchard `builder.rs` L451; the Updater has no α setter. frost-tools only reads it (`zcash-sign/src/sign.rs` L164–176). frost-rerandomized 3.0.0 deprecates explicit-randomizer `sign` (`lib.rs` L132) in favour of α = H(seed ‖ commitments), because participants should not trust the coordinator's randomizer. Our order: α is fixed in the PCZT before the co-signer commits nonces, one session at a time, and the co-signer commits only after holding the PCZT. Whether this is sufficient is open item 1.
- **Applying the signature:** `Signer::apply_ironwood_signature(&mut self, index: usize, signature: redpallas::Signature<SpendAuth>)` (L478), or `sign_ironwood_with<E, F>(self, f: F)` with `F: FnOnce(&Pczt, &mut orchard::pczt::Bundle, &mut u8)` (`low_level_signer/mod.rs` L28–53) and `Action::apply_signature` (orchard `pczt/signer.rs` L47). Both verify against `rk`.
- **FROST is already on our stack.** `zakura-reddsa` 2.2.0 has a `frost` feature bringing frost-rerandomized 3.0.0 and frost-core 3.0.0: FROST(Pallas, BLAKE2b-512), DKG and `EvenY` for the `ak` sign bit (`frost/redpallas.rs` L342–420). frost-tools uses 2.x; wire compatibility between 2.x and 3.x is unverified and irrelevant when both sides run our code.
- **Escrow FVK.** `FullViewingKey::from_bytes(bytes: &[u8; 96]) -> Option<Self>` (keys.rs L440–453) is public with no feature flag: `ak` from the group key, `nk` from a shared `sk`, `rivk` via the ZIP 2005 `use_qsk = true` derivation computed by us. `unstable-frost` only makes `SpendValidatingKey::to_bytes` (L193) and `from_bytes` (L205–221) public; they are `pub(crate)` otherwise. Zakura has no `from_sk_ak_incompatible_with_quantum_recoverability_and_will_be_removed`; the equivalent is `from_bytes`. zakura-orchard 2.2.0 has V3 note `rcm` only, no qsk or recovery support (note.rs L179–220).
- **Build bug in zakura-orchard 2.2.0:** `unstable-frost = []` (Cargo.toml L93) does not enable `dep:visibility`, so `cargo check` fails with E0433 at keys.rs L193/L205. Workaround: also enable `unstable-voting-circuits`, or skip `unstable-frost` since `FullViewingKey::from_bytes` covers the need. Worth a Zakura issue.
- **frost-tools cannot handle our branch ID.** `sighash_v6.rs` is byte-identical between zcash_primitives 0.29.0 and zakura-primitives 2.2.0, but upstream zcash_protocol 0.10 maps Nu7 to `0xffff_ffff` behind `cfg(zcash_unstable = "nu7")` (consensus.rs L737–757) and pczt 0.8 calls `BranchId::try_from` at parse (lib.rs L493), so zcash-sign rejects a 0x77190AD9 PCZT. Linking is also impossible: orchard 0.15.5 vs zakura-orchard 2.2.0, pczt 0.8.0-rc.1 vs rc4, pasta_curves 0.5 vs 2.2, ff/group 0.13 vs 0.14, halo2 0.3/0.5 vs 2.2.0, rand 0.8 vs 0.10, reddsa 0.5.1 vs 2.2.0.
- **Deferred signing works in principle.** `DeferredPcztBuilder` (zakura-primitives builder.rs L469; `new` L491; `add_ironwood_spend` L597) accepts v6 only; because the v6 txid and sighash exclude the anchor, co-signing can precede the anchor and witness, and A can re-anchor after co-signing without the co-signer. Not run end to end.
- **Share repair:** frost-core 3.0 `repair_share_part1` needs at least `min_signers` helpers (`keys/repairable.rs` L120). A 2-of-2 share cannot be repaired; 2-of-3 can.
- **Node behaviour on ths-pinned `ebda15e`:** measured same-input renewal rejected while the original was pooled, then mined after expiry eviction. Expiry 0 survived 1,000 empty blocks and mined. Sources: `crates/zakura-chain/src/transaction.rs` L679–699, `crates/zakurad/src/components/mempool/storage.rs` L951–980; [evidence](expiry-reissue.md#consensus-expiry-versus-mempool-policy). These are not main-only facts; live Zebra remains open.
- **Renewal feasibility:** retained proved pre-IO PCZT produced same-action/same-proof/α expiry variants with fresh FROST and dummy/binding signatures; both extracted/validated on a synthetic tree, **not mined**. Ordinary-signature renewal did mine. Complete effects include fields outside the action vector; retain dummy signing material, never reuse FROST nonces ([source lines and results](expiry-reissue.md#expiry-renewal-is-not-re-anchoring)).
- **Proof of concept: [`poc/escrowcheck`](../../poc/escrowcheck/README.md).** On the Zakura 2.2.0 pins it makes a 2-of-2 group key (trusted dealer), builds the escrow FVK and address, receives and decrypts a V3 note, builds a PCZT at branch 0x77190AD9, has the co-signer read sighash, α and nullifiers from the PCZT bytes alone, produces a rerandomized FROST aggregate, and passes `apply_ironwood_signature`, proving and `TransactionExtractor::extract` (which batch-validates the proof and all signatures). Output: a v6 transaction of 9,166 bytes, txid equal to the sighash. Reproduced in two orbs on 25 October 2026 (`cargo run --release --locked`, about 2.5 min to build). It uses a one-leaf local tree and was not mined on `ths`.
- **Estimates [unverified]:** the signing port is about 80–120 lines because zakura-pczt supplies the sighash and signature application (the PoC is 166 non-blank lines for keygen, FVK, PCZT, signing, proving and extraction). An escrow core with DKG, durable nonce and lock storage, policy checks and framing is about 400–700 lines of Rust, before BitChat transport and Swift bindings. The earlier 300–500-line frost-tools port estimate is superseded.

### Topology comparison (R1)

| # | Topology | Stops A double-spending B? | Breaks when | Who strands whom | Recovery |
|---|---|---|---|---|---|
| 0 | Pending + gateway mempool receipt | No; loss bounded | A races through another partition or node | A strands B (refuses re-issue) | Seed restore, which looks like a double-spend |
| 1 | Bilateral 2-of-2 A+B | Yes against A alone | **B's share or lock state** leaks or is rolled back; A's share leaking is harmless | Both ways: A blocks B's re-issue; B freezes A's escrow | None in 2-of-2 |
| 2 | A + mesh co-signer C, atomic nullifier lock | Yes if C honest and durable | A+C collude; C's lock DB restored from backup; nonce reuse. C equivocation is provable (two group-Y signatures for one nf) | C strands A's escrow; A strands B's re-issue | None in 2-of-2; a restored C must fence until the chain passes its expiry cap |
| 3a | 2-of-3 {A, C, cold share held by A} | **No** (A signs alone) | Always | None | Gives no protection |
| 3b | 2-of-3 {A, C, B} | Only for the payee holding a share | A + any share-holding payee bypass C; B+C can steal | A depends on B if C is lost | Repairable |
| 3c | 2-of-3 {A, C hot, R cold, fenced} | Yes, up to the fence | A+C or A+R collude; C+R can steal | Needs both C and R lost | R sweeps to A after the fence, or A+R repair C's share |
| 3d | A + C, C's share also tlock-encrypted to A | Yes until time T | Beacon or puzzle assumption fails; after T, A signs alone | None after T | Automatic at T; needs rotation each epoch |
| 4 | Payee gossip of pending nullifiers | No; detects inside a partition after propagation | Cross-partition, simultaneous, direct-to-gateway races | — | — |
| 5a | DAPS nonce bound to nf | Deterrence only; breaks honest re-issue | A drains first | — | — |
| 5b | VTS-emulated refund (PayMo/UAS) | Like 3d, no beacon | Timing calibration; no Pallas implementation | None after T | Timed |
| 5c | FastPay quorum 2f+1 of 3f+1 | Yes if ≤ f misbehave | > f collude | Equivocating payer locks the note | Repairable |

### Card-network offline acceptance: what transfers

Card networks do not prevent offline losses; they bound and assign them. Offline EMV approval proves the card is genuine, never the balance ([US Payments Forum 2016](https://www.emv-connection.com/downloads/2016/04/Merchant-Processing-during-Communication-Disruption-FINAL-April-2016.pdf)). Source note: [card_network_offline_acceptance.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/card_network_offline_acceptance.md). Concrete limits and formulas: [pending-tier.md §5](pending-tier.md).

| Card control | Source | Here |
|---|---|---|
| Floor limit: above it, go online | [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf) | Pending only below a fleet amount [decided] |
| Consecutive-offline counters (tags 9F14, 9F23) enforced in the card's secure element | [OpenSCDP EMV](https://www.openscdp.org/scripts/tutorial/emv/terminalriskmanagement.html) | Partly: payee counts per payer; the payer's `seq` is not enforced by hardware, so no cap is based on it |
| Random online processing below the floor | Visa Core Rules | Soft threshold: past ⌈C/2⌉ outstanding, wait for the gateway's `submitted` if reachable |
| Pre-departure hot list | [American Banker / GuestLogix](https://www.americanbanker.com/payments/news/guestlogix-rolls-out-travel-industry-hot-list-to-block-fraudulent-cards) | Nullifier list in chain bundles and shared pending nullifiers, never payer keys |
| Named loss owner | American Banker; [Adyen-based guide](https://support.ventrata.com/en/articles/13384674-how-to-activate-offline-payments-for-adyen-terminals) | The payee accepting at pending absorbs a failure |
| Sync deadline (24 h; 4 days transit) | Visa Rules 5.7.3.3 | Fleet expiry delta and the stale-view limit |
| Issuer collects a failed charge as debt | — | Partly: the PaymentStatement is a debt claim under the fleet agreement; ZEC itself is bearer |

### Prior art: what transfers and what doesn't

Source notes: [offline_payment_prior_art.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/offline_payment_prior_art.md), [decentralized_double_spend.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/decentralized_double_spend.md); R3 sources in [pending-tier.md §6](pending-tier.md).

| Approach | Source | Transfers? |
|---|---|---|
| Chaum ecash: a double-spend reveals identity | Chaum, Fiat, Naor, CRYPTO '88; Compact E-Cash, EUROCRYPT 2005 | Needs an issuer; Zcash conflicts reveal no identity. The PaymentStatement supplies identity by agreement instead |
| Consensusless quorums: 2f+1 of 3f+1 | FastPay, [arXiv 2003.11506](https://arxiv.org/abs/2003.11506); Guerraoui et al., [arXiv 1906.05574](https://arxiv.org/abs/1906.05574) | Logos phase; needs ≥ 4 signers |
| Listening period before accepting a fast payment | Karame, Androulaki, Čapkun, CCS 2012; Satoshi's snack-machine thread | Yes: 5 s wait when another payee is connected |
| Collateral/statekeepers, penalty deposits | Snappy, NDSS 2020; Brick, FC 2021; CCS 2015 | No: need scripts or timelocks |
| Timed release of a co-signer share | PayMo ([ePrint 2020/1441](https://eprint.iacr.org/2020/1441)); VTS, CCS 2020; tlock ([ePrint 2023/189](https://eprint.iacr.org/2023/189)); RSW 1996 | Candidate for v1.2 recovery (3d) |
| Trusted-hardware wallets | BIS Polaris 2023; IBM ePrint 2024/1746 | No Zcash spend-once applet; seed backup defeats it |
| Store-and-forward relays (txTenna, Meshtastic) | [txTenna](https://github.com/remyers/txTenna) | Transport only |
| Custodial ecash over mesh (Cashu, Fedimint) | [Cashu NUT-11](https://cashubtc.github.io/nuts/11/) | Lesson only: prevention needs a lock made while online, which is the v1.1 escrow |
| Bearer URIs (ZIP 324) | [ZIP 324](https://zips.z.cash/zip-0324) | No: Draft, Sapling-only |
| Lightning-style channels | — | No shielded scripts or timelocks; bilateral escrow gives cumulative states without a refund path |

## Open

1. [unverified] **Gates v1.1.** Review malicious payer/coordinator and chosen-α behavior against Re-Randomized FROST ([ePrint 2024/436](https://eprint.iacr.org/2024/436)). Use ZF's current seed/commitment-derived construction, not the deprecated explicit-randomizer API; fitting it to the payer-prepared PCZT remains unresolved. Do not copy draft DKG `sk=ext`; key derivation needs separate review ([prior art](../prior-art.md)).
2. [unverified] Lock and nonce durability: kill the co-signer between the lock write and the share release and confirm there is never a second release; a co-signer restored from an old backup refuses to sign until the fence.
3. [unverified] The PoC transaction mines on `ths` once the escrow address is funded from a dev account and spent with a real witness.
4. [unverified] `Signer::new` succeeds on a PCZT redacted of value, recipient, rseed, fvk, witness and rcv.
5. [awaiting owner / unimplemented] Complete-effects lock, immutable epoch fence and encrypted retained pre-IO state. Same-action/same-proof expiry renewal is feasible, not a byte-identical transaction; real-funded FROST mining and durable storage remain gates ([expiry-reissue.md](expiry-reissue.md)).
6. [unverified] ZIP 2005 `use_qsk` rivk derivation against test vectors; whether the rc7 wallet DB can import a FROST-derived UFVK and produce a PCZT, else use the direct Builder.
7. [unverified] Live Zebra mempool/expiry behavior with the same bytes; pinned Zakura results are in Findings.
8. [unverified] tlock Rust support, drand trust and RSW speed-up bounds (v1.2).
9. Tests that gate v1.1: a second spend of a locked nullifier to a different payee is refused; a **same-action/same-proof expiry renewal with fresh signatures is mined from real-funded FROST notes** and old bytes are rejected after expiry; a crash between lock and release never produces conflicting releases or nonce reuse; a restored co-signer refuses until the immutable fence and reconciliation; B rejects `rk` not derived from Y. Family, reorg and branch gates are in [expiry-reissue.md](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research).
10. Pending-tier open items are in [pending-tier.md §7](pending-tier.md).
11. **Required before v1.1 safety claims:** write C's failure model for backup rollback, cloning, concurrent sessions/retries, ambiguous crashes, nonce persistence, replacement authorization, device loss and every alternate recovery/spend path. Honest, non-rollbackable C is a condition, not a consequence of 2-of-2 FROST ([prior art](../prior-art.md)).

### Corrections to archive text and to the first version of this file

- Report and research note: "intent plus chain fraud proof" and "attributable evidence" → conflict evidence; with a PaymentStatement, a debt claim under the fleet agreement.
- Research note §5: "k × floor limit bounds total liability across partitions" → false; local control only.
- Card note §3.4 and report: hot list of payer basecamp keys → nullifiers only, never automatic.
- Research note §10.4 and report: `lock_time` refund path open → resolved, not enforced; bonds dropped.
- Research note §9 staging (signed intents gossiped mesh-wide) → details to the payee only.
- This file, 25 October morning: "bilateral escrow... unless the payer's share leaks" → it breaks when the **payee's** (co-signer's) share or lock state leaks or is rolled back; the payer's share leaking is harmless for double-spend.
- This file and privacy P6: "FROST participants hold the escrow FVK" → not required; the co-signer needs no viewing key.
- This file: "port frost-tools' zcash-sign, about 300–500 lines" → frost-tools cannot parse NU7 PCZTs; the Zakura-native path is proven offline in `poc/escrowcheck`; estimates above.
- This file: keys.rs L193 is `to_bytes`, L205–221 `from_bytes`, both `pub(crate)` unless `unstable-frost`; `FullViewingKey::from_bytes` needs no feature; there is no `from_sk_ak_*` in Zakura.
- This file: "Next phase: bilateral 2-of-2 FROST escrow" → next phase is the co-signer component with a mesh co-signer first; bilateral is a configuration of it.
