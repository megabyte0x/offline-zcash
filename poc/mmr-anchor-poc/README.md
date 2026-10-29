Scratch proof of concept (not production code). Run against a live `ths` v0.4.1 chain:

The v0.7.7 app does not yet consume these proofs. It still trusts gateway chain
reports; this dated Regtest experiment establishes path construction, not
mainnet proof-of-work security. See the [planned verifier](../../docs/components/payee-verifier.md).

    cargo build --release
    ROOTREV=sapling ./target/release/mmrpoc x <anchor-block-height>   # z_gettreestate returns the Sapling root byte-reversed
    ./target/release/eqbench                                          # Equihash (200,9) verify timing, mainnet block 415000
    ./target/release/regpow $(curl ... getblockheader '["50", false]') # Regtest header Equihash (48,5) check

Result on 2026-10-25 (tip 132): 131/131 history roots matched header N+1 hashBlockCommitments;
anchors of txs bae7dc5a… (root of block 105) and 4dde00b5… (root of block 106) verified by MMR path;
proofs 1,109-2,778 B; Equihash verify 96 µs/header; Regtest headers carry no valid Equihash solution.
