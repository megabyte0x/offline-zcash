# Transaction expiry and re-issue (problem #3)

Status: research complete, **awaiting owner decision**. The proposals below are not ratified or implemented. Problems #1 (pending tier / nullifier-lock co-signer) and #2 (chain-state trust) remain settled; their expiry-related assumptions are corrected here, not silently promoted into new decisions. Next: problem #4, [gateway protocol](gateway.md).

## Decided

- Existing design: A builds, proves and signs offline; D submits raw bytes without spending or viewing keys. Branch ID, expiry and fee come from fleet configuration. Never repay a stuck payment from unrelated notes ([offline-builder.md](offline-builder.md)).
- No expiry profile, repair-message schema or payment-family state machine has been approved. The owner must accept or reject the proposals individually; the review guide is linked from [open-work.md](../open-work.md).

## Findings

Research cutoff: **25 October 2026**. All execution below used disposable funds on `ths` v0.4.1 / Zakura v1.6.0 Regtest, not mainnet. Source revisions and line citations are below. Evidence: final itemized report, boundary/reorg/PCZT logs, successful cases and sample logs. These are historical research results, not a shipped application or a fresh integration run.

### Source revisions

| Label | Revision | Paths below are relative to this repository |
|---|---|---|
| C | [Zakura Common 2.2.0, `b6cf4ce`](https://github.com/zakura-core/common/tree/b6cf4cefe38fdccc6d2b0dc868f7d6ef01683413) | `crates/zcash_primitives/`, `crates/orchard/` |
| P | [wallet-libraries `f94186c`](https://github.com/zakura-core/wallet-libraries/tree/f94186c7153ba14af6f9ce21898942c2c788d611) | `librustzcash/pczt/`, published `zakura-pczt` 0.1.0-rc4 |
| Z | [Zakura v1.6.0, `ebda15e`](https://github.com/zakura-core/zakura/tree/ebda15e23ed0189787e21b4625c6e4f62d10932f) | `crates/zakura-*`, `crates/zakurad/` |
| B | [Zebra `1e03ee7`](https://github.com/ZcashFoundation/zebra/tree/1e03ee753fbc72d70882cdfd3c8bdd58b80f0f97) | `zebra-*`, `zebrad/` |
| ZIP | [ZIPs `2bdf394`](https://github.com/zcash/zips/tree/2bdf39494320e5e04057ac93b1794031ebfb00b5) | `zips/` |

### Consensus expiry versus mempool policy

Every measured transaction was v6, Ironwood-only, two actions, 9,166 bytes, fee 10,000 zats.

| Case | Measured result on pinned Zakura |
|---|---|
| Expiry 0 | Accepted at tip 109; stayed in mempool through 1,000 deliberately empty blocks; mined at 1,110. Zero means **never expires** |
| Expiry 499,999,999 | Accepted at tip 1,113, mined at 1,114. Maximum is **inclusive** |
| Expiry 500,000,000 | Rejected, RPC `-25`. Error says “less than” 499,999,999, although equality works |
| Expiry 107 at tip 106 | Accepted and mined at **107**, its expiry height |
| Expiry 106 or 105 at tip 106 | Rejected: candidate block 107 exceeds expiry |
| Resident expiry 1,118 | Present at tip 1,117, evicted at tip 1,118; old bytes then rejected |
| Same-input, later-expiry replacement | Rejected while original is resident (input conflict); accepted after expiry eviction, mined at 1,119 |

Expiry E permits inclusion in block E. At tip E the next block is too late, so eviction at `tip >= E` is consistent with inclusive block validity. This release has **no three-block expiring-soon refusal** in the tested path: tip+1 worked. ZIP 218 names a three-block threshold; that is not a measured Zakura admission rule or a merchant safety bound.

Sources: Z `crates/zakura-chain/src/transaction.rs` L679–699 (zero), `crates/zakura-consensus/src/transaction/check.rs` L592–667 (bounds), `crates/zakurad/src/components/mempool/storage.rs` L951–980 (eviction); B `zebra-chain/src/transaction.rs` L164–179, `zebra-consensus/src/transaction/check.rs` L547–622, `zebrad/src/components/mempool/storage.rs` L883–922 and `downloads.rs` L430–486. [ZIP 203](https://zips.z.cash/zip-0203) L34–77; [ZIP 218](https://zips.z.cash/zip-0218) L645–690.

No relative-delta ceiling or accepted-transaction age TTL was found in these inspected admission/storage paths. Zebra's one-hour `eviction_memory_time` is rejection-cache retention, **not resident transaction TTL** (B `zebrad/src/components/mempool/config.rs` L21–33, L52–65). Long expiry promises neither retention nor mining. Live Zebra, real multi-day wall-clock residence, restarts, capacity eviction, congestion and other miners remain untested.

### Old roots, reorgs and re-anchoring

- An anchor at height **1,113** still mined at **11,488**: **10,375 blocks old**. This tests block age, not three elapsed days. No consensus recent-anchor window was found (Z `crates/zakura-state/src/service/check/anchors.rs` L124–155; B `zebra-state/src/service/check/anchors.rs` L120–160).
- A two-block `ths reorg` removed a root at 11,490 while preserving the input note from 11,489. Submission failed with `unknown Ironwood anchor`. New witnesses/anchor/proof with **all original ordinary signatures** were accepted and mined at 11,492 with the **same txid**. Retained-FROST-signature re-anchoring was **not executed**; the synthetic-tree FROST expiry-renewal experiment below is separate. The root must cease to be a valid historical root on the selected chain; merely removing one block that contained a repeated root is insufficient.
- Compatible v6 re-anchoring changes authorizing data and auth digest, not txid or applicable signatures. It needs a new Merkle path for **every real spent note to one common anchor**, private proving state and a new proof. If the input note itself was orphaned, no witness repairs it until the funding exists again on the selected chain.
- An MMR proof that an anchor exists is not a spending witness. Public commitment ranges/subtrees/checkpoints could update A's witnesses over the mesh without internet on A. Position-specific queries to D reveal which commitments and originating transactions interest A.
- Zebra's 1,000-block rollback window is local node policy, not consensus finality (B `zebra-chain/src/parameters/constants.rs` L20–33).

Sources: C `crates/orchard/src/bundle/commitments.rs` L98–107; [ZIP 229](https://zips.z.cash/zip-0229) L382–395; [ZIP 374](https://zips.z.cash/zip-0374) L1384–1422; [ZIP 258](https://zips.z.cash/zip-0258) L172–207. Live Zebra re-anchoring and privacy-preserving mesh witness transport remain open.

### Expiry renewal is not re-anchoring

- Expiry-only renewal retained **byte-identical actions and proof**, but changed expiry, txid, sighash and signatures. It is not a byte-identical transaction. Ordinary signatures were mined in the replacement test above.
- A retained, serialized/reloaded **proved pre-IO-finalization PCZT** produced expiry-50 and expiry-60 FROST variants: same actions/proof/α, fresh FROST nonces and fresh dummy/binding signatures; both extracted and validated. This used a synthetic funding tree and **was not mined**. Real-funded FROST renewal remains a gate.
- Fresh construction randomness changed dummy nullifiers; the real input nullifier remained. Retained construction state can keep all actions identical. Reusing one construction seed for unrelated payments accidentally reused a dummy nullifier in the experiment; seeds must be unique per family. Structured retained state is preferable to relying solely on RNG replay.
- On the orb's two-vCPU Xeon 2.60 GHz: proving-key setup median 401 ms (9 samples, 385–656); two-action Ironwood proof **729 ms** (9, 684–783); direct re-sign **1.254 ms** (19, 1.223–1.639). Excludes scanning, BLE, FROST interaction and phones.

Source details:

| Fact | File and lines |
|---|---|
| Expiry in header digest | C `crates/zcash_primitives/src/transaction/txid.rs` L225–240 |
| Proof inputs exclude expiry/signatures | C `crates/orchard/src/circuit.rs` L1460–1513, L1556–1583 |
| Cloneable retained proved unauthorized bundle | C `crates/orchard/src/builder.rs` L1561–1566, L1710–1755; construction RNG at L1150–1195 |
| Expiry private; combining different expiries fails; no public updater setter | P `librustzcash/pczt/src/common.rs` L125–126, L230–237; `roles/updater/mod.rs` L300–307 |
| Fresh header reconstruction components | C `crates/zcash_primitives/src/transaction/builder.rs` L784–794; P `librustzcash/pczt/src/roles/creator/mod.rs` L288 |
| IO finalization consumes dummy signing key | C `crates/orchard/src/pczt/io_finalizer.rs` L60–68 |
| Anchor setter rejects existing proof/conflicting anchor, not automatic clearing | P `librustzcash/pczt/src/roles/updater/mod.rs` L109–127; explicit clearing in `roles/redactor/orchard.rs` L66–81 |
| Flags and value balance committed separately from actions | C `crates/orchard/src/bundle/commitments.rs` L147–165 |

Thus “identical actions” alone does not lock complete effects. A rolling relative expiry cap alone is not an absolute recovery fence. Neither finding establishes a production persistence or signing protocol.

### Lifetime, freshness, privacy and upgrades

- Four clocks must stay separate: **bundle freshness, transaction validity, C's reservation lifetime, confirmation policy** ([prior-art.md](../prior-art.md)). The NU7 default of 120 after **target height**, normally tip+1, preserves about 50 minutes, not offline carriage. Common 2.2.0 defaults to 40 (C `crates/zcash_primitives/src/transaction/builder.rs` L54).
- Thirty minutes is about **72 NU7 target blocks**; 20 blocks is 8m20s. H+20 against an allowed thirty-minute-old H is not a current-tip margin. The 30-minute limit has no sourced Zcash safety basis; it would still stop new acceptance during a multi-day header outage even with long-lived accepted payments.
- The 100-block recent-anchor window is a provisional **new-payment acceptance policy**, not consensus. Applying it retroactively to accepted payments would obstruct old-root renewal. Ten confirmations is a provisional **low-value demo policy**, not universal finality: ZIP 315 recommends ten for untrusted output spendability; Kraken listed fifty ([prior-art.md](../prior-art.md)).
- A clustered, pre-NU7 mainnet sample of **1,500 mined non-coinbase transactions** had only **4 with ≥10,368 blocks remaining at inclusion** (3 of 1,240 v6). 181 had expiry zero (2 v6). A raw-byte cross-check found 63,920 blocks remaining in one long-lived transaction. This is not construction delta, a representative population estimate or evidence about failed/unmined transactions. It supports a fingerprinting concern; uniform fleet policy does not blend into ordinary wallets. Sample evidence; [ZIP 315](https://zips.z.cash/zip-0315) L502–507, L532–535; [ZIP 317](https://zips.z.cash/zip-0317) L224–226.
- Shutdown examples are not uplink guarantees: Internet Society Pulse records [Bangladesh, 5 days 12 hours](https://pulse.internetsociety.org/en/shutdowns/internet-suspended-in-bangladesh-18-july-2024) and [Sudan, 100 days 9 hours](https://pulse.internetsociety.org/en/shutdowns/internet-suspended-in-sudan-2-february-2024/). Neither proves every person's alternatives were unavailable throughout.
- **No scheduled NU8-or-later activation height was found** at the research cutoff: B `zebra-chain/src/parameters/network_upgrade.rs` L62–72, L104–120 ends at NU7; `constants.rs` L98–106 sets NU7 3,543,405. This is dated evidence, not a promise that no upgrade will occur. Branch mismatch at mining is rejected (B `zebra-consensus/src/transaction/check.rs` L888–907). `with_expiry_height` only assigns a value; it enforces no activation cap (C `builder.rs` L1032–1043). Future proof compatibility is unverified.

### Family safety and unresolved expiry

Same real notes exclude competing versions on **one valid chain**, not across orphaned histories. Adjacent overlap is not transitive: `{a} → {a,b} → {b}` allows first and last to coexist. Any relaxed family rule needs a nonempty nullifier intersection across **all versions**, not just neighbors; fixed canonical effects are stronger.

Expiry proves no future inclusion after E, **not non-inclusion before E**. Offline evidence cannot show a nullifier was never spent ([ZIP 221](https://zips.z.cash/zip-0221) L660–663). Trusted unspent reconciliation also does not cancel an accepted debt. In 2-of-2 escrow B still cannot renew without A: it preserves a debt claim, not unilateral recovery.

## Proposed — awaiting owner decision

Numbering follows the owner's twelve-decision walkthrough, not the research report's grouping. Every item is **proposed**, including the recommended default. Nothing below changes wire formats or claims implementation.

1. **Fleet expiry profiles.** Configurable delta = 3× declared maximum payment-to-uplink interval: courier ≤1 day → **10,368 blocks (~3 days)**; shutdown ≤7 days → **72,576 (~21 days)**. Proposed default: shutdown, only with that uplink promise. One profile per fleet epoch, not payer-selected per payment. Neither covers arbitrary shutdowns.
2. **Payment family.** All versions of one invoice are one obligation, counted once; inclusion of any version settles it. Retain all bytes/statements/evidence. Require byte-identical actions or a nonempty common nullifier across every version (plus fixed payee/invoice/amount/tier); ordinary renewal preserves full canonical effects. Dedupe variants by `(txid, auth_digest)`; no self-superseding txid cycle for a same-txid authorization refresh.
3. **Complete lock and immutable fence.** C locks the full effecting payload: action count/order/fields/ciphertexts, flags, value balance, pool and format. An absolute key-epoch expiry fence is fixed at lock creation and never rolled forward by renewals; no expiry-0. No unlock on expiry alone; recovery also needs reconciliation and retirement/rotation of old authority.
4. **Repair initiation.** A proactively re-issues once its verified tip reaches expiry; B may request repair with a private authenticated `ozc1:rq:` message. Proposed fields: version/fleet/parties, old statement hash/txid/auth digest, request ID, reason (expiry/anchor/branch), verified height/hash and evidence reference. Exact layout remains open. Dedupe requests; process known inclusion/conflict first. Replacement uses `ozc1:pay:` with fresh seq/statement; expiry/branch replacement sets bit0 and `supersedes_txid`. B fsyncs before receipt; update the same sharer's nullifier entry without increasing exposure.
5. **Expired-unresolved.** No inclusion known when verified selected-chain tip reaches E → `expired-unresolved`; debt, counters, bytes and note reservations stay. Repair does not reset exposure. Any old family inclusion still settles it; a non-family spend is conflict. Orphaned inclusion reopens exposure. One source's expiry claim cannot clear debt or free notes.
6. **Separate freshness from settlement.** Keep 30-minute new-acceptance freshness **provisionally**. Replace flat H+20 with **remaining life ≥ view lag + uplink delay + confirmations + margin**, all in conservative block units. A proposed target-time budget is \(E-H_B \ge \lceil(S+U)/25\text{ s}\rceil+Q+M\): lag S, uplink U, confirmation allowance Q, margin M. It is not a hard wall-clock guarantee; bundle arrival does not prove latest-chain freshness. Existing-family repair may verify a historical root or re-anchor rather than reapply the new-payment 100-block window. A valid repair/evidence response bypasses the blocked-payer/already-pending-invoice refusal for that obligation.
7. **Upgrade cap and configuration.** Expiry ≤ next known activation − 1. At every signing/renewal A and C check authenticated fleet configuration: network, active/next branch, activation, supported rules, version/freshness. Proposed cap: \(E=\min(h_t+\Delta,A-1,F,499{,}999{,}999)\), omitting absent caps; reject zero or insufficient remaining lifetime. D may carry configuration, not author it. Expiry/branch changes need fresh authorization; unspecified future proof reuse is not assumed.
8. **Hostage case.** If A will not renew, B keeps the debt claim and off-chain enforcement; 2-of-2 escrow does not fix this. Longer expiry reduces, not removes, the risk. Pre-signed ladders would only give finite extensions bounded by branches/fences.
9. **Reject zero.** Enforce at builder, B and C despite consensus support for never-expiring transactions.
10. **Encrypted retained state.** Persist a proved pre-signing/pre-IO-finalization snapshot including dummy keys and construction randomness. Reconstruct headers rather than mutate signed PCZTs; expiry-only renewal reuses actions/proof but refreshes all needed signatures with fresh randomness/FROST nonces. Lost-state and library-migration behavior need a policy.
11. **Re-anchor on A.** Obtain new paths and prove on A, preferably from generic mesh tree updates, not note-position queries to D. Compatible re-anchoring itself retains txid and signatures and needs no new FROST session; **if signing again, always use fresh FROST nonces**. Do not give private proving material to D.
12. **Application durability.** Persist/re-queue beyond BitChat's **24-hour / 8-attempt** sender outbox; transport ACK is not durable acceptance. Maintain family accounting and `(txid, auth_digest)` variant deduplication independently of BitChat.

**Optional alternative #13: ZIP 318-style bucketed expiry.** At NU7, interval E = 103,680 and expiry = `(h − h % E) + 2E`, approximately **30–60 days** from scheduled broadcast. Released migration code exists, but this implies long validity and old anchors, not the same merchant risk model. The discussion leaned against it for v1; **not selected**. See [prior art](../prior-art.md).

## Open

### Eight research open items

1. Owner approval of uplink envelope/profile and merchant willingness to hold that debt.
2. Whether to relax thirty-minute new acceptance; recompute detection windows and loss caps if so.
3. Absolute signer-epoch fence, signing cessation/rotation and recovery reconciliation.
4. Repair wire schema, reasons/states and same-txid refresh statement semantics.
5. Generic tree updates/checkpoints, witness persistence, mesh bandwidth and privacy.
6. Sensitive pre-IO persistence format, library upgrades and lost-state behavior.
7. Future upgrade-config authority, freshness and offline distribution.
8. FROST chosen-α/coordinator security and escrow-key derivation review; research did not close them. Use ZF's current seed-and-commitment rerandomized construction rather than copying the deprecated explicit-randomizer path; do not copy draft DKG `sk=ext` ([prior art](../prior-art.md)). Compatibility with this PCZT workflow remains a gate.

**Additional required v1.1 item: C's written failure model.** State the invariant that no conflicting still-valid authorization can be released for a locked nullifier, covering backup rollback, database cloning, concurrent signing/retries, ambiguous crashes, nonce consumption, replacement authorization, device loss and every alternate recovery/spend-key path. Honest, non-rollbackable C is an assumption to enforce/test, not a property supplied by FROST. C unavailable also locks funds.

### Ten gating tests (not completed by the research)

1. Live Zebra with the same bytes: zero/max/above-max/near-expiry/expired/resident-expiry/old-anchor cases.
2. Fund FROST escrow on ths; mine real-witness same-action/same-proof expiry renewal with fresh signatures and durable C locks.
3. Configured upgrade crossing: old-branch rejection, replacement acceptance, cap at A−1; separately prove pre-activation anchor history.
4. Crash A between reserve/sign/send; C between lock/nonce write/share release; B between save/counter/share/receipt. No release, double count or nonce reuse.
5. Three-generation families including `{a} → {a,b} → {b}`, old-version inclusion, duplicates, out-of-order replacements and lost receipts.
6. Orphan inclusion and anchor separately; also orphan an input note. Restore accounting, never fabricate witnesses. Mine a same-txid re-anchor retaining the original FROST signatures; only the ordinary-signature case was executed.
7. Gateway lag versus incompatible forks, withheld inclusion and one-source expiry. No false release/non-payment conclusion. Ancestor/descendant tips are lag; competing branches require validated chain/work selection, not claimed-height voting.
8. Retained old-root renewal beyond the provisional 100-block new-payment window; tampered witness/root updates rejected.
9. Lifetime budget at maximum view lag/uplink delay, including activation/fence truncation.
10. Mac/iPhone proving time and memory, witness-update bytes, repair delivery and restart/retry beyond BitChat's 24-hour TTL.

No controlled auto-mining test was run; [AGENTS.md issue #7](../../AGENTS.md) stays unverified. Explicit `generate` mined the test transactions. The failed 120-second rolled-back-block replay had no diagnosed upstream cause; coinbase-only scratch blocks replaced it. The maximum-expiry error wording defect is logged separately; none of these establishes a new consensus restriction.
