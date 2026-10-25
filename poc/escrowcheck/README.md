# escrowcheck

Throwaway feasibility check for the bilateral 2-of-2 FROST escrow ([docs/components/double-spend.md](../../docs/components/double-spend.md)). One binary, `src/main.rs`, that signs an Ironwood v6 spend with a rerandomized FROST group key on the Zakura Common 2.2.0 stack.

## Crate pins

| Dependency | Package | Version | Features |
|---|---|---|---|
| `orchard` | `zakura-orchard` | `=2.2.0` | `unstable-frost`, `unstable-voting-circuits` |
| `pczt` | `zakura-pczt` | `=0.1.0-rc4` | `signer`, `orchard`, `io-finalizer`, `prover`, `tx-extractor`, `zcp-builder` |
| `zcash_primitives` | `zakura-primitives` | `=2.2.0` | |
| `zcash_protocol` | `zakura-protocol` | `=2.2.0` | `local-consensus` |
| `reddsa` | `zakura-reddsa` | `=2.2.0` | `frost` (brings `frost-rerandomized` / `frost-core` 3.0.0) |
| `frost-rerandomized` | | `3.0.0` | |
| `pasta_curves` | `zakura-pasta-curves` | `=2.2.0` | |

Others: `ff` 0.14, `rand` 0.10 (`sys_rng`), `rand_core` 0.6 (`getrandom`, for FROST), `zcash_note_encryption` 0.4.2, `incrementalmerkletree` 0.8.2, `shardtree` 0.7, `blake2b_simd` 1, `blake3` 1, `hex` 0.4. `Cargo.lock` holds the resolved versions.

## unstable-frost workaround

`zakura-orchard` 2.2.0 declares `unstable-frost = []` (Cargo.toml L93) but its `#[cfg_attr(feature = "unstable-frost", visibility::make(pub))]` attributes (`src/keys.rs` L193, L205) need the optional `visibility` crate, which only `unstable-voting-circuits` enables. With `unstable-frost` alone, `cargo check` fails with E0433 "cannot find module or crate `visibility`". Enabling `unstable-voting-circuits` as well fixes it. `unstable-frost` is only used here for `SpendValidatingKey::from_bytes`/`randomize` checks; the escrow FVK itself is built with the always-public `FullViewingKey::from_bytes`.

## Run

```
cargo run --release
```

Takes about 1.5 s after the build (the Ironwood proving key is generated locally). Prints the branch ID, sighash, α, nullifiers and the extracted transaction, then `OK: ...`. Any failure panics.

## What it does

1. 2-of-2 FROST key generation with a trusted dealer (`reddsa::frost::redpallas`).
2. Builds an Ironwood FVK whose `ak` is the group key; `nk` from a shared `sk`, `rivk` via the ZIP 2005 `use_qsk = true` derivation, injected through `FullViewingKey::from_bytes`.
3. Creates a V3 Ironwood note to the escrow address and decrypts it with the escrow IVK.
4. Builds a PCZT with `Builder` on a `LocalNetwork` with every upgrade through NU7 at height 1 (branch `0x77190AD9`, v6).
5. Co-signer side, from the serialized PCZT bytes and the group `ak` only: recomputes the sighash (`Signer::shielded_sighash`), reads the nullifiers, reads α via `sign_ironwood_with` and checks `rk == ak.randomize(α)`.
6. Rerandomized FROST signing over (sighash, α) and aggregation.
7. Applies the aggregate with `Signer::apply_ironwood_signature`, proves, and extracts with `TransactionExtractor::extract`, which batch-validates the proof and all signatures. Checks the txid equals the sighash and the branch is NU7.

Result seen on 2026-10-25: v6, branch `0x77190ad9`, 9,166 bytes.

## What it does not prove

- The note sits in a **one-leaf local tree**; the anchor is not a real chain root. The transaction was **not mined or broadcast on `ths`**; consensus acceptance by a Zakura node is untested.
- Trusted-dealer keygen, not DKG; both shares live in one process, so no message transport, nonce persistence or nullifier lock is exercised.
- The ZIP 2005 `rivk_ext` derivation is not checked against test vectors.
- `DeferredPcztBuilder` (sign before the anchor) is not exercised.
- No wire compatibility with frost-tools / frost-client (frost-core 2.x) is tested.
