# Offline builder

The payer's builder creates, proves and signs a v6 Ironwood-only transaction with no network. It takes the fleet expiry, fee and branch ID from configuration and outputs the raw bytes.
It is Rust on Zakura Common `=2.2.0` (the stack `ths` v0.4.1 uses) behind our own mobile wrapper (Swift for iOS or Kotlin/JNI for Android; the first phone platform is not decided). The app and wrapper are not built. Scratch research compiled the direct builder and mined ordinary-signature transactions; FROST PCZT variants validated on a synthetic tree, not mined ([expiry-reissue.md](expiry-reissue.md)). The wallet-DB path remains unverified.
Chain and protocol constants are in [chain.md](chain.md). Checking the result on the payee side is in [payee-verifier.md](payee-verifier.md).

## Decided

### Crate pins [decided]

The 2.2.0, 3.x and 4.x type families cannot be mixed in one binary, so pin every crate exactly. `zakura-client-*` rc7 asks for `^2.0.0` and would otherwise float.

| Crate | Version | Features |
|---|---|---|
| `zakura-protocol` | `=2.2.0` | `local-consensus` |
| `zakura-primitives` | `=2.2.0` | `circuits`, `std`, `multicore` |
| `zakura-orchard` | `=2.2.0` | `circuit` |
| `zakura-keys` | `=2.2.0` | |
| `zakura-proofs` | `=2.2.0` | |
| `zakura-pczt` (optional) | `=0.1.0-rc4` | |
| `zakura-client-backend`, `zakura-client-sqlite` (optional, synced wallet DB) | `=0.1.0-rc7` | |

Import under the upstream names, as `ths` does, for example `zcash_primitives = { package = "zakura-primitives", version = "=2.2.0" }`.

### Function sequence [decided as the plan; direct path exercised in scratch research]

1. **Network:** `LocalNetwork { .., nu6_3: Some(1), nu7: Some(1) }` to match the `ths` chain. `BranchId::for_height` then selects NU7.
2. **Direct path:** `Builder::new(net, target_height, BuildConfig::Standard { ironwood_anchor: Some(anchor), .. })` → `add_ironwood_spend(fvk, note, merkle_path)` (notes must be `NoteVersion::V3`) → `add_ironwood_output(ovk, addr, value, memo)` → `with_expiry_height(h)` → `build(..)` → `BuildResult::transaction()` → `Transaction::write`.
3. **Wallet-DB path:** `propose_transfer` → `create_proposed_transactions(.., expiry_height: Some(h))` → `get_transaction(txid)` → `write`.

### Rules [decided]

- Expiry is always set explicitly from fleet policy, either `with_expiry_height` or `Some(h)`. Never rely on the library default of 40 blocks (issue log #6 in [AGENTS.md](../../AGENTS.md); [chain.md](chain.md#expiry-zip-203-zip-218)).
- Fee: 10,000 zats for 2 actions, from configuration. Output is padded to 2 actions.
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
- **No Swift or uniffi bindings exist.** The iOS wrapper is ours.
- **Inputs needed offline:** spend authorizing key, full viewing key, each spent note's plaintext and position, a 32-level Merkle path (about 1 KB) to any earlier final Ironwood root plus that anchor, the recipient receiver (from a ZIP 321 URI), a current-height estimate, branch ID, fee and a fresh OS RNG ([zcash_ironwood_testnet.md Q3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md)).
- **Proving key:** Halo 2 at K=11, generated locally with nothing to download. Key preparation takes about 25–29 MiB (zakura-orchard 2.2.0 `benches/README.md`).
- **Measured Ironwood research:** two-action proof median 729 ms, direct re-sign 1.254 ms on a two-vCPU orb Xeon, not phone performance. Expiry renewal retained identical actions/proof; retained pre-IO PCZT FROST variants validated on a synthetic tree. Fresh construction randomness changes dummy nullifiers; retained state can keep them fixed. No production crash-safe snapshot exists ([evidence, samples and source lines](expiry-reissue.md#expiry-renewal-is-not-re-anchoring)).
- **Adjacent Orchard benchmark, not Ironwood:** 2 actions about 0.65–1.35 s on M4 Max, verification 4–12 ms ([dashpay/orchard PR #13](https://github.com/dashpay/orchard/pull/13)). Earlier iPhone/vendor timing claims are not a verified budget for our circuit/stack. Target-phone time/RAM/battery remain open ([prior art](../prior-art.md)).
- **Anchor choice leaks:** an old anchor reveals roughly when the wallet last synced. [ZIP 318](https://zips.z.cash/zip-0318) uses shared boundary heights to avoid "linking transactions via anchor selection". Stale anchor, long expiry and burst timing together fingerprint fleet transactions (gap G6, [reconciliation](../../archive/reports/Design%20review%20reconciliation.md)).
- **Logos wallet core does not fit as the builder:** it signs and broadcasts in one job, exports no raw bytes or PCZT, hard-codes expiry, and uses upstream crates ([gateway.md](gateway.md#logos-phase-facts); [logos_zcash_modules.md §5](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).
- **Hardware signers** give no double-spend protection, because the seed can be restored on a second device ([report, Ironwood section](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).

## Open

- [unverified] Production offline app plus separate payee process and wallet-DB path. Scratch direct-path compilation/mining and 9,166-byte v6 shape are established; they do not implement the end-to-end app.
- [unverified] Phone proving time and memory on the Ironwood circuit with Zakura 2.2.0 (open check 9). Test: run the `ironwood_k11_prover` bench and an iOS build.
- [unverified] Crash safety of reservations. Test: kill the app between reserve and sign, and between sign and send. The note must stay Reserved.
- [decided 26 Oct] Fleet expiry profile: shutdown, 72,576 blocks for ≤7-day uplink ([proposal #1](expiry-reissue.md#proposals-and-owner-decisions-26-october)).
- [unverified] Whether to use a shared boundary anchor height (ZIP 318) to reduce anchor fingerprinting.
- [decided, unimplemented] Family protocol and A-initiated repair; encrypted pre-IO snapshot deferred (minimal renewal re-proves from the same notes). Ordinary-signature renewal mined; real-funded FROST renewal, persistence and family-wide invariants remain [gating tests](expiry-reissue.md#ten-gating-tests-not-completed-by-the-research).
- [unverified] Whether the rc7 wallet DB can import a FROST-derived UFVK and produce a PCZT; otherwise escrow notes use the direct `Builder`.
- v1.1: the co-signer group component (weighted DKG, redacted PCZT, FROST signing, lock store), about 400–700 lines of Rust plus Swift bindings (estimate from the 2-of-2 design); details and gating tests in [double-spend.md](double-spend.md). The earlier "port frost-tools, 300–500 lines" item is superseded.
