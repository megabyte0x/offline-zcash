# Transaction expiry and re-issue (problem #3)

**v0.7.7:** configured finite expiry (default delta 72,576), persisted/retried
unchanged bytes, and unspent-reservation unlock only after local scan passes
inclusive expiry—not on a gateway expiry report. Unlock is not debt cancellation.

**Unbuilt design:** payment families, automatic renewal, immutable signer fences
and `expired-unresolved` accounting. [Owner decisions, 26 October](#proposals-and-owner-decisions-26-october)
specify the minimal build; references to C apply to every co-signer of the
[group](double-spend.md). Problems #1 (re-decided 26 October) and #2 (chain-state
trust) remain settled; this corrects their expiry assumptions, not their decisions.

## Decided

- A builds, proves and signs offline; D submits bytes without spending/viewing keys. Fleet config supplies branch, expiry and fee. Never repay a stuck payment from unrelated notes ([offline-builder.md](offline-builder.md)).
- Shutdown profile, family accounting and A-initiated repair were accepted; repair layouts and same-txid refresh semantics remain open ([open work](../open-work.md)). Next: #4, [gateway protocol](gateway.md).

## Findings

Research cutoff: **25 October 2026**. All execution used disposable Regtest funds
on `ths` v0.4.1 / Zakura v1.6.0, not mainnet or the current app. Evidence: final
itemized report, boundary/reorg/PCZT logs, successful cases and sample logs.

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

Block E is valid; at tip E the next block is too late, hence eviction at
`tip >= E`. The tested path had **no three-block expiring-soon refusal**: tip+1
worked. ZIP 218's three-block threshold is neither a measured Zakura admission
rule nor a merchant safety bound.

Sources: Z `crates/zakura-chain/src/transaction.rs` L679–699 (zero), `crates/zakura-consensus/src/transaction/check.rs` L592–667 (bounds), `crates/zakurad/src/components/mempool/storage.rs` L951–980 (eviction); B `zebra-chain/src/transaction.rs` L164–179, `zebra-consensus/src/transaction/check.rs` L547–622, `zebrad/src/components/mempool/storage.rs` L883–922 and `downloads.rs` L430–486. [ZIP 203](https://zips.z.cash/zip-0203) L34–77; [ZIP 218](https://zips.z.cash/zip-0218) L645–690.

No relative-delta ceiling or resident-age TTL was found in the inspected paths.
Zebra's one-hour `eviction_memory_time` retains rejections, **not resident txs**
(B `zebrad/src/components/mempool/config.rs` L21–33, L52–65). Long expiry promises
neither retention nor mining. Live Zebra, multi-day wall-clock residence,
restarts, capacity eviction, congestion and other miners remain untested.

### Old roots, reorgs and re-anchoring

- An anchor at height **1,113** still mined at **11,488**: **10,375 blocks old**. This tests block age, not three elapsed days. No consensus recent-anchor window was found (Z `crates/zakura-state/src/service/check/anchors.rs` L124–155; B `zebra-state/src/service/check/anchors.rs` L120–160).
- A two-block `ths reorg` removed the root at 11,490 but preserved the input note from 11,489. Submission failed: `unknown Ironwood anchor`. New witnesses/anchor/proof with **all original ordinary signatures** mined at 11,492, **same txid**. Retained-FROST-signature re-anchoring was **not executed**; synthetic-tree expiry renewal below is separate. A removed root must no longer be historical on the selected chain; removing one occurrence of a repeated root is insufficient.
- Compatible v6 re-anchoring changes auth data/digest, not txid or applicable signatures. It needs private proving state, a new proof and paths for **every real spent note to one common anchor**. Orphaned funding must first reappear on the selected chain; witnesses cannot repair it.
- MMR anchor proofs are not spending witnesses. Public commitment ranges/subtrees/checkpoints could update A's witnesses offline over mesh; position-specific D queries reveal commitments/originating txs of interest.
- Zebra's 1,000-block rollback window is local node policy, not consensus finality (B `zebra-chain/src/parameters/constants.rs` L20–33).

Sources: C `crates/orchard/src/bundle/commitments.rs` L98–107; [ZIP 229](https://zips.z.cash/zip-0229) L382–395; [ZIP 374](https://zips.z.cash/zip-0374) L1384–1422; [ZIP 258](https://zips.z.cash/zip-0258) L172–207. Live Zebra re-anchoring and privacy-preserving mesh witness transport remain open.

### Expiry renewal is not re-anchoring

- Expiry renewal kept **byte-identical actions/proof**, not the whole tx: expiry, txid, sighash and signatures changed. Ordinary-signature renewal mined above.
- A serialized/reloaded **proved pre-IO-finalization PCZT** yielded expiry-50/60 FROST variants: same actions/proof/α, fresh FROST nonces and dummy/binding signatures. Both extracted/validated on a synthetic funding tree, **not mined**. Real-funded FROST renewal remains a gate.
- Fresh randomness changed dummy nullifiers, not the real input nullifier; retained state can preserve all actions. Reusing a construction seed across unrelated payments accidentally reused a dummy nullifier. Use unique family seeds, preferably structured retained state rather than RNG replay alone.
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

Identical actions do not fix complete effects; a rolling relative cap is not an
absolute recovery fence. Neither establishes a production persistence/signing protocol.

### Lifetime, freshness, privacy and upgrades

- Separate **bundle freshness, tx validity, C's reservation lifetime and confirmation policy** ([prior-art.md](../prior-art.md)). NU7's default 120 after **target height** (normally tip+1) is ~50 minutes, not offline carriage. Common 2.2.0 defaults to 40 (C `crates/zcash_primitives/src/transaction/builder.rs` L54).
- Thirty minutes ≈ **72 NU7 target blocks**; 20 blocks = 8m20s. H+20 against thirty-minute-old H gives no current-tip margin. The unsourced 30-minute limit stops new acceptance during multi-day header outages despite long-lived accepted payments.
- The provisional 100-block window governs **new acceptance**, not consensus or existing-family renewal. Ten confirmations is provisional **low-value demo policy**, not finality: ZIP 315 recommends ten for untrusted output spendability; Kraken listed fifty ([prior-art.md](../prior-art.md)).
- A clustered, pre-NU7 mainnet sample of **1,500 mined non-coinbase transactions** had only **4 with ≥10,368 blocks remaining at inclusion** (3 of 1,240 v6). 181 had expiry zero (2 v6). A raw-byte cross-check found 63,920 blocks remaining in one long-lived transaction. This is not construction delta, a representative population estimate or evidence about failed/unmined transactions. It supports a fingerprinting concern; uniform fleet policy does not blend into ordinary wallets. Sample evidence; [ZIP 315](https://zips.z.cash/zip-0315) L502–507, L532–535; [ZIP 317](https://zips.z.cash/zip-0317) L224–226.
- Shutdown examples are not uplink guarantees: Internet Society Pulse records [Bangladesh, 5 days 12 hours](https://pulse.internetsociety.org/en/shutdowns/internet-suspended-in-bangladesh-18-july-2024) and [Sudan, 100 days 9 hours](https://pulse.internetsociety.org/en/shutdowns/internet-suspended-in-sudan-2-february-2024/). Neither proves every person's alternatives were unavailable throughout.
- **No scheduled NU8-or-later activation height was found** at the research cutoff: B `zebra-chain/src/parameters/network_upgrade.rs` L62–72, L104–120 ends at NU7; `constants.rs` L98–106 sets NU7 3,543,405. This is dated evidence, not a promise that no upgrade will occur. Branch mismatch at mining is rejected (B `zebra-consensus/src/transaction/check.rs` L888–907). `with_expiry_height` only assigns a value; it enforces no activation cap (C `builder.rs` L1032–1043). Future proof compatibility is unverified.

### Family safety and unresolved expiry

Same real notes exclude versions on **one valid chain**, not orphaned histories.
Adjacent overlap is insufficient: `{a} → {a,b} → {b}` lets first and last coexist.
A relaxed family rule needs a nonempty intersection across **all versions**;
fixed canonical effects are stronger.

Expiry proves no future inclusion after E, **not non-inclusion before E**.
Offline evidence cannot prove a nullifier was never spent ([ZIP 221](https://zips.z.cash/zip-0221) L660–663).
Even trusted unspent reconciliation cannot cancel debt. 2-of-2 escrow still
needs A to renew; B has a debt claim, not unilateral recovery.

## Proposals and owner decisions (26 October)

Numbering follows the owner's twelve-decision walkthrough, not the research grouping.

1. [Accepted: shutdown profile, 72,576 blocks] **Fleet expiry profiles.** Configurable delta = 3× declared maximum payment-to-uplink interval: courier ≤1 day → **10,368 blocks (~3 days)**; shutdown ≤7 days → **72,576 (~21 days)**. Proposed default: shutdown, only with that uplink promise. One profile per fleet epoch, not payer-selected per payment. Neither covers arbitrary shutdowns.
2. [Accepted] **Payment family.** All versions of one invoice are one obligation, counted once; inclusion of any version settles it. Retain all bytes/statements/evidence. Require byte-identical actions or a nonempty common nullifier across every version (plus fixed payee/invoice/amount/tier); ordinary renewal preserves full canonical effects. Dedupe variants by `(txid, auth_digest)`; no self-superseding txid cycle for a same-txid authorization refresh.
3. [Accepted; applies to each co-signer of the group] **Complete lock and immutable fence.** C locks the full effecting payload: action count/order/fields/ciphertexts, flags, value balance, pool and format. An absolute key-epoch expiry fence is fixed at lock creation and never rolled forward by renewals; no expiry-0. No unlock on expiry alone; recovery also needs reconciliation and retirement/rotation of old authority.
4. [Accepted: A's automatic re-issue only; B's `ozc1:rq:` request deferred] **Repair initiation.** A proactively re-issues once its verified tip reaches expiry; B may request repair with a private authenticated `ozc1:rq:` message. Proposed fields: version/fleet/parties, old statement hash/txid/auth digest, request ID, reason (expiry/anchor/branch), verified height/hash and evidence reference. Exact layout remains open. Dedupe requests; process known inclusion/conflict first. Replacement uses `ozc1:pay:` with fresh seq/statement; expiry/branch replacement sets bit0 and `supersedes_txid`. B fsyncs before receipt; update the same sharer's nullifier entry without increasing exposure.
5. [Accepted] **Expired-unresolved.** No inclusion known when verified selected-chain tip reaches E → `expired-unresolved`; debt, counters, bytes and note reservations stay. Repair does not reset exposure. Any old family inclusion still settles it; a non-family spend is conflict. Orphaned inclusion reopens exposure. One source's expiry claim cannot clear debt or free notes.
6. [Accepted] **Separate freshness from settlement.** Keep 30-minute new-acceptance freshness **provisionally**. Replace flat H+20 with **remaining life ≥ view lag + uplink delay + confirmations + margin**, all in conservative block units. A proposed target-time budget is \(E-H_B \ge \lceil(S+U)/25\text{ s}\rceil+Q+M\): lag S, uplink U, confirmation allowance Q, margin M. It is not a hard wall-clock guarantee; bundle arrival does not prove latest-chain freshness. Existing-family repair may verify a historical root or re-anchor rather than reapply the new-payment 100-block window. A valid repair/evidence response bypasses the blocked-payer/already-pending-invoice refusal for that obligation.
7. [Accepted; checked by A, B and every co-signer] **Upgrade cap and configuration.** Expiry ≤ next known activation − 1. At every signing/renewal A and C check authenticated fleet configuration: network, active/next branch, activation, supported rules, version/freshness. Proposed cap: \(E=\min(h_t+\Delta,A-1,F,499{,}999{,}999)\), omitting absent caps; reject zero or insufficient remaining lifetime. D may carry configuration, not author it. Expiry/branch changes need fresh authorization; unspecified future proof reuse is not assumed.
8. [Accepted] **Hostage case.** If A will not renew, B keeps the debt claim and off-chain enforcement; 2-of-2 escrow does not fix this. Longer expiry reduces, not removes, the risk. Pre-signed ladders would only give finite extensions bounded by branches/fences.
9. [Accepted] **Reject zero.** Enforce at builder, B and C despite consensus support for never-expiring transactions.
10. [Deferred: minimal renewal rebuilds and re-proves from the same notes] **Encrypted retained state.** Persist a proved pre-signing/pre-IO-finalization snapshot including dummy keys and construction randomness. Reconstruct headers rather than mutate signed PCZTs; expiry-only renewal reuses actions/proof but refreshes all needed signatures with fresh randomness/FROST nonces. Lost-state and library-migration behavior need a policy.
11. [Accepted] **Re-anchor on A.** Obtain new paths and prove on A, preferably from generic mesh tree updates, not note-position queries to D. Compatible re-anchoring itself retains txid and signatures and needs no new FROST session; **if signing again, always use fresh FROST nonces**. Do not give private proving material to D.
12. **[Approved 26 Oct] Application durability.** Persist/re-queue beyond BitChat's **24-hour / 8-attempt** sender outbox; transport ACK is not durable acceptance. Maintain family accounting and `(txid, auth_digest)` variant deduplication independently of BitChat.

**[Rejected 26 Oct] Optional alternative #13: ZIP 318-style bucketed expiry.** At NU7, interval E = 103,680 and expiry = `(h − h % E) + 2E`, approximately **30–60 days** from scheduled broadcast. Released migration code exists, but this implies long validity and old anchors, not the same merchant risk model. The discussion leaned against it for v1; **not selected**. See [prior art](../prior-art.md).

## Open

### Eight research open items

1. Validate the chosen ≤7-day uplink assumption and merchant willingness to hold debt; the 72,576-block shutdown profile was accepted 26 October, not measured as an availability guarantee.
2. Whether to relax thirty-minute new acceptance; recompute detection windows and loss caps if so.
3. Absolute signer-epoch fence, signing cessation/rotation and recovery reconciliation.
4. Repair wire schema, reasons/states and same-txid refresh statement semantics.
5. Generic tree updates/checkpoints, witness persistence, mesh bandwidth and privacy.
6. Sensitive pre-IO persistence format, library upgrades and lost-state behavior.
7. Future upgrade-config authority, freshness and offline distribution.
8. FROST chosen-α/coordinator security and escrow-key derivation review; research did not close them. Use ZF's current seed-and-commitment rerandomized construction rather than copying the deprecated explicit-randomizer path; do not copy draft DKG `sk=ext` ([prior art](../prior-art.md)). Compatibility with this PCZT workflow remains a gate.

**Required v1.1 failure model:** no conflicting still-valid authorization for a
locked nullifier, covering rollback/cloning, concurrency/retries, ambiguous
crashes, nonce consumption, replacements, device loss and every alternate
recovery/spend-key path. FROST supplies neither honest non-rollbackable C nor
availability. The [group model](double-spend.md#failure-model-gate-1-written-26-october-reasoned-not-yet-tested)
is written; enforcement/tests remain gates.

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

The expiry research used explicit `generate`; the subsequent v0.1.0 bench confirmed external submissions need explicit mining ([AGENTS.md issue #7](../../AGENTS.md)). The failed 120-second rolled-back-block replay had no diagnosed upstream cause; coinbase-only scratch blocks replaced it. The maximum-expiry error wording defect is logged separately; none of these establishes a new consensus restriction.
