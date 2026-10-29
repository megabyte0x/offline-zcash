# Payee verifier and receipts

The payee authenticates the received bytes, decrypts its own output and checks the
transaction before saving it. **Current implementation: v0.7.7, test coins only.**
No offline check proves unspent inputs or prevents a payer double-spend. A
pending receipt is an IOU, and ten confirmations are not finality.

## Built

- `verify.rs` checks v6, configured branch, Ironwood-only components, the
  action-count fee, proofs/signatures, decrypted amount, expiry, local anchor
  window and conflicts. Anchors come from locally scanned **gateway-reported**
  chain data, not PoW/MMR proofs.
- Only known invited payers can receive pending acceptance under local caps.
  Open/unknown payers wait for gateway-reported inclusion; the app does not apply
  the pending floor to that non-pending route. It does not yet implement invoice
  matching, binary Receipt v1, device-keystore enrollment or shared nullifiers.
- A catching-up recipient saves authenticated envelopes in a durable queue
  (32 entries / 4 MiB) and rechecks on chain progress. v0.7.6 rechecks their saved
  statement against the original mesh sender even after it disappears from
  discovery or the app restarts. A queued envelope is not acceptance.
- Verified outputs persist as incoming, never spendable until a scan supplies
  positions/witnesses. An exact already-scanned transaction reconciles even
  after its old anchor root is pruned. Refusal cannot cancel independently
  submitted bytes.

The sections below specify the **unbuilt mainnet chain-proof and receipt design**,
which treats the [gateway](gateway.md) as untrusted (ADV-G). Current gateway
signatures authenticate a report, not chain truth. Builder: [offline-builder.md](offline-builder.md).

## Decided

### Checks, in order [decided]

| # | Check | Source of truth |
|---|---|---|
| 1 | Version is v6 | tx header |
| 2 | Ironwood-only: no transparent, Sapling or Orchard components | tx |
| 3 | Branch ID equals the configured `0x77190AD9` | config ([chain.md](chain.md)) |
| 4 | Remaining expiry checked against verified height; initial H+20 is provisional/insufficient for the allowed stale view; replaced by the lag + uplink + confirmations + margin budget (#3 proposal 6, decided 26 Oct) | last verified chain bundle; [expiry-reissue](expiry-reissue.md) |
| 5 | For **new payments**, anchor in the provisional recent set, proved by MMR path to verified header. This is acceptance policy, not consensus age validity; historical-root renewal treatment is proposed (#6) | last verified chain bundle |
| 6 | Own output decrypts with the payee's ivk; recompute `cmx`; amount and memo (invoice nonce) match the invoice | payee keys |
| 7 | Proofs and signatures are valid | `BatchValidator` |
| 8 | Fee: `valueBalanceIronwood` equals the fleet fee for the action count (10,000 zats for 2 actions) | config |
| 9 | PaymentStatement: enrolled payer whose Noise peer sent it; `txid` and `auth_digest` recomputed from the bytes; `amount_zat`, `expiry_height`, `invoice_hash` match; `seq` above the last from this payer; re-issue rules if flagged | [pending-tier.md](pending-tier.md) |
| 10 | No nullifier of the tx matches a pending share from another payee with a different txid | nullifier index |
| 11 | Pending policy: floor, per-payer count and value caps, payee total, stale-view limit, payer not blocked | payee state |

Code path for the transaction checks (used by the app; the additional chain-proof and invoice checks above are unbuilt): `Transaction::read(bytes, branch)` → `ironwood_bundle()` → `decrypt_outputs_with_keys(&[ivk])` → `signature_hash(.., SignableInput::Shielded, ..)` → `BatchValidator::new(&vk)`, `add_bundle`, `validate`. Planned formats, reason codes and policy values for 8–11 are in [pending-tier.md](pending-tier.md).

- Checks 6–7 alone prove only that the tx is well formed against **the anchor it states**, which the payer can fabricate (gap G3, [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md)). Checks 1–5 are required.
- **Escrow tier (v1.1):** when the statement's tier flag is set, the payee also checks that each spend's `rk` equals the certified group key randomized by the α the payer sent privately ([double-spend.md](double-spend.md)).
- Receipt: the 260-byte **Receipt v1** in [pending-tier.md](pending-tier.md), signed by the payee's device-bound payment key, sent only after the payee's store is fsynced. Its `checked` state renders as **"passes checks at height H; pending"**, where H is the payee's last known height. Never "verified" or "funded".
- The payee saves the bytes to its own storage on receipt, because BitChat timelines are memory-only. It then forwards them to the gateway as a separate private message.
- After check 7 passes, the payee posts the tx's nullifiers to the fleet-payee group and, if another payee is connected, waits 5 s before signing the receipt ([pending-tier.md](pending-tier.md)).
- Conflicts are settled only at mining, by nullifier. A mined **non-family** transaction spending a payment nullifier is conflict evidence, not proof of fraud: key theft and a restored seed look the same. An authorized same-obligation replacement is settlement, not conflict; family rules decided 26 Oct ([proposal #2](expiry-reissue.md#proposals-and-owner-decisions-26-october)). With the PaymentStatement, unpaid non-family conflict becomes a debt claim under the fleet agreement. Hot lists hold nullifiers only, never payer keys; nothing is hot-listed automatically.
- Counters are local loss controls only. They cannot cap total loss across partitions: one credential shown to N disconnected payees exposes N × the limit; nullifier sharing lowers N from payees to partitions.

### Chain-state checks, no trust in the gateway [decided]

Decided 25 October 2026 (problem #2 in [open-work.md](../open-work.md)). The gateway D is untrusted on every network (ADV-G). Each belief the payee holds about the chain is checked as follows; the local chain is a harness and never a reason to skip a row.

| Payee's belief | Check, with no trust in D | Mainnet cost |
|---|---|---|
| Tip is height H, hash X | Header chain from a pinned setup checkpoint: Equihash (200,9) and difficulty against a pinned floor, parent links. A full day of headers is ≈ 5.6 MB over BLE, so the payee receives **sampled headers** (FlyClient-style: tip, random work-weighted samples with ZIP 221 MMR paths, last few contiguous), owner decision 26 Oct, protocol and security bound not yet specified; where two gateways are reachable both tips must agree | Equihash verify 96 µs/header on an orb Xeon, ≈ 0.33 s per day of headers [unverified on Mac/iPhone] |
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

### Receipt states [decided base states; expiry/family corrections proposed]

| State | Entered when | Old design term |
|---|---|---|
| received | bytes stored on the payee | — |
| checked | checks 1–11 pass; signed receipt "passes checks at height H; pending" | pending |
| checked (single-source) | as `checked`, but H and the anchor set come from one verified gateway only; the stricter ⌈C/2⌉ cap applies until a second gateway's tip agrees | — |
| submitted | gateway broadcast receipt for this txid arrives | pending |
| included | txid Merkle branch to a PoW-verified header under the tip shows the txid in a block | pending |
| confirmed | Provisional **low-value demo policy**: 10+ confirmations, not universal finality. ZIP 315 concerns output spendability; Kraken listed 50 ([prior art](../prior-art.md)). Harness count carries no PoW security | final |
| conflicted | a **non-family** transaction mines a payment nullifier; authorized replacement inclusion settles the same obligation (family rules pending) | conflicted |
| expired-unresolved | verified selected-chain tip reaches E with no inclusion **known**, not proof none occurred; retain debt; A re-issues. Proposal #5, decided 26 Oct; state value 6 | — |
| rejected | a check fails; the receipt carries the reason code ([pending-tier.md](pending-tier.md)) | — |

Store exact variants by `(txid, auth_digest)`: re-anchoring keeps txid but changes auth digest. #2 (decided 26 Oct) groups all invoice versions into one family, counted once; any version's inclusion settles it; reorgs reopen exposure. Not yet implemented. Exposure resets at `confirmed` (10) on harness and mainnet. An expired unpaid obligation stays counted; valid existing-family repair/evidence bypass follows #6.

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
- Regtest blocks carry no proof of work: Zakura sets `with_disable_pow(true)` (`zakura-network/src/parameters/testnet.rs` L1424–1425 at `ebda15e`); live headers are 177 B with all-zero 36-byte solutions and nonces and synthetic 2011-epoch timestamps (issue log #8 in [AGENTS.md](../../AGENTS.md)). The proposed `regtest-no-pow` flag is reserved for this; the production header validator is not built.
- ZIP 307 light wallets check only `prevHash`, and a malicious server can show fake payments ([Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)). This is the failure the chain-state checks above avoid.
- Superseded: the old report's "proof a nullifier was never spent" and its "signed hot list of double-spending keys" ([reconciliation §5](../../archive/reports/Design%20review%20reconciliation.md)).

### Double-spend context

- No published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. Layer status and staging are in [double-spend.md](double-spend.md).
- After any uplink, verify notices (T′ bytes plus txid branch to verified header) against open nullifiers. A **non-family** spend means conflict; an authorized replacement settles the same obligation. Family matching and orphaned-inclusion accounting await proposal #2/#5; txid inequality alone cannot decide this.

## Open

- [unverified] A tx proven against a fabricated anchor is rejected by check 5 (open check 3). Test: build against a made-up tree and feed it to the payee.
- [unverified] A deliberate conflicting spend ends as conflicted, not confirmed (open check 4). Test: on `ths`, mine a second tx that spends the same note.
- [unverified] Payee restarts between receipt and forwarding; the gateway appears later and submits (open check 6).
- Design decision (25 October 2026): complete MVP v1 must not trust the gateway's bundle. The current testnet app explicitly does trust gateway chain reports; the chain-state checks above and [build order](../open-work.md) remain unfinished.
- Provisional values: recent-anchor set = last **100 blocks for new acceptance**, not consensus; expiry margin = **20 blocks**, inadequate against allowed thirty-minute (~72-target-block) view lag. Keep them visible, not as safety bounds; the expiry budget is [proposal #6](expiry-reissue.md#proposals-and-owner-decisions-26-october), decided 26 Oct; numbers stay provisional. Thirty-minute freshness and ten confirmations also remain provisional with separate risk models.
- [unverified] Equihash (200,9) verify time on Mac and iPhone (96 µs/header on an orb Xeon); difficulty-floor pinning and refresh ([gateway.md](gateway.md#open)).
- [unverified] Mainnet MMR path and txid-branch sizes; only Regtest is measured (1,109–2,778 B per anchor path).
- Resolved 26 Oct: `checked (single-source)` is bit 0x80 of the proposed Receipt state byte, not a new state value; the layout in [pending-tier.md](pending-tier.md) records it. Binary Receipt v1 itself is unbuilt.
- Pending-tier open items (key edge cases, group messages, dummy-action nullifiers, gating tests) are in [pending-tier.md §Open](pending-tier.md).
- Gating tests for the chain-state checks (merged in [open-work.md](../open-work.md)): new-payment anchors inside/outside the provisional 100-block window; historical-root renewal separately; tampered/unknown-signature bundle refusal; 3-tx inclusion branch and wrong txid/position; expiry on both sides of the **owner-approved lag/uplink/confirmation budget** (H+20 remains only the initial provisional boundary); flipped anchor/leaf/peak/`authDataRoot`; real/corrupted mainnet headers with PoW flag off and harness headers with flag on; disagreeing gateways; single-source cap; pre-activation anchor history. [Ten problem #3 gates](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research) add family, crash, expiry, reorg and activation cases.
