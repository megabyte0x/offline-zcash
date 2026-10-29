# Double-spend: acceptance tiers and the co-signer group

Owner decision, 26 October 2026, replacing the [25 October single-C 2-of-2 design](../../archive/reports/Double-spend%20design%202026-10-25.md). Its "Co-signer facts verified in source" section retains PCZT, sighash, α, `rk` and FVK API citations.

**Unbuilt design.** v0.7.7 has reservations, conflict checks and invited-payer
admission caps, no open/unknown pending acceptance, no co-signer service and no
independent chain receipts. Its ten gateway-reported confirmations are not the
final tier below. FROST PoCs retain their older pins; the app uses Common 4.0.

## The constraint

A or an online seed-holder can sweep a note while B's tx1 is on the mesh—even
**before** paying B. Consensus sees valid signatures, not mesh state, witness
signatures or bonds. Prevention requires key splitting, trusted hardware or
waiting for the chain. Native ZEC without special hardware or a mint leaves
**key splitting** as the owner's chosen prevention.

## Decided

| Tier | When | Stops the online sweep? |
|---|---|---|
| Pending | Below the floor (0.5 ZEC, provisional), any note | No. Loss capped ([pending-tier.md](pending-tier.md)) |
| **Co-signed** | Above the floor, escrow notes | Yes, if at most 1 of 4 co-signers lies **or** is unavailable |
| Receipt-gated | Above the floor, ordinary notes, B can reach a gateway | Narrows the race only; prefer a verified block before handing over goods |
| Final | Mined, verified by B ([payee-verifier.md](payee-verifier.md)) | Yes (consensus, reorg risk aside) |

- **Group:** A plus 4 independent co-signers; A and any 3 sign. Co-signers cannot sign without A.
- **Keys:** stock `zakura-reddsa` 2.2.0 `frost`, threshold 7 of 8 identifiers: A holds 4, each co-signer 1. Measured layout: t = n + ⌊n/2⌋ + 1. Smaller 5-of-6 (A holds n − k + 1, threshold n + 1) is equivalent on paper, unmeasured. Nested additive keys need custom FROST and were rejected.
- **Routing:** the nearest co-signer may relay, but 3 must sign. "Any one" lets A obtain signatures in separate partitions.
- **Locks:** one complete-effects family per nullifier; fsync locks and consumed nonce before releasing a share. No timeout unlock; restore/clone counts as a liar. Full rules below.
- **No viewing key:** recompute sighash from a redacted PCZT; check `rk` and outputs to the stated payee (archived APIs above).
- **Placement:** near A over mesh, without internet; the gateway may be days away.
- **Payee check:** every `rk` derives from the certified group key with A's private α. Threshold participation is invisible on chain; B trusts the enrolled certificate.
- **Bilateral option:** the payee alone co-signs recurring pairs, accepting a single point of failure.

## Findings

Co-signer research evidence: `quorum-frost-evidence.tar.gz`, retained in its research orb.

- **Weighted layouts sign and mine.** Every payer-plus-majority subset passed `apply_ironwood_signature`, `TransactionExtractor::extract` and decryption: 4 subsets for n = 3 and 5 for n = 4. Two real-funded 9,166-byte v6 transactions were mined on `ths` at heights 107 and 109.
  - DKG took 57 ms (n = 3) and 93 ms (n = 4). Signing took 5–7 ms. Single samples on a 2-vCPU x86 orb.
  - The signature is 64 B whatever the quorum. DKG payload is 2,496 / 4,432 B.
  - Payer-only, co-signer-only and payer-plus-minority attempts were rejected by the API. That is not a proof of unforgeability.
- **2-of-3 fails with one liar (measured).** A signed one spend with C1 and C2 and a conflicting one with C2 and C3. Both extracted; the first was mined and the second was rejected only as a duplicate nullifier.
- **Quorum rules:**
  - Safety against f liars needs 2k − n > f. Against crashes only, 2k > n.
  - Availability needs k ≤ n − f − c. Combined: n ≥ 3f + 2c + 1.
  - So 3-of-4 handles 1 liar or 1 offline co-signer, not both.
  - A's extra identifiers add no safety.
  - Conflicting concurrent proposals can strand a note, which only hurts A.
- **Rejected alternatives,** researched 26 October:
  - Transparent CLTV bond: CLTV is enforced on v6 (measured) and CSV is not implemented. A self-sweep produces no second statement, so nothing gets slashed.
  - Witness quorum: needs N ≥ 3f + 1 and q = 2f + 1. It prevents conflicting mesh certificates, not the sweep. Not adopted.
  - Clean-slate survey: hardware purses and mints were ruled out by the owner. Timelocked keys, cheques and pre-allocated notes don't prevent double-spends.
- **Race facts** (race research):
  - Zebra 7.0.0 (`e8db87e`) rejects a mempool Ironwood-nullifier conflict before insertion, with no fee replacement (`zebrad/src/components/mempool/storage/verified_set.rs` L153–162, L323–330). Zakura `ebda15e` does the same.
  - First-seen is local and temporary: a block holding tx2 overrides it.
  - zcashd has no Ironwood support.
  - Head-start block probability 1 − e^(−Δ/T) is a lower bound on sweep success, not the success rate.
- **Carriage time is A's window** (latency research): modelled, not measured, at roughly 1 h in a city, 5 h at 20 km rural, 1–2 days at 100 km in mountains, and 4+ days cross-country, and possibly never. Receipt-gated acceptance is practical only near a gateway.

## Failure model (gate 1; written 26 October, reasoned, not yet tested)

**Invariant:** at most one *payment family* gets a valid group signature per
escrow nullifier nf. Canonical effects—actions, flags, value balance, pool,
format—stay fixed; versions change only expiry/branch within the absolute fence.
With **at most one faulty co-signer**, two 3-of-4 quorums overlap in 2 members,
including an honest member who refuses a second family.

**What every co-signer must do:**
- **Lock at round 2.** Round-1 nonce commitments release nothing. Before releasing a round-2 share, one fsynced transaction compare-and-sets `nf → family digest` for **every** PCZT spend, writes the absolute fence and consumes the session nonce. Refuse if any nf belongs to another family.
- **Never unlock:** not for expiry, timeout, reorg, restart or operator request. Retention costs 72 bytes per nf.
- **Single-use, memory-only nonces:** discard unconsumed nonces on restart; start a new session. Persist only consumption, never reusable nonces.
- **Renew within the family:** identical canonical effects, expiry ≤ fence and nonzero, fleet-configured branch.
- **Device-bound state:** seal shares/locks with a `ThisDeviceOnly` keychain key, Secure-Enclave-wrapped where available; exclude from backups.

| Failure | What happens | Within the fault budget? |
|---|---|---|
| One co-signer lies (signs two families for one nf) | The other quorum still holds an honest member who refuses | Yes, f = 1 |
| A co-signer is restored from a backup or cloned | Its old locks are gone, so it can sign again. **Counts as the one liar** | Uses up f; retire it (reconfiguration) |
| Two co-signers faulty, or one liar plus one restored | Two families can get signatures; the chain mines one | **No.** Detected only on chain |
| A + 2 co-signers collude | A double-spend is possible | **No.** Same bound |
| Crash after fsync, before sending the share | The lock exists and nothing escaped. The same family is re-signed in a new session | Yes |
| Crash before fsync | Nothing locked, nothing released | Yes |
| Two concurrent requests for one nf | The compare-and-set serialises them; the second family is refused | Yes |
| A splits the first votes (C1 and C2 lock P1, C3 and C4 lock P2) | Neither family reaches 3, so the note is stuck forever | Hurts only A; accepted |
| 1 co-signer offline | The other 3 sign | Yes |
| 2 co-signers offline | Escrow notes can't be spent; ordinary notes fall back to pending or receipt-gated | Accepted; availability only |
| A loses all 4 shares | Funds are lost unless A backed up its shares (allowed; useless without a quorum) | Accepted |
| A retired co-signer keeps its old share | Refresh and repair don't revoke it, so it is a potential liar until the notes move | Replacement means sweeping every escrow note to a new group key online |
| A fake group key (A alone controls Y) | B's `rk` check would pass | B accepts Y only with a certificate signed by all 4 co-signers' enrolled payment keys at DKG |
| A malicious α or coordinator | Open: randomizer gate (Open 2) | Unverified |

**Evidence:** two mined spends of one nf under the group key show threshold
failure, but FROST hides participants. Each co-signer therefore retains a signed
family-approval record with its lock for attribution; no record for a mined
family means it was not in that quorum.

## Open

1. **Safety gate:** test fsync/release crashes (no second release or nonce reuse), concurrent requests for one nf, retirement after restore, and rejection of uncertified group keys.
2. **Randomizer:** review of payer-chosen α against Re-Randomized FROST ([ePrint 2024/436](https://eprint.iacr.org/2024/436)). Deprecation is not a forgery finding, but fixed-α same-proof renewal stays a gate. Don't copy draft DKG `sk=ext`.
3. **Reconfiguration** [proposed]: implement the online sweep to a new group key described above; share repair/refresh cannot replace it. This replaces v1.2's fenced-cold-share/tlock options. A's own share backups are allowed; without them total share loss is unrecoverable.
4. Escrow FVK against ZIP 2005 `use_qsk` vectors; `Signer::new` on a redacted PCZT; payee policy checks for real, dummy and mixed inputs.
5. Group size, co-signer enrollment and placement per town; mesh and phone timing for one co-signing round.
6. Harness layout [decided 26 Oct]: 4 co-signer processes on B, C (two) and D; A is never its own co-signer. Demo only.
7. Expiry/renewal and reorg gates from [expiry-reissue.md](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research), now with the group.
