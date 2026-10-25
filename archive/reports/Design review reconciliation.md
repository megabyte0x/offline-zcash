# Design review reconciliation

Status: 25 October 2026. This note reconciles two independent reviews of `MVP v1 design.md`, `Offline Zcash over Logos mesh.md` and `research_notes/`, both measured against the three-device scenario (A pays B, both offline; C is online and submits):

- Review 1: internal research notes (gap and security analysis).
- Review 2: internal research notes (source-level audit).

Where they disagreed, the point was re-checked in source; the evidence decides. Nothing here was run; all checks are source reads.

## 1. Finalized (both reviews agree)

These are fixed; later work builds around them.

- **The mesh carries a finished transaction.** A builds, proves and signs offline; B checks and stores; C submits the bytes and holds no spending or viewing key. Value moves only when mined.
- **Txid is the transport identity; resubmission is idempotent.** Re-issue spends the same notes so at most one version mines. (Caveat from review 2: a v6 re-anchor keeps the txid, so caches key on txid plus auth digest, as Zakura does.)
- **Local `ths` v0.4.1 NU7 chain first, mainnet after NU7 activates** (height 3,543,405) with Common 4.x or custom mainnet parameters, each send approved.
- **Offline builder on Zakura Common `=2.2.0`**, the stack pinned in `AGENTS.md`.
- **Received funds cannot be re-spent offline.** Unmined change is unspendable; pre-split notes cap the number of payments.
- **Pending is an IOU.** No offline check proves the inputs are unspent. "Final" only after mining.
- **FROST quorum (layer 1) is out of the three-device MVP**; it needs at least 4 committee signers plus the payer.

## 2. Common gaps (both reviews found them)

| # | Gap | Priority |
|---|---|---|
| G1 | The step-3 public broadcast of a signed intent (nullifiers, txid, payee, amount, expiry, payer key) to every relay and the gateway is a plaintext payment record. BitChat public payloads are signed, not encrypted. | P0 |
| G2 | Identity-keyed controls (per-payer counters, hot list of payer keys, bonds) link every payment to a stable identity. Workable only inside a closed fleet; not with strangers. | P0 |
| G3 | "Verified" does not mean "funded": `BatchValidator` checks proofs and signatures against the anchor the transaction states, which the payer can fabricate. Expiry, branch ID and unspent inputs are not checked either. | P0 |
| G4 | The gateway is a trust point: it can withhold, delay, forge chain state and sees every transaction first. | P1 |
| G5 | Bonded intents (layer 2) have no shielded refund path and put the bond in committee custody. | P1 |
| G6 | Fleet transactions are fingerprintable (stale anchor, long expiry, burst timing). | P2 |
| G7 | Zebra 7 acceptance of transactions built with Zakura 2.2.0 is unverified. | P1 |
| G8 | Expiry vs outage length: a 3-day expiry is short against real shutdowns, and re-issue needs the payer. | P2 |
| G9 | The transport for laptops was undecided: BitChat has no Linux or Windows client. Resolved: the laptops are Macs, so BitChat. | Resolved |

## 3. Contradictions, re-checked

| Topic | Review 1 | Review 2 | Re-check | Decision |
|---|---|---|---|---|
| `lock_time` on shielded v6 | Open question; drop layer 2 if not enforced | Ignored | Zakura v1.6.0 (`ebda15e`, pinned by ths v0.4.1) `zakura-chain/src/transaction.rs` L620–644 returns `None` unless a transparent input has a non-final sequence; Zebra main has the same logic. | **Review 2.** Not enforced, so the bond has no refund path. Layer 2 as written is dropped. |
| FROST 2-of-2 bilateral escrow in the MVP | Promote into MVP | Keep FROST out of the first demo | frost-tools (`53df99c`) supports Ironwood v6 but on upstream crates that cannot link with Zakura 2.2.0. zakura-orchard 2.2.0 has `unstable-frost`, `SpendValidatingKey::from_bytes`, `FullViewingKey::from_bytes`, and zakura-pczt rc4 has `sign_ironwood_with`. Feasible with about 300–500 lines of glue; FVKs are non-standard and not quantum-recoverable. | **Review 2 for ordering.** Base flow first; bilateral escrow is the next phase, because it is the only real double-spend protection available with three devices. |
| What headers can prove | PoW headers + ZIP 221 MMR can authenticate anchors | Headers don't vouch for compact-block data; no proof of non-spend | ZIP 221 leaves carry Orchard roots; ZIP 258 (Final) adds Ironwood roots; Zakura 1.6.0 implements it (`history_tree.rs`, `zcash_history.rs`). No nullifier commitment exists (ZIP 221 L660–663). Headers cannot show C's chain is the heaviest. | **Both, on different points.** Anchors and inclusion can be authenticated (needs MMR proof code that does not exist yet); non-spend cannot. "Proof a nullifier was never spent" is removed. |
| 2,000-zat fee line (report Phase 0) | Error | Not raised | Zebra ≥ 6.4.0 accepts 1,000 per action; ZIP 2009 (Draft) lowers ZIP 317 to 1,000; Zakura 1.6.0 accepts 400 per action. | **Not an error.** The fleet fee stays 10,000 zats (what the wallet emits), as the design already says. |
| Logos Store nodes | Store on all three | Two Store nodes suffice | No code requires three. | Two suffice. |
| Laptop transport | Logos Delivery private cluster | BitChat on three Macs | See section 4. | **Review 2.** The laptops are Macs, so BitChat (decided 25 October 2026). |

## 4. Transport facts found during the re-check (neither review had them)

- **BitChat private messages have a persistent sender outbox:** sealed on disk, 24 h, 8 attempts, resent until acknowledged (`MessageRouter.swift` L128–135, L462–483 at `04995f68`). So "B offline when A sends" is covered if the transaction goes as a private DM (base64, about 12.3 KB, under the 60,000-character limit). B must copy it to its own storage on receipt, because timelines are memory-only. Private file sends are not outboxed. macOS 13+/iOS 16+ only.
- **Logos Delivery (`logosdeliverynode` v0.39.2):** Linux x64 and macOS arm64 binaries. Store is off by default with 2-day retention (`--store-message-retention-policy=time:1209600` for 14 days). The sender gives up after 1 minute with no connected peer (`send_service.nim` `MaxTimeInCache`). Store sync defaults to a 1 h window (`--store-sync-range`), and `--store-resume` is capped at 6 h. `--reliability` works only with `--entry-layer=messaging|channels`. Never use cluster IDs 1 or 2 (they force public presets).
- **Decision:** the three laptops are Macs, so the demo uses the BitChat fork (matches the iOS plan). Logos stays for the later Logos phase. Transport rules are in `MVP v1 design.md`.

## 5. Broken items to fix first (agreed by both reviews, or confirmed by the re-check)

1. **G1:** relays and the gateway get transaction bytes only. Payment details and the payer's accountability signature go to the payee over the Noise-encrypted private channel. Applied in `MVP v1 design.md`.
2. **G3:** the payee checks v6, Ironwood-only, branch ID, expiry against its last known height, and the anchor against a set of recent anchors from chain bundles. The receipt reads "passes checks at height H; pending". Applied.
3. **Note release:** the rule "proof from chain data that its nullifier was never spent" is impossible. Release relies on a sync the device trusts. Applied.
4. **Layer 2 bond:** dropped as written (no refund path; custody). Applied.
5. **Fraud proof → conflict evidence:** a nullifier in a different txid proves a conflict, not fraud (re-issue and key theft produce the same pattern). Hot lists hold nullifiers, never payer keys, and nothing is hot-listed automatically. Applied.
6. **Bond cap claim** ("total offline liability ≤ k × floor limit") is false across partitions: one credential shown to 100 disconnected payees exposes 100 × the limit. Counters are local loss controls only. Superseded in `MVP v1 design.md`; the research note still contains the old claim.

Superseded text in `Offline Zcash over Logos mesh.md` (not edited): the intent fan-out to gateways (step 5 of the flow), "proof the nullifier was never spent", the intent-signature attribution rule, and the open `lock_time` question.

## 6. Tests both reviews ask for

1. Build one offline v6 transaction on `ths`, verify in a separate payee process, mine it.
2. Submit the same bytes to Zebra 7.
3. Fabricated-anchor transaction: the payee must reject it.
4. Deliberate conflicting spend: B reports "conflicted", not "final".
5. Packet capture: no plaintext amounts, payees or keys on the mesh.
6. Restart B between receipt and forwarding; C appears later and submits.
