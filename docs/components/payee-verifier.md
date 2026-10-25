# Payee verifier and receipts

The offline payee checks the received bytes, decrypts its own output, applies the pending policy and replies with a signed receipt: "passes checks at height H; pending".
No offline check proves the inputs are unspent or that the payer will not double-spend, so a pending payment is an IOU from a known payer. It becomes final only once mined and confirmed.
The payee learns the tip, anchors, inclusion and conflicts only from the [gateway](gateway.md), which the design treats as untrusted (ADV-G). Every chain fact it acts on is therefore checked against proof of work and Merkle paths, on mainnet and on the local harness alike. The builder that produces the bytes is in [offline-builder.md](offline-builder.md).

## Decided

### Checks, in order [decided]

| # | Check | Source of truth |
|---|---|---|
| 1 | Version is v6 | tx header |
| 2 | Ironwood-only: no transparent, Sapling or Orchard components | tx |
| 3 | Branch ID equals the configured `0x77190AD9` | config ([chain.md](chain.md)) |
| 4 | Expiry is above the payee's last known height, with margin | last verified chain bundle |
| 5 | Anchor is in the payee's set of recent anchors, each proved by an MMR path to a verified header (table below) | last verified chain bundle |
| 6 | Own output decrypts with the payee's ivk; recompute `cmx`; amount and memo (invoice nonce) match the invoice | payee keys |
| 7 | Proofs and signatures are valid | `BatchValidator` |
| 8 | Fee: `valueBalanceIronwood` equals the fleet fee for the action count (10,000 zats for 2 actions) | config |
| 9 | PaymentStatement: enrolled payer whose Noise peer sent it; `txid` and `auth_digest` recomputed from the bytes; `amount_zat`, `expiry_height`, `invoice_hash` match; `seq` above the last from this payer; re-issue rules if flagged | [pending-tier.md](pending-tier.md) |
| 10 | No nullifier of the tx matches a pending share from another payee with a different txid | nullifier index |
| 11 | Pending policy: floor, per-payer count and value caps, payee total, stale-view limit, payer not blocked | payee state |

Code path for 1–7 (functions read in source, not compiled): `Transaction::read(bytes, branch)` → `ironwood_bundle()` → `decrypt_outputs_with_keys(&[ivk])` → `signature_hash(.., SignableInput::Shielded, ..)` → `BatchValidator::new(&vk)`, `add_bundle`, `validate`. Formats, reason codes and policy values for 8–11 are in [pending-tier.md](pending-tier.md).

- Checks 6–7 alone prove only that the tx is well formed against **the anchor it states**, which the payer can fabricate (gap G3, [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md)). Checks 1–5 are required.
- **Escrow tier (v1.1):** when the statement's tier flag is set, the payee also checks that each spend's `rk` equals the certified group key randomized by the α the payer sent privately ([double-spend.md](double-spend.md)).
- Receipt: the 260-byte **Receipt v1** in [pending-tier.md](pending-tier.md), signed by the payee's device-bound payment key, sent only after the payee's store is fsynced. Its `checked` state renders as **"passes checks at height H; pending"**, where H is the payee's last known height. Never "verified" or "funded".
- The payee saves the bytes to its own storage on receipt, because BitChat timelines are memory-only. It then forwards them to the gateway as a separate private message.
- After check 7 passes, the payee posts the tx's nullifiers to the fleet-payee group and, if another payee is connected, waits 5 s before signing the receipt ([pending-tier.md](pending-tier.md)).
- Conflicts are settled only at mining, by nullifier. A nullifier mined in a different txid is **conflict evidence, not proof of fraud**: a re-issue and key theft look the same. With the payer's PaymentStatement it becomes a debt claim against the payer's device key under the fleet agreement. Hot lists hold nullifiers only, never payer keys, and nothing is hot-listed automatically.
- Counters are local loss controls only. They cannot cap total loss across partitions: one credential shown to N disconnected payees exposes N × the limit; nullifier sharing lowers N from payees to partitions.

### Chain-state checks, no trust in the gateway [decided]

Decided 25 October 2026 (problem #2 in [open-work.md](../open-work.md)). The gateway D is untrusted on every network (ADV-G). Each belief the payee holds about the chain is checked as follows; the local chain is a harness and never a reason to skip a row.

| Payee's belief | Check, with no trust in D | Mainnet cost |
|---|---|---|
| Tip is height H, hash X | Header chain from a pinned setup checkpoint: Equihash (200,9) and difficulty against a pinned floor, parent links. A full day of headers is ≈ 5.6 MB over BLE, so the payee receives the tip header plus the work since the last checkpoint sampled; where two gateways are reachable both tips must agree | Equihash verify 96 µs/header on an orb Xeon, ≈ 0.33 s per day of headers [unverified on Mac/iPhone] |
| Anchor from block N is a real Ironwood root | MMR path from leaf N to `hashBlockCommitments` of header N+1 (needs `authDataRoot(N+1)`, 32 B); header N+1 must lie under the verified tip | 3–9 KB per anchor [modelled at 290 B/node, not measured]; 1,109–2,778 B measured on Regtest |
| Txid T was mined | Txid Merkle branch to a verified header. The v6 txid commits nullifiers, outputs, value balance and expiry ([ZIP 229](https://zips.z.cash/zip-0229)), so no authorizing-data path is needed | 5–11 KB [unverified] |
| Nullifier nf was spent elsewhere (conflict) | D sends a targeted notice nf → T′ with T′'s raw bytes (~9.2 KB) and txid branch; the payee verifies it like inclusion. Absence of a notice is not proof of absence | ≈ 10 KB per conflict |
| No conflict exists (completeness) | Cannot be proved by one gateway. Mitigations: ≥ 2 independent gateways for the same window, payee-to-payee nullifier sharing ([pending-tier.md](pending-tier.md)), loss caps | — |

- **Single-gateway degradation:** with one gateway reachable, the payee still verifies PoW and paths but marks the state `checked (single-source)` and applies the stricter cap (the ⌈C/2⌉ outstanding rule in [pending-tier.md](pending-tier.md)) until a second source confirms the tip. [decided]
- **Nullifier delivery:** targeted conflict notices by default; the full nullifier list for the fleet window on request (audits, payees that were out of range). [decided]
- **Bundle signature:** every bundle is signed by the gateway's enrollment payment key ([gateway.md](gateway.md)); a bundle with a bad or unknown signature is dropped before any other check. Two conflicting signed tips from one key are portable evidence. [decided]
- **Harness flag:** Zakura Regtest disables proof of work ([chain.md](chain.md)), so the Equihash/difficulty/timestamp check alone is skipped under an explicit `regtest-no-pow` flag, which is itself tested with mainnet header vectors. MMR paths, txid branches, conflict notices, two-gateway agreement and signatures run unchanged on the local chain. Never infer the network from the solution length. [decided]
- Build order inside v1 (a sequence, not a downgrade): (1) bundle format with every slot, signature, anchor-in-window, expiry margin, txid branch, MMR anchor verifier (15 lines exist in [`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md)); (2) Equihash/difficulty checker with the pinned floor and mainnet vectors; (3) two-gateway agreement and single-source degradation. Gating tests are under Open.
- Verifier size: the prototype's anchor verifier is 15 lines; production estimate 100–150 lines on the payee [unverified].

### Receipt states [decided]

| State | Entered when | Old design term |
|---|---|---|
| received | bytes stored on the payee | — |
| checked | checks 1–11 pass; signed receipt "passes checks at height H; pending" | pending |
| checked (single-source) | as `checked`, but H and the anchor set come from one verified gateway only; the stricter ⌈C/2⌉ cap applies until a second gateway's tip agrees | — |
| submitted | gateway broadcast receipt for this txid arrives | pending |
| included | txid Merkle branch to a PoW-verified header under the tip shows the txid in a block | pending |
| confirmed | 10+ confirmations on PoW-verified headers ([ZIP 315](https://zips.z.cash/zip-0315)). On the harness the count is exercised but carries no security: one local miner, no PoW | final |
| conflicted | one of the tx's nullifiers is mined in a different txid | conflicted |
| expired | the chain passes the expiry height with the tx unmined; the payer re-issues from the same notes | — |
| rejected | a check fails; the receipt carries the reason code ([pending-tier.md](pending-tier.md)) | — |

State is keyed by txid. A re-anchored copy has the same txid, so the auth digest is stored alongside it. Counters reset when a payment reaches `included`; an `expired` payment that is not re-issued stays counted and blocks the payer at this payee.

## Findings

### What offline checks establish

- Decrypt + `cmx` + signatures + Halo 2 proof prove: *if* the anchor is a real Ironwood root, then the spent notes existed at that anchor, the nullifiers are correctly derived and value balances ([zcash_ironwood_testnet.md Q4](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md)). Proofs bind the anchor as a public input ([ZIP 374](https://zips.z.cash/zip-0374)).
- They cannot prove that (a) the anchor is real, (b) the nullifier is not already on chain, or (c) no conflicting spend will be mined first. Check 5 narrows (a) to "as of the payee's last bundle". Nothing closes (c).
- Verifying takes about 4–12 ms per 2-action bundle on desktop hardware, and the verifying key is derived locally.

### What headers can prove

| Claim | Provable from headers? | Basis |
|---|---|---|
| Anchor from block N is a real root | Yes, from header N+1, via an MMR path; verified against live `ths` v0.4.1 | [ZIP 221](https://zips.z.cash/zip-0221) history leaves carry Orchard roots. [ZIP 258](https://zips.z.cash/zip-0258) (Final) adds `hashEarliestIronwoodRoot` / `hashLatestIronwoodRoot`; a tx's `anchorIronwood` equals the leaf's `end_ironwood_root` for block N (start equals end in one leaf, ZIP 258 L176–211). Zakura 1.6.0 builds the leaf in `zcash_history.rs` L494–508. The history tree resets at each upgrade activation (`history_tree.rs` L313–321) |
| Tx is included (its effects: outputs, nullifiers, values, expiry) | Yes: txid branch to `hashMerkleRoot` of a verified header. Mainnet header 1,487 B, Regtest 177 B; branch 5–11 KB on mainnet [unverified]. Txid root matched the header for 132/132 Regtest blocks | the Merkle tree covers the txid, which commits to the effecting data ([ZIP 229](https://zips.z.cash/zip-0229)) |
| The exact bytes (anchor, proofs) were mined | Possible with a `hashAuthDataRoot` path, but **not needed**: the payee stores the auth digest it checked and the txid already fixes the effects | `hashBlockCommitments = BLAKE2b-256("ZcashBlockCommit", historyRoot ‖ authDataRoot ‖ 0³²)`, Zakura `block/commitment.rs` L301–320, `zakura-state/src/service/check.rs` L321–335; `ZcashAuthDatHash` tree in `merkle.rs` L374–405 ([ZIP 244](https://zips.z.cash/zip-0244)). Verified in source |
| `hashBlockCommitments` of header N+1 | Reproducible from historyRoot(N) and `authDataRoot(N+1)`; the anchor proof therefore needs `authDataRoot(N+1)` (32 B) in the bundle | 131/131 rebuilt roots matched on `ths` ([`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md)) |
| A nullifier was never spent | **No** | No nullifier-set commitment exists ([ZIP 221](https://zips.z.cash/zip-0221) L660–663) |
| The chain shown is the heaviest | **No** | Headers show work, not that a heavier chain does not exist. Cross-check bundles from ≥ 2 gateways and validate Equihash PoW against the pinned floor |

- No light-client MMR proof code exists in Zcash libraries: `zcash_history` `tree.rs` only appends, truncates and returns the root; nodes keep only peaks ([gateway.md](gateway.md#findings)). The gateway rebuilds the tree and the payee runs the 15-line path verifier from the prototype.
- Regtest blocks carry no proof of work: Zakura sets `with_disable_pow(true)` (`zakura-network/src/parameters/testnet.rs` L1424–1425 at `ebda15e`); live headers are 177 B with all-zero 36-byte solutions and nonces and synthetic 2011-epoch timestamps (issue log #8 in [AGENTS.md](../../AGENTS.md)). The `regtest-no-pow` flag exists only for this.
- ZIP 307 light wallets check only `prevHash`, and a malicious server can show fake payments ([Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)). This is the failure the chain-state checks above avoid.
- Superseded: the old report's "proof a nullifier was never spent" and its "signed hot list of double-spending keys" ([reconciliation §5](../../archive/reports/Design%20review%20reconciliation.md)).

### Double-spend context

- No published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. Layer status and staging are in [double-spend.md](double-spend.md).
- After any uplink, the payee checks the bundle's conflict notices (and, on request, the full nullifier list) against its open payments' nullifiers. A notice that verifies (T′ bytes, txid branch to a verified header) with one of them in a different txid moves the state to conflicted.

## Open

- [unverified] A tx proven against a fabricated anchor is rejected by check 5 (open check 3). Test: build against a made-up tree and feed it to the payee.
- [unverified] A deliberate conflicting spend ends as conflicted, not confirmed (open check 4). Test: on `ths`, mine a second tx that spends the same note.
- [unverified] Payee restarts between receipt and forwarding; the gateway appears later and submits (open check 6).
- Superseded (25 October 2026): the earlier open item "have the payee trust the gateway's bundle in MVP v1". The payee never trusts the bundle; the chain-state checks above are the design and the build order inside v1 is in [open-work.md](../open-work.md).
- Initial values, not yet measured: recent-anchor set = roots from the last 100 blocks; expiry margin = 20 blocks ([pending-tier.md](pending-tier.md)). They apply to mainnet as well until measurement (anchor age distribution, uplink interval) replaces them.
- [unverified] Equihash (200,9) verify time on Mac and iPhone (96 µs/header on an orb Xeon); difficulty-floor pinning and refresh ([gateway.md](gateway.md#open)).
- [unverified] Mainnet MMR path and txid-branch sizes; only Regtest is measured (1,109–2,778 B per anchor path).
- [unverified] Whether `checked (single-source)` needs its own Receipt state byte or a flag bit; Receipt v1 in [pending-tier.md](pending-tier.md) has no slot for it yet.
- Pending-tier open items (key edge cases, group messages, dummy-action nullifiers, gating tests) are in [pending-tier.md §Open](pending-tier.md).
- Gating tests for the chain-state checks (merged with the list in [open-work.md](../open-work.md)): anchors just inside and just outside the 100-block window; a tampered bundle and a bundle signed by an unknown key are rejected; a txid branch verifies for a block with 3 transactions and fails for the wrong txid or position; expiry margin on both sides of the H + 20 boundary; real anchors verify against header N+1 and the tip while a flipped anchor, leaf, peak position or `authDataRoot` fails; a real mainnet header passes the Equihash/difficulty check and a corrupted one fails, with and without `regtest-no-pow`; one lying gateway of two is detected; single-source state applies the stricter cap; an anchor from before an upgrade activation is handled (needs a mainnet-like chain, issue log #1).
