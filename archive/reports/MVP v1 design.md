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

Platform order: iOS first; a Mac is the fallback if iOS integration blocks (BitChat shares one codebase across both). The three-laptop demo (payer, payee, gateway) runs on three Macs (macOS 13+) with the BitChat fork, decided 25 October 2026.

Transport rules from BitChat at `04995f68`:

- Send the transaction as a private message (base64, about 12.3 KB, under the 60,000-character limit), not as a file or public broadcast. Private messages sit in a sealed on-disk outbox and are resent until acknowledged (24 h, 8 attempts). Private file sends are not outboxed.
- The payee app saves the transaction to its own storage on receipt; BitChat keeps conversations in memory only.
- The payee forwards the bytes to the gateway as a separate private message; a packet addressed to the payee stops there.
- Use the same build configuration on all three Macs: Debug and Release use different Bluetooth service UUIDs.

## One payment

| # | Step | Code path | Network |
|---|---|---|---|
| 0 | Setup: sync, split funds into 10+ notes, exchange BitChat keys, load fleet settings (branch ID, expiry delta, fee, limits) | lightwalletd sync | Once |
| 1 | Payee sends a ZIP 321 request | BitChat | Mesh |
| 2 | Payer reserves notes and builds offline | `Builder::new` → `add_ironwood_spend` / `add_ironwood_output` → `with_expiry_height` → `build` → `Transaction::write` | None |
| 3 | Payer sends the transaction and the payment details (invoice, amount, payer's accountability signature) to the payee as a Noise-encrypted private message, kept in the outbox. Relays and the gateway receive the raw transaction bytes only, never an intent or payer identity | BitChat fork | Mesh |
| 4 | Payee checks v6, Ironwood-only, branch ID, expiry against its last known height, anchor in its set of recent anchors, its own output, proofs and signatures; replies "passes checks at height H; pending" | `Transaction::read` → `decrypt_outputs_with_keys` → `BatchValidator` | Mesh |
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
     └── expired unmined AND trusted sync past expiry shows nullifier unspent
 Change from a mined tx ──▶ Available
 Change from an unmined tx ──▶ Pending (not spendable)
```

- Pre-split into 10+ confirmed notes while online; the number of notes caps the number of separate offline payments.
- Reserve before signing, and persist the reservation so a crash cannot free it.
- Release a reserved note only after expiry, when a sync the device trusts, past the expiry height, shows its nullifier unspent. No header commits to the nullifier set (ZIP 221), so non-spend cannot be proven from a chain bundle.
- Change becomes spendable only after a sync including its parent block; unmined change cannot be spent (Zakura client rc7 refuses).
- One spending device per account. Two devices with the same key cannot see each other's reservations.

## Double spending

No offline check proves a note is unspent, and no published scheme prevents a fully disconnected payer from double-spending without trusted hardware or a co-signer. A "pending" payment is therefore an IOU from a known payer. Reviews and source checks on 25 October 2026 changed the layers (see `Design review reconciliation.md`):

| Layer | When | Guarantee |
|---|---|---|
| 1. Quorum-certified notes: payer + any 2f+1 of 3f+1 FROST signers (e.g. any 3 of 4), each signing a nullifier for one transaction only | A quorum is reachable | Prevented if ≤ f signers misbehave |
| 2. ~~Bonded accountable intents~~ dropped as written: Zakura ignores `lock_time` on fully shielded transactions (v1.6.0 `zakura-chain/src/transaction.rs` L620–644), so a bond has no refund path, and a committee-held bond is custody. Signed counters cannot cap total loss across partitions (one credential shown to N disconnected payees exposes N × the limit); they are local loss controls only | — | — |
| 3. Conflict detection: a nullifier mined in a different txid is conflict evidence, not proof of fraud (re-issue and key theft look the same). Hot lists hold nullifiers only, never payer keys, and nothing is hot-listed automatically | Always | Detected after the next uplink |

Staging:

- **MVP v1:** pending tier (floor limit and per-payer counter kept on the payee), payment details and accountability signature sent only to the payee, conflict detection from chain data. Payees accept small amounts from known payers; final only when mined.
- **Next:** bilateral 2-of-2 FROST escrow between payer and payee, the only prevention available with three devices. Needs glue porting frost-tools' `zcash-sign` to zakura-orchard 2.2.0 (`unstable-frost`) and zakura-pczt rc4 (`sign_ironwood_with`); escrow FVKs are non-standard and not quantum-recoverable.
- **Later:** a 4-signer quorum (layer 1); in the Logos phase, basecamp committees.

## Stages

| Stage | Network | Change |
|---|---|---|
| MVP v1 | Local NU7 chain, ths v0.4.1 | Everything above |
| MVP v2 | Mainnet; NU7 from height 3,543,405 (~5 Nov 2026) | Same branch ID; Zakura Common 4.x or custom mainnet params; gateway over Tor; real ZEC, each send approved |
| Logos phase | Mainnet | Basecamps and Logos Delivery replace phones and BitChat; logos-co Zebra module as gateway; quorum committees |

## Open checks

1. Build one offline Ironwood transaction with Zakura 2.2.0 on the local chain and verify it on a separate payee process.
2. Zebra 7 accepts the same bytes.
3. A transaction proven against a fabricated anchor is rejected by the payee.
4. A deliberate conflicting spend ends as "conflicted" at the payee, not "final".
5. Packet capture shows no plaintext amounts, payees or keys on the mesh.
6. Payee restarts between receipt and forwarding; the gateway appears later and submits.
7. Anchor authentication from headers: ZIP 221/258 history-tree leaves carry Ironwood roots (Zakura 1.6.0 implements them), but no light-client MMR proof code was found.
8. frost-tools bytes (sighash, α, signature) interoperate with a Zakura 2.2.0 transaction (for the escrow phase).
9. Phone proving time; BitChat delivery time for 9,166 bytes over 1 and 3 hops.
10. ~~`lock_time` on fully shielded v6~~ resolved: not enforced.
