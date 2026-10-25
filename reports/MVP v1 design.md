# MVP v1 design: offline Ironwood payments over a phone mesh

Status: design as agreed on 25 October 2026, before implementation. Nothing here is built yet except the local chain. Background and sources are in `Offline Zcash over Logos mesh.md` and `research_notes/`; repository rules and the thus-spoke-zakura issue log are in `AGENTS.md`.

The mesh never moves coins. It moves a finished transaction, already signed and proven, from phone to phone until a device with internet submits it. Value moves only when it is mined.

## Components

```
┌──────────────────────┐   BLE mesh   ┌──────────────┐   BLE mesh   ┌──────────────────────┐
│ Payer phone (offline)│─────────────▶│ Relay phones │─────────────▶│ Payee phone (offline)│
│  Offline builder     │   tx bytes   │ BitChat fork │   tx bytes   │  Verifier +          │
│  (Zakura Common      │◀─────────────│ TTL 7        │◀─────────────│  pending policy      │
│   2.2.0, Swift wrap) │   receipt    └──────┬───────┘   receipt    │  BitChat fork        │
│  BitChat fork +      │                     │                      └──────────────────────┘
│  outbox / resend     │                     ▼  ▲ receipts, headers, proofs
└──────────────────────┘          ┌──────────────────────┐
                                  │ Gateway device       │
                                  │ (has internet)       │
                                  └──────────┬───────────┘
                                             │ SendTransaction / sendrawtransaction
                                             ▼
                                  ┌──────────────────────┐
                                  │ Local chain (laptop) │
                                  │ thus-spoke-zakura    │
                                  │ v0.4.1, NU7 at h=1   │
                                  └──────────────────────┘
```

| Component | Job | Status |
|---|---|---|
| Local chain | Zakura node on NU7 (`0x77190AD9`), lightwalletd `127.0.0.1:9067`, RPC `127.0.0.1:18232`, funded accounts | Running; a 9,166-byte, 2-action Ironwood send with a 10,000-zat fee was mined |
| Offline builder | Builds, proves and signs a v6 Ironwood transaction with no network, with the fleet expiry and fee; outputs raw bytes | To build: Rust on Zakura Common `=2.2.0` (pins in `AGENTS.md`), own Swift wrapper |
| BitChat fork | Transaction broadcast type, receipt type, header/proof type; outbox that resends until mined or expired; longer reassembly and sync retention | To build |
| Verifier + pending policy | Decrypt own output, check proof and signatures, apply floor limit, per-payer counter and hot list, return a signed "pending" receipt | To build |
| Gateway submitter | Submits, returns a broadcast receipt, then the inclusion proof and headers | To build; the laptop in v1, or a phone through a port forward (ths binds to loopback only) |

Platform order: iOS first; a Mac is the fallback if iOS integration blocks (BitChat shares one codebase across both).

## One payment

| # | Step | Code path | Network |
|---|---|---|---|
| 0 | Setup: sync, split funds into 10+ notes, exchange BitChat keys, load fleet settings (branch ID, expiry delta, fee, limits) | lightwalletd sync | Once |
| 1 | Payee sends a ZIP 321 request | BitChat | Mesh |
| 2 | Payer reserves notes and builds offline | `Builder::new` → `add_ironwood_spend` / `add_ironwood_output` → `with_expiry_height` → `build` → `Transaction::write` | None |
| 3 | Signed broadcast, flooded by relays, kept in the outbox; carries a signed intent (nullifiers, txid, payee, amount, expiry) | BitChat fork | Mesh |
| 4 | Payee verifies and replies with a "pending" receipt | `Transaction::read` → `decrypt_outputs_with_keys` → `BatchValidator` | Mesh |
| 5 | Gateway submits; node mines | `SendTransaction` | Gateway only |
| 6 | Inclusion proof and headers return; payee marks final; on expiry the payer re-issues from the same notes | BitChat fork | Mesh |

## Fixed rules

- Branch ID, expiry delta and fee come from configuration, never hard-coded.
- Expiry is set by the builder from fleet policy. The library default is 40 blocks (`DEFAULT_TX_EXPIRY_DELTA` in Zakura 2.2.0), about 17 minutes at NU7's 25-second blocks.
- Fee: 10,000 zats for 2 Ironwood actions, matching the wallet.
- Txid is the identity of a payment. Resending the same bytes through any relay or gateway is harmless.
- Conflicts are settled only at mining, by nullifier.
- A stuck payment is re-issued from the same notes, so at most one version can be mined. Never re-pay from different notes.
- A transaction is bound to its branch ID; anything unmined at a network upgrade dies and is re-issued.

## Notes

```
 Available ──sign tx──▶ Reserved ──mined──▶ Spent
     ▲                     │
     └── expired unmined AND chain shows nullifier unspent
 Change from a mined tx ──▶ Available
 Change from an unmined tx ──▶ Pending (not spendable)
```

- Pre-split into 10+ confirmed notes while online; the number of notes caps the number of separate offline payments.
- Reserve before signing, and persist the reservation so a crash cannot free it.
- Release a reserved note only after expiry and proof from chain data that its nullifier was never spent.
- Change becomes spendable only after a sync including its parent block; unmined change cannot be spent (Zakura client rc7 refuses).
- One spending device per account. Two devices with the same key cannot see each other's reservations.

## Double spending

No offline check proves a note is unspent, and no published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. A single co-signer was rejected as a central point of failure. The design uses three layers instead (details and sources: `research_notes/Offline Zcash over Logos mesh/decentralized_double_spend.md`):

| Layer | When | Guarantee |
|---|---|---|
| 1. Quorum-certified notes: payer + any 2f+1 of 3f+1 FROST signers (e.g. any 3 of 4), each signing a nullifier for one transaction only | A quorum is reachable | Prevented if ≤ f signers misbehave |
| 2. Bonded accountable intents: signed intent with per-epoch counter, bond under a committee key, paid out on a fraud proof; floor limit and open intents ≤ bond ÷ k | No quorum reachable | Deterred and compensated |
| 3. Gossip of intents and fraud proofs; hot list | Always | Detected quickly |

Staging:

- **MVP v1:** pending tier (floor limit, per-payer counter), signed intents, fraud-proof detection at the gateway (nullifier mined in a different txid), hot-list gossip. Payees accept small amounts from known payers; final only when mined.
- **Next:** bond and counters (layer 2), and a 4-device quorum (layer 1).
- **Logos phase:** basecamp committees; layer 1 by default whenever a quorum is reachable.

## Stages

| Stage | Network | Change |
|---|---|---|
| MVP v1 | Local NU7 chain, ths v0.4.1 | Everything above |
| MVP v2 | Mainnet; NU7 from height 3,543,405 (~5 Nov 2026) | Same branch ID; Zakura Common 4.x or custom mainnet params; gateway over Tor; real ZEC, each send approved |
| Logos phase | Mainnet | Basecamps and Logos Delivery replace phones and BitChat; logos-co Zebra module as gateway; quorum committees |

## Open checks

1. Build one offline Ironwood transaction with Zakura 2.2.0 on the local chain and verify it on a separate payee process.
2. frost-tools bytes (sighash, α, signature) interoperate with a Zakura 2.2.0 transaction.
3. Offline payee check of committee authorization (`rk = ak + α·G`) and what revealing α leaks.
4. Whether Zakura enforces `lock_time` on fully shielded v6 transactions (bond refund).
5. Fraud-proof soundness from light-client data.
6. Phone proving time; BitChat delivery time for 9,166 bytes over 1 and 3 hops.
