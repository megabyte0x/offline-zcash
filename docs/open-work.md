# Open work

Ordered. Each item names the component doc that owns it. Update this list as items close.

## Design problems (in order)

Status saved 26 October 2026. “Settled” means design agreed, not implemented.

| # | Problem | Status / owner |
|---|---|---|
| 1 | Offline double-spend | **Settled:** pending IOU tier now, exclusive nullifier-lock co-signer v1.1 ([double-spend](components/double-spend.md), [pending-tier](components/pending-tier.md)) |
| 2 | Chain-state trust at B | **Settled**, with two owner decisions below still pending ([payee-verifier](components/payee-verifier.md)) |
| 3 | Transaction expiry and re-issue | **Research complete; proposals awaiting owner decision**, not ratified or implemented ([expiry-reissue](components/expiry-reissue.md)) |
| 4 | Gateway protocol | **Next design step:** bundle/receipt wire encoding, signature coverage, retries/idempotency and lag/fork handling ([gateway](components/gateway.md)) |
| 5 | BitChat durability | Next after #4: app queue beyond transport TTL, ACK meaning, restart/retry ([transport-bitchat](components/transport-bitchat.md)) |
| 6 | Fleet fingerprint | Next after #5: expiry/anchor/shape/uplink privacy tradeoff ([privacy-and-threats](privacy-and-threats.md)) |
| 7 | Stack compatibility | Next after #6: pinned Zakura bytes accepted by Zebra/upstream, deliberate library migration ([chain](components/chain.md)) |
| 8 | Offline limits and phone proving cost | Next after #7: note budget, no unmined change, real Ironwood phone time/RAM ([offline-builder](components/offline-builder.md)) |
| 9 | Policy values | Next after #8: measured exposure, freshness, anchor window, confirmations and loss appetite ([pending-tier](components/pending-tier.md)) |

### Awaiting owner decision

- **Problem #3:** individually review the [twelve proposals plus optional #13](components/expiry-reissue.md#proposed--awaiting-owner-decision): courier/shutdown profiles (proposed shutdown default, not chosen), payment families/count-once, complete C locks/absolute epoch fence, proactive/requested repair, expired-unresolved/debt retention, four-clock budget, activation/config cap, hostage debt, zero refusal, encrypted pre-IO snapshot, A-only re-anchoring/tree updates, app re-queue. ZIP 318 buckets remain an unselected alternative, discussion leaned against v1.
- The external **owner visual walkthrough** exists: [commentable expiry/re-issue guide](https://t-03h472h7xgrjhpyj2u0ruxd9v-p28781.onamp.dev/expiry-reissue.html) (guide thread). It covers two profiles, ten scenarios and rejection tradeoffs; not copied into this checkout. No ratification was recorded there.
- **Problem #2 leftover A:** unify exposure reset to `confirmed` (**10 confirmations**, provisional low-value demo policy) on both harness and mainnet? Existing docs had `included` on harness versus `confirmed` on mainnet; no choice is made in this snapshot.
- **Problem #2 leftover B:** Receipt v1 has no `checked (single-source)` slot: new state byte versus flag bit. Decide before finalizing gateway/receipt encoding.
- Before **v1.1 safety claims**, write C's failure model (rollback/cloning/concurrent signing/replacement authorization and all recovery paths), and resolve FROST rerandomization / DKG derivation security gates. [Prior-art inventory, flaws, negative searches and contacts](prior-art.md) informs these decisions; cutoff 25 October 2026.

## Build MVP v1 (in order)

This is the implementation backlog, distinct from the problem-by-problem design order above. Do not treat pending problem #3 policies as approved build requirements. Research PoCs do not implement the app.

| # | Task | Done when | Doc |
|---|---|---|---|
| 1 | Offline builder: one v6 Ironwood transaction with Zakura Common 2.2.0, no network, fleet expiry and fee | Raw bytes written; mined on `ths` | [offline-builder](components/offline-builder.md) |
| 2 | Payee verifier in a separate process: read, decrypt own output, proofs and signatures, v6/Ironwood-only/branch ID/expiry/anchor checks | Accepts the tx from 1; receipt says "passes checks at height H; pending" | [payee-verifier](components/payee-verifier.md) |
| 3 | Gateway submitter: `sendrawtransaction`, then a signed bundle back (tip header, txid branch) | Idempotent resubmission; B verifies the branch and moves to "included" | [gateway](components/gateway.md) |
| 4 | BitChat fork on four Macs: tx + statement as private message A→B, B persists, B→D forward, receipts back | The restart test below passes | [transport-bitchat](components/transport-bitchat.md) |
| 5 | Pending tier on payer and payee: device-bound payment keys and enrollment, PaymentStatement and Receipt, fee check, seq, limits, conflict detection | Floor refusal, 4th payment refused, reset at owner-selected threshold (pending), non-family spend ends "conflicted"; repair/family rules await #3 | [pending-tier](components/pending-tier.md) |
| 6 | Payee-to-payee nullifier sharing over BitChat group messages, with C as second payee; gateway mempool receipt | Same note paid to B then C while connected: C refuses with reason 8 | [pending-tier](components/pending-tier.md) |
| 7 | Chain state, step 1: bundle wire format with every slot, signed with the gateway's enrollment payment key; gateway rebuilds the history MMR from `getblock h 0` + `z_gettreestate`, persists leaves, rebuilds on reorg; payee checks signature, anchor-in-window, expiry margin, txid branch, MMR anchor path to header N+1 | Tests 8–14 pass on `ths` | [gateway](components/gateway.md), [payee-verifier](components/payee-verifier.md) |
| 8 | Chain state, step 2: Equihash (200,9) and difficulty checker against a pinned floor from a setup checkpoint; `regtest-no-pow` flag | Tests 15–16 pass | [payee-verifier](components/payee-verifier.md), [chain](components/chain.md) |
| 9 | Chain state, step 3: two-gateway tip agreement; `checked (single-source)` with the stricter cap; targeted conflict notices; full nullifier list on request | Tests 17–19 pass with a second gateway process | [payee-verifier](components/payee-verifier.md), [pending-tier](components/pending-tier.md) |
| 10 | Tor submission path on the gateway: fresh circuit per tx, random delay | A send reaches a public endpoint through Tor; endpoint sees only the exit IP | [gateway](components/gateway.md) |

## Problem #2, decided 25 October 2026: chain state without trusting the gateway

B is offline and learns the tip, anchors, nullifier spends and inclusion only from D. D is untrusted (ADV-G) on every network, so each chain fact is checked against proof of work and Merkle paths; the table is in [payee-verifier.md](components/payee-verifier.md), the gateway duties and bundle contents in [gateway.md](components/gateway.md). Three sub-decisions:

1. **Single-gateway degradation.** With one gateway reachable, B still verifies PoW and paths but marks the state `checked (single-source)` and applies the stricter ⌈C/2⌉ cap until a second source confirms. [decided]
2. **Nullifier delivery.** Targeted conflict notices (`nf → T′` with T′'s bytes and txid branch) by default; the full nullifier list for the fleet window on request. [decided]
3. **Gateway signing key.** D signs bundles with its enrollment payment key, the same per-device Ed25519 scheme as payer and payee, so two conflicting signed tips from one gateway are portable evidence. [decided]

The Regtest harness has no proof of work, so only the Equihash/difficulty/timestamp check is disabled there, under an explicit `regtest-no-pow` flag that is itself tested with mainnet header vectors. Everything else runs unchanged on the local chain.

## Tests that gate MVP v1

1. Fabricated-anchor transaction is rejected by the payee. ([payee-verifier](components/payee-verifier.md))
2. Deliberate conflicting spend: B reports "conflicted", not "final". ([double-spend](components/double-spend.md))
3. A sends to B → B confirms only after saving → A disappears → B restarts → D appears later → B delivers, D submits, confirmation returns. ([transport-bitchat](components/transport-bitchat.md))
4. Packet capture: no plaintext amounts, payees or keys on the mesh. ([privacy-and-threats](privacy-and-threats.md))
5. Same tx bytes accepted by Zebra 7, not only Zakura. ([chain](components/chain.md))
6. BitChat delivery time for a 9,166-byte tx plus statement (base64 ≈ 12.7 KB) over 1 and 3 hops. ([transport-bitchat](components/transport-bitchat.md))
7. A statement with a wrong `auth_digest`, wrong amount, replayed `seq` or non-fleet fee is refused with the right reason code; a receipt is sent only after the payee's store is fsynced. ([pending-tier](components/pending-tier.md))
8. **New-payment acceptance** anchors inside/outside the provisional 100-block window: accept/refuse with reason 6. Not a consensus age bound; test existing-family historical-root renewal separately after #3 approval. ([payee-verifier](components/payee-verifier.md))
9. A tampered bundle and a bundle signed by an unknown key are rejected before any other check. ([gateway](components/gateway.md))
10. Txid branch verifies for a block with 3 transactions and fails for the wrong txid or the wrong position. ([gateway](components/gateway.md))
11. Expiry on both sides of the owner-approved lag/uplink/confirmation/margin budget (#3 proposal 6). Initial H + 20 remains **provisional and insufficient** against thirty-minute view lag (~72 NU7 target blocks), not a safety guarantee. ([payee-verifier](components/payee-verifier.md))
12. The gateway's rebuilt history roots match every header's `hashBlockCommitments`, including after `ths reorg` and `ths rollback`; the rebuilt root agrees with `zcash_history::Tree`. ([gateway](components/gateway.md))
13. Real anchors verify against header N+1 and the tip; a flipped anchor, leaf, peak position or `authDataRoot` fails. ([payee-verifier](components/payee-verifier.md))
14. Conflict notice: a deliberate conflicting spend delivered as `nf → T′` with T′'s bytes and branch ends as `conflicted` (extends test 2). ([payee-verifier](components/payee-verifier.md))
15. A real mainnet header passes the Equihash (200,9) and difficulty check and a corrupted one fails, with `regtest-no-pow` off; with it on, a live 177-byte Regtest header is accepted and all other bundle checks still run. ([chain](components/chain.md))
16. An anchor from before an upgrade activation is proved against the pre-activation tree; needs a mainnet-like chain (issue log #1 in [AGENTS.md](../AGENTS.md)). ([chain](components/chain.md))
17. One lying gateway of two is detected: conflicting signed tips are kept as evidence and the payee refuses pending acceptance until they agree. ([gateway](components/gateway.md))
18. With one gateway, the state is `checked (single-source)` and the ⌈C/2⌉ cap is a hard cap; a second agreeing gateway lifts it. ([pending-tier](components/pending-tier.md))
19. The full nullifier list for the fleet window arrives on request and the payee finds the same conflict as the targeted notice. ([gateway](components/gateway.md))

## Next phase (MVP v1.1): nullifier-lock co-signer

- One component: 2-of-2 FROST DKG through `zakura-reddsa`'s `frost` feature, escrow FVK from `FullViewingKey::from_bytes`, redacted PCZT, co-signer recomputes the sighash, durable nullifier lock and nonce store, expiry cap; about 400–700 lines of Rust plus Swift bindings. Deploy with mesh co-signer C first, then as the bilateral configuration. ([double-spend](components/double-spend.md))
- Gating tests: a second spend of a locked nullifier to another payee is refused; **same-action/same-proof expiry renewal with fresh signatures is mined from real-funded FROST notes**, old bytes rejected after expiry; lock/nonce crash safety, restored-C immutable fence/reconciliation and `rk` group binding. Synthetic-tree FROST variants validated, not mined. [Ten problem #3 gates](components/expiry-reissue.md#ten-gating-tests-not-completed-by-the-research) add families, branches, reorgs, gateway disagreement, lifetime budgets and >24-hour transport.
- Follow-up to [`poc/escrowcheck`](../poc/escrowcheck/README.md): fund the escrow address from a dev account on `ths`, spend with a real witness, and mine it.
- Escrow-tier conflict notices use the same bundle slot as problem #2 above; the co-signer's lock store should also consume them. ([payee-verifier](components/payee-verifier.md))

## MVP v1.2

- Co-signer recovery: 2-of-3 {payer, hot co-signer, cold fenced recovery share} or timelock-encrypted release of the co-signer's share. ([double-spend](components/double-spend.md))

## Known gaps carried forward (not fixed by design)

- The gateway is the first spy, and one gateway cannot prove that no conflict exists (completeness); ≥ 2 gateways, nullifier sharing and loss caps bound it. ([gateway](components/gateway.md))
- Headers cannot prove the heaviest chain; the pinned difficulty floor must track mainnet. ([payee-verifier](components/payee-verifier.md))
- Fleet transactions are fingerprintable on chain. ([privacy-and-threats](privacy-and-threats.md))
- No expiry profile chosen: proposed 3-day courier assumes ≤1-day uplink; proposed 21-day shutdown assumes ≤7-day uplink. Neither covers arbitrary shutdowns, and thirty-minute **new-acceptance** freshness remains provisional/separate. Renewal needs A even in 2-of-2 escrow; refusal leaves a debt claim. ([expiry-reissue](components/expiry-reissue.md))
- Mainnet needs Common 4.x or custom params with the NU7 height (library step). ([chain](components/chain.md))
- Unmeasured: Equihash and Ironwood proving on Mac/iPhone; mainnet proof sizes. The 100-block new-payment anchor window, 20-block initial expiry margin, thirty-minute freshness and ten confirmations are **provisional** with the reasons above, not validated mainnet safety bounds. ([gateway](components/gateway.md), [pending-tier](components/pending-tier.md))
