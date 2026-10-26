# Double-spend: acceptance tiers and the co-signer group

Decided 26 October 2026 by the owner. This replaces the 25 October design (single 2-of-2 co-signer C), now in [archive](../../archive/reports/Double-spend%20design%202026-10-25.md). Its "Co-signer facts verified in source" section remains the line-level reference for PCZT, sighash, α, `rk` and FVK APIs.

## The constraint

The chain accepts any validly signed spend, and A's spending key alone is enough. So A, or an online friend with A's seed, can sweep a note to itself while B's tx1 is still on the mesh, even **before** paying B. Mesh state, witness signatures and bonds are invisible to consensus. Only three things stop that sweep: splitting the spending key, trusted hardware, or waiting for the chain. The owner chose native ZEC with no special hardware and no mint, which leaves **key splitting** as the only prevention.

## Decided

| Tier | When | Stops the online sweep? |
|---|---|---|
| Pending | Below the floor (0.5 ZEC, provisional), any note | No. Loss capped ([pending-tier.md](pending-tier.md)) |
| **Co-signed** | Above the floor, escrow notes | Yes, if at most 1 of 4 co-signers lies **or** is unavailable |
| Receipt-gated | Above the floor, ordinary notes, B can reach a gateway | Narrows the race only; prefer a verified block before handing over goods |
| Final | Mined, verified by B ([payee-verifier.md](payee-verifier.md)) | Yes (consensus, reorg risk aside) |

- **Co-signer group:** payer A plus 4 independent co-signers. A signs together with any 3 of them; co-signers can't sign without A.
- **Keys:** stock FROST through `zakura-reddsa` 2.2.0 `frost`. A holds 4 identifiers, each co-signer holds 1, and the threshold is 7 of 8. That is the layout t = n + ⌊n/2⌋ + 1 that was measured. The smaller 5-of-6 layout (A holds n − k + 1 identifiers, threshold n + 1) is equivalent on paper but unmeasured. Nested additive keys are rejected because they would need custom FROST.
- **Routing:** the nearest co-signer may relay the request, but a quorum of 3 co-signers must sign. "Any one co-signer" is unsafe: A would get one signature per partition.
- **Lock rule:** each co-signer signs at most one complete-effects payload per nullifier. Before releasing a share, it writes the lock and the consumed nonce in one fsynced transaction. It never unlocks on a timeout. A restored or cloned co-signer counts as a lying one.
- **Co-signers hold no viewing key.** Each recomputes the sighash from a redacted PCZT and checks `rk` and that the outputs pay the stated payee (APIs in the archive file above).
- **Co-signers live near payers.** They need mesh reach to A, not internet. A town can co-sign while its gateway is days away.
- **Payee check:** each spend's `rk` must derive from the certified group key with A's private α. Whether the threshold was met is invisible on chain, so B relies on the enrolled group certificate.
- **Bilateral configuration** (the payee as sole co-signer) stays available for recurring pairs. It has a single point of failure by construction.

## Findings

Measured by the co-signer research; evidence `quorum-frost-evidence.tar.gz` is kept in that thread's orb.

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

**Invariant.** For each escrow nullifier nf, at most one *payment family* ever gets a valid group signature. A family is one canonical effects payload: the same actions, flags, value balance, pool and format. Versions inside a family differ only in expiry or branch, and only within the lock's absolute fence. The invariant holds while **at most one** of the four co-signers is faulty. Any two quorums of 3 out of 4 share 2 co-signers, so at least one honest co-signer sits in both and refuses the second family.

**What every co-signer must do:**
- **Lock at round 2, not round 1.** Committing nonces in round 1 releases nothing. Before releasing its round-2 share, a co-signer runs one fsynced database transaction that does three things:
  - a compare-and-set of `nf → family digest` for every spend in the PCZT;
  - writes the absolute fence;
  - marks the session nonce as consumed.

  If any nf is locked to another family, it refuses.
- **Never unlock.** Expiry, a timeout, a reorg, a restart or an operator request never remove a lock. A lock is 72 bytes per nf, so keeping them forever is cheap.
- **Nonces are single-use and live only in memory.** A restart discards every unconsumed nonce, and the next request starts a new session. Persisted nonce state may only mark a nonce consumed, never restore it for use.
- **Sign renewals only inside the family:** identical canonical effects, a new expiry at or below the fence, the fleet-configured branch, never expiry 0.
- **Keep the share and lock store on the device.** Seal them with a key held `ThisDeviceOnly` in the keychain, wrapped by the Secure Enclave where available. Exclude them from backups.

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

**Evidence.** Two mined spends of one nf under the group key show that the threshold was broken. FROST aggregates hide *which* co-signers signed. So for attribution each co-signer keeps, with its lock, a signed record of the family it approved. A co-signer that holds no record for a mined family wasn't part of that quorum.

## Open

1. **Before any safety claim:** the failure model above (written 26 Oct) must be tested: kill between fsync and release (no second release, no nonce reuse), concurrent requests for one nf, a restored co-signer treated as retired, and B rejecting an uncertified group key.
2. **Randomizer:** review of payer-chosen α against Re-Randomized FROST ([ePrint 2024/436](https://eprint.iacr.org/2024/436)). Deprecation is not a forgery finding, but fixed-α same-proof renewal stays a gate. Don't copy draft DKG `sk=ext`.
3. **Reconfiguration** [proposed]: replacing a lost co-signer. Share repair and refresh keep the group key but neither moves locks nor revokes old shares, so retiring a co-signer means sweeping escrow notes online to a new group key. A may back up its own shares; they are useless without a quorum. Losing all of A's shares with no backup is unrecoverable. This replaces the old v1.2 options (fenced cold share, tlock).
4. Escrow FVK against ZIP 2005 `use_qsk` vectors; `Signer::new` on a redacted PCZT; payee policy checks for real, dummy and mixed inputs.
5. Group size, co-signer enrollment and placement per town; mesh and phone timing for one co-signing round.
6. Harness layout [decided 26 Oct]: 4 co-signer processes on B, C (two) and D; A is never its own co-signer. Demo only.
7. Expiry/renewal and reorg gates from [expiry-reissue.md](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research), now with the group.
