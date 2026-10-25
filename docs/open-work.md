# Open work

Ordered. Each item names the component doc that owns it. Update this list as items close.

## Build MVP v1 (in order)

| # | Task | Done when | Doc |
|---|---|---|---|
| 1 | Offline builder: one v6 Ironwood transaction with Zakura Common 2.2.0, no network, fleet expiry and fee | Raw bytes written; mined on `ths` | [offline-builder](components/offline-builder.md) |
| 2 | Payee verifier in a separate process: read, decrypt own output, proofs and signatures, v6/Ironwood-only/branch ID/expiry/anchor checks | Accepts the tx from 1; receipt says "passes checks at height H; pending" | [payee-verifier](components/payee-verifier.md) |
| 3 | Gateway submitter: `sendrawtransaction`, then inclusion proof and headers back | Idempotent resubmission; B moves to "included" | [gateway](components/gateway.md) |
| 4 | BitChat fork on three Macs: tx as private message A→B, B persists, B→C forward, receipts back | The restart test below passes | [transport-bitchat](components/transport-bitchat.md) |
| 5 | Pending policy on the payee: floor limit, per-payer counter, nullifier hot list, conflict detection | Conflicting spend ends "conflicted" | [double-spend](components/double-spend.md) |

## Tests that gate MVP v1

1. Fabricated-anchor transaction is rejected by the payee. ([payee-verifier](components/payee-verifier.md))
2. Deliberate conflicting spend: B reports "conflicted", not "final". ([double-spend](components/double-spend.md))
3. A sends to B → B confirms only after saving → A disappears → B restarts → C appears later → B delivers, C submits, confirmation returns. ([transport-bitchat](components/transport-bitchat.md))
4. Packet capture: no plaintext amounts, payees or keys on the mesh. ([privacy-and-threats](privacy-and-threats.md))
5. Same tx bytes accepted by Zebra 7, not only Zakura. ([chain](components/chain.md))
6. BitChat delivery time for a 9,166-byte tx (base64 ≈ 12.3 KB) over 1 and 3 hops. ([transport-bitchat](components/transport-bitchat.md))

## Next phase (MVP v1.1)

- Bilateral 2-of-2 FROST escrow between payer and payee: port frost-tools' `zcash-sign` to zakura-orchard 2.2.0 (`unstable-frost`) and zakura-pczt rc4; about 300–500 lines. ([double-spend](components/double-spend.md))
- Anchor authentication from headers: MMR proof code for ZIP 221/258 history-tree leaves (none found). ([payee-verifier](components/payee-verifier.md))

## Known gaps carried forward (not fixed by design)

- Gateway is a single trust point and first spy. ([gateway](components/gateway.md))
- Fleet transactions are fingerprintable on chain. ([privacy-and-threats](privacy-and-threats.md))
- Expiry (~3 days) is short against real shutdowns; re-issue needs the payer. ([chain](components/chain.md))
- Mainnet needs Common 4.x or custom params with the NU7 height. ([chain](components/chain.md))
