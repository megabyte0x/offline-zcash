# Payee verifier and receipts

The offline payee checks the received bytes, decrypts its own output, applies the pending policy and replies with a signed receipt: "passes checks at height H; pending".
No offline check proves the inputs are unspent or that the payer will not double-spend, so a pending payment is an IOU from a known payer. It becomes final only once mined and confirmed.
The builder that produces the bytes is in [offline-builder.md](offline-builder.md). Chain bundles and inclusion proofs come from the [gateway](gateway.md).

## Decided

### Checks, in order [decided]

| # | Check | Source of truth |
|---|---|---|
| 1 | Version is v6 | tx header |
| 2 | Ironwood-only: no transparent, Sapling or Orchard components | tx |
| 3 | Branch ID equals the configured `0x77190AD9` | config ([chain.md](chain.md)) |
| 4 | Expiry is above the payee's last known height, with margin | last chain bundle |
| 5 | Anchor is in the payee's set of recent anchors from chain bundles | last chain bundle |
| 6 | Own output decrypts with the payee's ivk; recompute `cmx`; amount and memo match the invoice | payee keys |
| 7 | Proofs and signatures are valid | `BatchValidator` |
| 8 | Pending policy: floor limit and per-payer counter, kept on the payee | payee state |

Code path (functions read in source, not compiled): `Transaction::read(bytes, branch)` → `ironwood_bundle()` → `decrypt_outputs_with_keys(&[ivk])` → `signature_hash(.., SignableInput::Shielded, ..)` → `BatchValidator::new(&vk)`, `add_bundle`, `validate`.

- Checks 6–7 alone prove only that the tx is well formed against **the anchor it states**, which the payer can fabricate (gap G3, [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md)). Checks 1–5 are required.
- Receipt wording: **"passes checks at height H; pending"**, where H is the payee's last known height. Never "verified" or "funded".
- The payee saves the bytes to its own storage on receipt, because BitChat timelines are memory-only. It then forwards them to the gateway as a separate private message.
- Conflicts are settled only at mining, by nullifier. A nullifier mined in a different txid is **conflict evidence, not proof of fraud**: a re-issue and key theft look the same. Hot lists hold nullifiers only, never payer keys, and nothing is hot-listed automatically.
- Counters are local loss controls only. They cannot cap total loss across partitions: one credential shown to N disconnected payees exposes N × the limit.

### Receipt states [decided]

| State | Entered when | Old design term |
|---|---|---|
| received | bytes stored on the payee | — |
| checked | checks 1–8 pass; signed receipt "passes checks at height H; pending" | pending |
| submitted | gateway broadcast receipt for this txid arrives | pending |
| included | inclusion proof + headers show the txid in a block | pending |
| confirmed | 10+ confirmations on headers the payee trusts ([ZIP 315](https://zips.z.cash/zip-0315)) | final |
| conflicted | one of the tx's nullifiers is mined in a different txid | conflicted |
| expired | the chain passes the expiry height with the tx unmined; the payer re-issues from the same notes | — |

State is keyed by txid. A re-anchored copy has the same txid, so the auth digest is stored alongside it.

## Findings

### What offline checks establish

- Decrypt + `cmx` + signatures + Halo 2 proof prove: *if* the anchor is a real Ironwood root, then the spent notes existed at that anchor, the nullifiers are correctly derived and value balances ([zcash_ironwood_testnet.md Q4](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md)). Proofs bind the anchor as a public input ([ZIP 374](https://zips.z.cash/zip-0374)).
- They cannot prove that (a) the anchor is real, (b) the nullifier is not already on chain, or (c) no conflicting spend will be mined first. Check 5 narrows (a) to "as of the payee's last bundle". Nothing closes (c).
- Verifying takes about 4–12 ms per 2-action bundle on desktop hardware, and the verifying key is derived locally.

### What headers can prove

| Claim | Provable from headers? | Basis |
|---|---|---|
| Anchor from block N is a real root | Yes, from header N+1 or later, via an MMR path | [ZIP 221](https://zips.z.cash/zip-0221) history leaves carry Orchard roots. [ZIP 258](https://zips.z.cash/zip-0258) (Final) adds `hashEarliestIronwoodRoot` / `hashLatestIronwoodRoot`. Zakura 1.6.0 implements this (`history_tree.rs`, `zcash_history.rs`). Block N's leaf is committed in the next header's history root |
| Tx is included (its effects: outputs, nullifiers, values) | Yes: header + `hashMerkleRoot` branch (header 1,487 bytes, about 1.7 KB in total) | the Merkle tree covers the txid, which commits to the effecting data only |
| The exact bytes (anchor, proofs) were mined | Needs an additional `hashAuthDataRoot` path | v6 anchors are auth data ([ZIP 229](https://zips.z.cash/zip-0229)); `hashAuthDataRoot` is in `hashBlockCommitments` ([ZIP 244](https://zips.z.cash/zip-0244)) [unverified: not checked in source] |
| A nullifier was never spent | **No** | No nullifier-set commitment exists ([ZIP 221](https://zips.z.cash/zip-0221) L660–663) |
| The chain shown is the heaviest | **No** | Headers show work, not that a heavier chain does not exist. Cross-check bundles from ≥ 2 gateways and validate Equihash PoW |

- No light-client MMR proof code was found for Zcash anchors or inclusion ([MVP v1 design, open check 7](../../archive/reports/MVP%20v1%20design.md)).
- ZIP 307 light wallets check only `prevHash`, and a malicious server can show fake payments ([Hornby deck](https://defuse.ca/downloads/Fixing%20Privacy%20Problems%20in%20the%20Zcash%20Light%20Wallet%20Protocol.pdf)).
- Superseded: the old report's "proof a nullifier was never spent" and its "signed hot list of double-spending keys" ([reconciliation §5](../../archive/reports/Design%20review%20reconciliation.md)).

### Double-spend context

- No published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. Layer status and staging are in [double-spend.md](double-spend.md).
- After any uplink, the payee scans the bundle for the payment's nullifiers. If one appears in a different txid, the state becomes conflicted.

## Open

- [unverified] A tx proven against a fabricated anchor is rejected by check 5 (open check 3). Test: build against a made-up tree and feed it to the payee.
- [unverified] A deliberate conflicting spend ends as conflicted, not confirmed (open check 4). Test: on `ths`, mine a second tx that spends the same note.
- [unverified] Payee restarts between receipt and forwarding; the gateway appears later and submits (open check 6).
- [unverified] MMR anchor proof and auth-data inclusion proof: there is no code. Decide whether to write one or have the payee trust the gateway's bundle in MVP v1.
- [unverified] Size of the recent-anchor set and expiry margin (check 4) are not chosen.
- [unverified] Key and format for signing the receipt.
- [unverified] On Regtest, 10 confirmations come from one local miner and carry no security; the count applies to mainnet.
