# Offline builder

The payer's builder creates, proves and signs a v6 Ironwood-only transaction with no network. It takes the fleet expiry, fee and branch ID from configuration and outputs the raw bytes.
The application uses Zakura Common `=4.0.0`, with UniFFI/Kotlin Android bindings and one shared Logos Basecamp engine. `ths` v0.4.1 still uses Common 2.2.0; compatible NU7 transaction bytes cross that process boundary, not Rust types. iOS and the upstream wallet-DB path remain unbuilt. Historical scratch research below uses its stated older versions.
Chain and protocol constants are in [chain.md](chain.md). Checking the result on the payee side is in [payee-verifier.md](payee-verifier.md).

**Current implementation: v0.7.7.** `builder.rs` and the separate payee path
have end-to-end Regtest and funded public-testnet evidence. The `Decided`
sections also retain future binary statements, keystore keys and renewal rules;
these are not the application's current JSON statement / sealed `ozm1` format.

## v0.7.0 testnet note plan

Choose fleet `network: "testnet"`. Fresh keys record their creation time; each
wallet chooses its own scan start automatically. Online creation uses tip−100;
offline creation interpolates creation−one day between the release checkpoint
(testnet 4,550,000 at Unix time 1,793,168,502) and the first-contact tip's height/time,
clamped to that checkpoint and the tip. Regtest uses RPC tip−100, at least 1.
Existing wallets keep their birthday; a fleet birthday is only a floor for new
wallets. `--birthday` remains a CLI bench override, not a user setup option.
A fresh wallet seeds its Ironwood ShardTree from
`GetTreeState(birthday - 1)`; compact sync discovers notes locally. No viewing key
goes to the gateway. The frontier size must match that block's Ironwood tree-size
metadata. This checks consistency, not a history-leaf proof.
Verified mesh receipts persist decrypted outputs immediately as incoming notes,
separate from the spendable inventory. The UI shows their pending incoming value.
Scanning merges by commitment and supplies the chain position and witness; only
then can the note be selected for spending. Included/mined gateway reports start
sync immediately; expired/conflicting reports remove unmined incoming outputs.
The wallet persists a pruned tree and 200 checkpoints/roots;
reorgs rewind notes, spends and witnesses to a retained checkpoint. Deeper forks
use the persisted 1,000-block hash log to find a conservative common ancestor P,
then reseed at min(first_note_height, P+1)−1, floored at birthday−1. First-note
height includes spent notes and mesh anchor+1 and survives reset. An empty wallet
uses P+1; without a common historical sample the fallback is its own birthday,
not the fleet default. Old wallets build their sparse log as they next scan.
Sync snapshots persist every five seconds and at completion/stop; payment and
receipt transmission always forces persistence first. A crash may replay batches,
but cannot leave the durable cursor ahead of the durable notes/tree.

Obtain TAZ at [Valar](https://faucet.testnet.valargroup.dev), advertised as 0.125 TAZ
per address per 24 hours with Ironwood `utest1` support. **Our 27 October claim
failed**: HTTP 202 queued claim `4b03979781900d2c7a46ada8ffbc04e2` later returned
`failed`, `the wallet cannot pay this address`, broadcast attempts `0/3`.
The zecpages fallback failed with `curl: (28) Connection timed out after 30001 milliseconds`.
Neither is evidence of a funded wallet; do not repeatedly submit claims to bypass
the faucet's rate limit. There is no gateway faucet on testnet.

**Pre-tag scan, 27 October 22:22 UTC:** the orb's internet-connected gateway
served public compact data to payer A on an internal Docker network without an
internet route (payee B is isolated there too). A scanned birthday 4,548,674
through **4,548,872**, finding **zero notes / zero received or spendable zat**.
This was local viewing-key scanning of compact actions, not a GitHub or
transparent-balance query. A's keys and wallet remain in the orb. The owner
approved release with the funded split → offline payment → ten-confirmation
public run and independent public confirmation **unverified**.

**Subsequent funded run, 28 October:** v0.7.5 Linux Basecamp automatically split
owner-provided TAZ, paid offline from a mined split note, recovered after restart
and reconnect, and showed ten gateway-reported confirmations on both wallets.
See [exact cases and limitations](../open-work.md#built). This closes the earlier
unfunded application-flow gap, not independent chain verification or phone tests.

**v0.7.5 automatic preparation:** with fewer than four spendable notes and a
reachable gateway, try 8, then 4, then 2 smaller notes, choosing the largest
count whose action-count fee is at most 2% of the funding note. About half its
value becomes equal notes; the remainder is change. Other mined notes remain
usable while this input is locked. A wallet funded by one note cannot retain an
immediately spendable remainder while spending that same note: its change must
mine and be scanned first. This is explicit in the UI, never unmined spending.
Each external funding transaction is attempted only once, durably before
proving. Self-split outputs and our own change never trigger another split;
failure/restart cannot create a new-transaction fee loop. Existing submitted
bytes still use the normal retry policy. Small balances stay untouched. The
wallet/web show progress; manual preparation is an advanced option.

**Advanced Prepare for offline** builds a self-payment from one mined note into N equal
notes and optional change (for example 10 × 0.0101 TAZ). Its fee is the larger of
the fleet floor (10,000 zat) and ZIP 317's fee for the padded action count;
change can add an action. Builder and verifier apply the same policy. Wait for
`mined` (10 confirmations, gateway-reported) **and a completed wallet sync** before
disconnecting. CLI `prepare --notes 10 --amount 0.0101` waits for the mined report;
web, Basecamp and Android show the split in payment history. Never spend unmined
change. One note limits how many independent offline payments can be made.

**Local-chain check, 27 October 2026:** Prepare split a 12,500,000-zat note into
10 × 1,010,000-zat notes and 2,345,000-zat change, with **11 actions and a
55,000-zat fee**. `ths` v0.4.1 mined
`20831a69d1475a45d39dcc1bc5c05659fdf694f86da146b808a806c95b6edb47`
at height 119; verbose RPC independently reports 11 actions and a 0.00055 value
balance. This proves the Common 4 split on the local chain, **not public testnet**.

Reservations persist before broadcast, survive rollback/restart, and release only
when the **locally scanned** tip exceeds the signed transaction's inclusive expiry.
Payments, Max and Prepare remain available during catch-up. Only batch application
and installation share the proving/reservation lock; network downloads do not.
The UI labels the balance with its scanned height, and installation merges any
reservations made while that batch downloaded before persisting it.
A gateway's expiry result alone cannot unlock them. This is not cancellation of
an accepted debt; the family/renewal protocol below remains deferred. A refusal
means **refused, may still mine until expiry**, because the same bytes may already
have been submitted.

## Decided

### Crate pins [decided]

The 2.x, 3.x and 4.x type families cannot be mixed in one binary; pin every crate exactly.

| Crate | Version | Features |
|---|---|---|
| `zakura-protocol` | `=4.0.0` | `local-consensus` |
| `zakura-primitives` | `=4.0.0` | `circuits`, `std`, `multicore` |
| `zakura-orchard` | `=4.0.0` | `circuit` |
| `zakura-keys` | `=4.0.0` | |
| `zakura-note-encryption`, `zakura-pasta-curves` | `=4.0.0` | Matching type family |
| `zakura-client-backend` | `=0.1.0-rc9.5` | Compact gRPC types and tree serialization |

Import under upstream names, for example `zcash_primitives = { package = "zakura-primitives", version = "=4.0.0" }`. `ProvingKey::build` now returns a Result. Relock the Basecamp module independently after migration. The optional PCZT family is rc9; it is not linked by the application.

### Function sequence [direct path built; wallet-DB path is research only]

1. **Network:** `LocalNetwork { .., nu6_3: Some(1), nu7: Some(1) }` to match the `ths` chain. `BranchId::for_height` then selects NU7.
2. **Direct path:** `Builder::new(net, target_height, BuildConfig::Standard { ironwood_anchor: Some(anchor), .. })` → `add_ironwood_spend(fvk, note, merkle_path)` (notes must be `NoteVersion::V3`) → `add_ironwood_output(ovk, addr, value, memo)` → `with_expiry_height(h)` → `build(..)` → `BuildResult::transaction()` → `Transaction::write`.
3. **Wallet-DB path:** `propose_transfer` → `create_proposed_transactions(.., expiry_height: Some(h))` → `get_transaction(txid)` → `write`.

### Rules [decided]

- Expiry is always set explicitly from fleet policy. Never rely on library defaults (Common 2.2: 40 blocks; Common 4 at NU7: 120).
- Fee: configured 10,000-zat floor for two actions; larger transactions pay the ZIP 317 action-count fee. Minimum two actions, not an exact two-action restriction.
- Proving stays on the payer's device, never on a gateway or shared prover. Delegating proving "reveals the private data of the notes being spent or created" ([ZIP 374](https://zips.z.cash/zip-0374)).
- The output is the raw tx bytes plus the txid and auth digest. The payer then signs the 276-byte **PaymentStatement** ([pending-tier.md](pending-tier.md)) with its device-bound Ed25519 payment key, after persisting `seq + 1`, and sends tx plus statement to the payee only, over the Noise private channel. Relays and the gateway see tx bytes only.
- **Escrow notes (v1.1)** are spent through the PCZT path instead: `DeferredPcztBuilder` or the standard builder emits a PCZT, the Redactor strips value, recipient, rseed, fvk, witness and rcv, the co-signer signs with FROST, and `Signer::apply_ironwood_signature` completes it ([double-spend.md](double-spend.md)). α is fixed in the PCZT by the payer before the co-signer commits nonces. Ordinary notes keep the direct path above.
- A stuck payment is re-issued from **the same real notes**, so at most one conflicting version can mine on **one valid chain**, not across orphaned histories. Never re-pay from unrelated notes. Canonical effects and family-wide exclusion are [decided 26 Oct](expiry-reissue.md#proposals-and-owner-decisions-26-october); adjacent shared-nullifier overlap is insufficient.
- One spending device per account. Two devices with the same key cannot see each other's reservations.

### Note lifecycle [reservation decided; family and A-initiated renewal decided 26 Oct]

```
 Available ──sign tx──▶ Reserved ──mined──▶ Spent
                         │
                         └── expiry: no automatic release; resolution policy pending
 Change from a mined tx   ──▶ Available
 Change from an unmined tx ──▶ Pending (not spendable)
```

- While online, pre-split into 10+ confirmed notes. The number of notes caps the number of separate offline payments.
- Reserve before signing, and persist the reservation so a crash cannot free it.
- Correction: expiry plus trusted unspent reconciliation does **not cancel accepted debt**, and no bundle proves non-inclusion/non-spend ([payee-verifier.md](payee-verifier.md#what-headers-can-prove)). The old expiry-release rule is insufficient. #5 (decided 26 Oct) keeps reservations through renewal until settlement or explicit resolution; not implemented.
- Change becomes spendable only after a sync that includes its parent block.

## Findings

- **Unmined change is unspendable:** `zakura-client-backend` rc7 `wallet.rs` L1414–1418 refuses it. Chained offline payments wait for each parent to be mined. The protocol itself allows signing such a spend, but it cannot be proven until a witness exists ([ZIP 374](https://zips.z.cash/zip-0374)).
- **`DeferredPcztBuilder`** (sign before choosing the anchor; zakura-primitives `builder.rs` L469, `new` L491, `add_ironwood_spend` L597) accepts v6 only. PCZT v2 permits signing without anchors/witnesses/proofs because v6 txid/sighash exclude the anchor. Re-anchoring needs fresh witnesses/proof, not new applicable signatures. The rc4 setter **rejects** an existing proof/conflicting anchor; it does not clear automatically: explicit redaction/update/re-proving is needed. Expiry changes need new signatures but can retain actions/proof ([ZIP 374](https://zips.z.cash/zip-0374); [source lines and experiments](expiry-reissue.md#expiry-renewal-is-not-re-anchoring)).
- **FROST on this stack is proven offline:** [`poc/escrowcheck`](../../poc/escrowcheck/README.md) builds a PCZT at branch 0x77190AD9 on the 2.2.0 pins, co-signs it with a rerandomized FROST 2-of-2 aggregate through `zakura-reddsa`'s `frost` feature, and extracts a valid 9,166-byte v6 transaction. frost-tools cannot be reused: upstream crates map NU7 to `0xffffffff` and cannot link against the Zakura 2.2.0 type family ([double-spend.md](double-spend.md)).
- **No upstream Swift or UniFFI bindings exist.** This repo provides UniFFI/Kotlin through `ozm-ffi`; an iOS wrapper is not built.
- **Inputs needed offline:** spend authorizing key, full viewing key, each spent note's plaintext and position, a 32-level Merkle path (about 1 KB) to any earlier final Ironwood root plus that anchor, the recipient receiver (from a ZIP 321 URI), a current-height estimate, branch ID, fee and a fresh OS RNG ([zcash_ironwood_testnet.md Q3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md)).
- **Proving key:** Halo 2 at K=11, generated locally with nothing to download. Key preparation takes about 25–29 MiB (zakura-orchard 2.2.0 `benches/README.md`).
- **Measured Ironwood research:** two-action proof median 729 ms, direct re-sign 1.254 ms on a two-vCPU orb Xeon, not phone performance. Expiry renewal retained identical actions/proof; retained pre-IO PCZT FROST variants validated on a synthetic tree. Fresh construction randomness changes dummy nullifiers; retained state can keep them fixed. No production crash-safe snapshot exists ([evidence, samples and source lines](expiry-reissue.md#expiry-renewal-is-not-re-anchoring)).
- **Adjacent Orchard benchmark, not Ironwood:** 2 actions about 0.65–1.35 s on M4 Max, verification 4–12 ms ([dashpay/orchard PR #13](https://github.com/dashpay/orchard/pull/13)). Earlier iPhone/vendor timing claims are not a verified budget for our circuit/stack. Target-phone time/RAM/battery remain open ([prior art](../prior-art.md)).
- **Anchor choice leaks:** an old anchor reveals roughly when the wallet last synced. [ZIP 318](https://zips.z.cash/zip-0318) uses shared boundary heights to avoid "linking transactions via anchor selection". Stale anchor, long expiry and burst timing together fingerprint fleet transactions (gap G6, [reconciliation](../../archive/reports/Design%20review%20reconciliation.md)).
- **Logos wallet core does not fit as the builder:** it signs and broadcasts in one job, exports no raw bytes or PCZT, hard-codes expiry, and uses upstream crates ([gateway.md](gateway.md#logos-phase-facts); [logos_zcash_modules.md §5](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).
- **Hardware signers** give no double-spend protection, because the seed can be restored on a second device ([report, Ironwood section](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).

### Notes, phone cost and bindings (#8, decided 26 October: 32-note harness inventory, foreground-only proving and co-signing on iOS)

- **Notes, not proving speed, limit offline payments.** Proposed starting inventory (Regtest, illustrative): 32 notes: 16 × 0.0501 and 8 × 0.2001 ZEC under the payer key, 4 × 0.5001 and 4 × 1.0001 under the group key. Each note includes 10,000 zats for one payment's fee. That is up to 24 pending-tier and 8 escrow one-note payments. Use the smallest sufficient note, at most 2 inputs, always 2 actions (Common 2.2.0 `orchard/src/builder.rs` L38–149). Fund, split, confirm and keep witnesses before going offline. When notes run out, stop paying; never spend unmined change. #8 research
- The 21-day expiry is carriage lifetime, not 21 days of new payments: 30-minute freshness and the 100-block anchor window still apply, and the mesh witness-update protocol is open.
- **Measured on the orb** (2-vCPU Xeon, warm, Common 2.2.0): two-action proof 675 ms (peak RSS 99.8 MiB), four-action proof 1.19 s, verify 11.5 ms. Proving key build 387 ms, verifying key build 214 ms. Projections only: Apple Silicon Mac 0.5–2 s, recent iPhone 1–5 s per proof; budget 256 MiB working memory. No Apple hardware measured.
- **Bindings:** one Rust `staticlib` plus a thin Swift wrapper. It exposes prepare/prove/finalize, verify payment and chain evidence, FROST rounds and fixed-policy PCZT redaction. Secrets, nonces and reservations stay in Rust. Stripped Linux binary 8.6 MiB. `cargo check` for iOS and macOS fails in `zakura-pasta-curves` without an Apple toolchain; it needs a Mac build.
- iOS runs co-signing in the foreground; background BLE carriage is best-effort (about 10 s per BLE wake).

## Open

- [built] Offline app and separate payee process: multi-hop Regtest benches and funded Linux Basecamp testnet flow. The upstream wallet-DB path remains unbuilt; the app uses its own persisted wallet and direct builder.
- [unverified] Phone proving time and memory on the Ironwood circuit with the current Common 4.0 stack (open check 9). Host timings are not phone measurements.
- [partial] Reservations persist before transmission and survive tested restart/reorg flows. Exhaustive process-kill injection at every reserve/sign/send boundary remains a durability gate.
- [decided 26 Oct] Fleet expiry profile: shutdown, 72,576 blocks for ≤7-day uplink ([proposal #1](expiry-reissue.md#proposals-and-owner-decisions-26-october)).
- [unverified] Whether to use a shared boundary anchor height (ZIP 318) to reduce anchor fingerprinting.
- [decided, unimplemented] Family protocol and A-initiated repair; encrypted pre-IO snapshot deferred (minimal renewal re-proves from the same notes). Ordinary-signature renewal mined; real-funded FROST renewal, persistence and family-wide invariants remain [gating tests](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research).
- [unverified] Whether the rc7 wallet DB can import a FROST-derived UFVK and produce a PCZT; otherwise escrow notes use the direct `Builder`.
- v1.1: the co-signer group component (weighted DKG, redacted PCZT, FROST signing, lock store), about 400–700 lines of Rust plus Swift bindings (estimate from the 2-of-2 design); details and gating tests in [double-spend.md](double-spend.md). The earlier "port frost-tools, 300–500 lines" item is superseded.
