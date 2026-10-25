# Offline builder

The payer's builder creates, proves and signs a v6 Ironwood-only transaction with no network. It takes the fleet expiry, fee and branch ID from configuration and outputs the raw bytes.
It is Rust on Zakura Common `=2.2.0` (the stack `ths` v0.4.1 uses) behind our own Swift wrapper for iOS. Nothing is built yet: the functions below were read in source at v2.2.0 / rc7 but have not been compiled.
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

### Function sequence [decided as the plan; not compiled]

1. **Network:** `LocalNetwork { .., nu6_3: Some(1), nu7: Some(1) }` to match the `ths` chain. `BranchId::for_height` then selects NU7.
2. **Direct path:** `Builder::new(net, target_height, BuildConfig::Standard { ironwood_anchor: Some(anchor), .. })` → `add_ironwood_spend(fvk, note, merkle_path)` (notes must be `NoteVersion::V3`) → `add_ironwood_output(ovk, addr, value, memo)` → `with_expiry_height(h)` → `build(..)` → `BuildResult::transaction()` → `Transaction::write`.
3. **Wallet-DB path:** `propose_transfer` → `create_proposed_transactions(.., expiry_height: Some(h))` → `get_transaction(txid)` → `write`.

### Rules [decided]

- Expiry is always set explicitly from fleet policy, either `with_expiry_height` or `Some(h)`. Never rely on the library default of 40 blocks (issue log #6 in [AGENTS.md](../../AGENTS.md); [chain.md](chain.md#expiry-zip-203-zip-218)).
- Fee: 10,000 zats for 2 actions, from configuration. Output is padded to 2 actions.
- Proving stays on the payer's device, never on a gateway or shared prover. Delegating proving "reveals the private data of the notes being spent or created" ([ZIP 374](https://zips.z.cash/zip-0374)).
- The output is the raw tx bytes plus the txid. The payment details and the payer's accountability signature go only to the payee, over the Noise private channel ([MVP v1 design, One payment](../../archive/reports/MVP%20v1%20design.md)).
- A stuck payment is re-issued from **the same notes**, so at most one version can be mined. Never re-pay from different notes.
- One spending device per account. Two devices with the same key cannot see each other's reservations.

### Note lifecycle [decided]

```
 Available ──sign tx──▶ Reserved ──mined──▶ Spent
     ▲                     │
     └── expired unmined AND a trusted sync past expiry shows the nullifier unspent
 Change from a mined tx   ──▶ Available
 Change from an unmined tx ──▶ Pending (not spendable)
```

- While online, pre-split into 10+ confirmed notes. The number of notes caps the number of separate offline payments.
- Reserve before signing, and persist the reservation so a crash cannot free it.
- Release a reserved note only after expiry, and only when a sync the device trusts, past the expiry height, shows its nullifier unspent. No chain bundle can prove non-spend (see [payee-verifier.md](payee-verifier.md#what-headers-can-prove)).
- Change becomes spendable only after a sync that includes its parent block.

## Findings

- **Unmined change is unspendable:** `zakura-client-backend` rc7 `wallet.rs` L1414–1418 refuses it. Chained offline payments wait for each parent to be mined. The protocol itself allows signing such a spend, but it cannot be proven until a witness exists ([ZIP 374](https://zips.z.cash/zip-0374)).
- **`DeferredPcztBuilder`** (sign before choosing the anchor) exists and accepts v6 only. PCZT v2 lets signers sign while anchors, witnesses and proofs are absent. Setting or replacing an anchor clears the proofs, and expiry and values stay fixed after signing ([ZIP 374](https://zips.z.cash/zip-0374)).
- **No Swift or uniffi bindings exist.** The iOS wrapper is ours.
- **Inputs needed offline:** spend authorizing key, full viewing key, each spent note's plaintext and position, a 32-level Merkle path (about 1 KB) to any earlier final Ironwood root plus that anchor, the recipient receiver (from a ZIP 321 URI), a current-height estimate, branch ID, fee and a fresh OS RNG ([zcash_ironwood_testnet.md Q3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md)).
- **Proving key:** Halo 2 at K=11, generated locally with nothing to download. Key preparation takes about 25–29 MiB (zakura-orchard 2.2.0 `benches/README.md`).
- **Proving time (Orchard circuit, not Ironwood's):** 2 actions take about 0.65–1.35 s on an M4 Max with the stock prover, and verify in about 4–12 ms ([dashpay/orchard PR #13](https://github.com/dashpay/orchard/pull/13)). iPhones take more than 3 s on today's stack; a vendor claims about 200 ms on the new Zakura stack ([CoinInsider](https://www.coininsider.org/news/zcash-proving-times-reach-around-200ms/), secondary).
- **Anchor choice leaks:** an old anchor reveals roughly when the wallet last synced. [ZIP 318](https://zips.z.cash/zip-0318) uses shared boundary heights to avoid "linking transactions via anchor selection". Stale anchor, long expiry and burst timing together fingerprint fleet transactions (gap G6, [reconciliation](../../archive/reports/Design%20review%20reconciliation.md)).
- **Logos wallet core does not fit as the builder:** it signs and broadcasts in one job, exports no raw bytes or PCZT, hard-codes expiry, and uses upstream crates ([gateway.md](gateway.md#logos-phase-facts); [logos_zcash_modules.md §5](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).
- **Hardware signers** give no double-spend protection, because the seed can be restored on a second device ([report, Ironwood section](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).

## Open

- [unverified] The sequence compiles and produces a tx accepted by the `ths` node (open check 1, [MVP v1 design](../../archive/reports/MVP%20v1%20design.md)). Test: build offline on the local chain, verify in a separate payee process, mine it.
- [unverified] The tx is byte-compatible with what `ths wallet send` produces (9,166 bytes, `06000080`, branch `d90a1977`). Test: compare the header, size and action count.
- [unverified] Phone proving time and memory on the Ironwood circuit with Zakura 2.2.0 (open check 9). Test: run the `ironwood_k11_prover` bench and an iOS build.
- [unverified] Crash safety of reservations. Test: kill the app between reserve and sign, and between sign and send. The note must stay Reserved.
- [unverified] Fleet expiry delta value. It is set from gateway cadence at the 25 s target; the old report's example is 10,368 blocks. It is not chosen for MVP v1.
- [unverified] Whether to use a shared boundary anchor height (ZIP 318) to reduce anchor fingerprinting.
- [unverified] Re-issue flow after expiry: same notes, new expiry, new txid. Test: let a tx expire on `ths`, re-issue, and confirm the old bytes are rejected.
- Later: 2-of-2 FROST escrow needs frost-tools' `zcash-sign` ported to zakura-orchard 2.2.0 (`unstable-frost`) and zakura-pczt rc4 (`sign_ironwood_with`), about 300–500 lines ([reconciliation §3](../../archive/reports/Design%20review%20reconciliation.md)).
