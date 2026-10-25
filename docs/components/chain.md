# Chain: local Regtest, Ironwood v6 rules, fees, mainnet step

MVP v1 runs on a local Regtest chain from thus-spoke-zakura (`ths`) v0.4.1: a Zakura 1.6.0 node with every upgrade through NU7 active at height 1, branch ID `0x77190AD9`, v6 transactions and the Ironwood pool.
This file holds the chain and protocol facts the other components depend on: size, expiry, branch binding, anchors and fees. Mainnet comes after MVP v1 and needs an explicit approval for every send.
`ths` bugs and workarounds are logged in [AGENTS.md](../../AGENTS.md) ("thus-spoke-zakura issue log"). That log is the master copy and is not repeated here.

## Decided

| Item | Value | Status |
|---|---|---|
| Environment | `ths` v0.4.1 ([zcashlabs/thus-spoke-zakura](https://github.com/zcashlabs/thus-spoke-zakura)), node Zakura v1.6.0 (`ebda15e`) | [decided] |
| Upgrades | NU7 (and everything before it) at height 1, hard-coded in `ths` (issue log #1) | [decided] |
| Branch ID | `0x77190AD9`; on the wire the header is `06000080` and the branch bytes are `d90a1977` | [decided] |
| Tx format | v6, Ironwood actions only, sent to ordinary unified addresses | [decided] |
| Endpoints (`ths endpoints --json`) | Zakura RPC `127.0.0.1:18232`, lightwalletd `127.0.0.1:9067` (plaintext), dashboard `127.0.0.1:32805`; loopback only (issue log #3) | [decided] |
| Chain name | Zakura reports `"chain": "test"` for this Regtest chain | [decided] |
| Health check | `getblockchaininfo` shows `consensus.chaintip = 77190ad9`, NU7 active at height 1, and an `ironwood` value pool | [decided] |
| Dev accounts | Accounts 1–5 come from a fixed public mnemonic; `ths` prints each account's unified spending key (hex). Account 1 starts with 5 Ironwood ZEC. `ths wallet faucet` and `ths wallet send` move funds between them | [decided] |
| Reorg tools | `ths reorg --depth N`, `ths rollback --depth N` | [decided] |
| Configuration | Branch ID, expiry delta and fee come from configuration, never hard-coded | [decided] |
| Fleet fee | 10,000 zats for 2 actions, which is what the wallet emits | [decided] |
| Branch epoch | A tx is bound to its branch ID. Anything unmined at an upgrade dies and is re-issued. Expiry must never straddle a known activation height | [decided] |
| `lock_time` | Ignored on fully shielded v6, so it cannot gate a refund (see Findings) | [decided] |
| Mainnet | NU7 at height **3,543,405** (~5 Nov 2026, [ZIP 259](https://zips.z.cash/zip-0259)), so mainnet uses the same branch ID. Each mainnet send needs explicit approval | [decided] |

Measured on v0.4.1 (2026-10-25): `ths wallet send --from 1 --to 2 --amount 1` produced a v6 tx with only Ironwood actions: 2 actions, **9,166 bytes**, a **10,000-zat fee** (5,000 per action) and expiry about 40 blocks out (mined at height 106 with `expiryheight` 146). The same send on v0.3.0 under NU6.3 was also 9,166 bytes with a 10,000-zat fee.

## Findings

### Transaction size ([ZIP 229](https://zips.z.cash/zip-0229) field sizes, confirmed by the measurement above)

| Part | Bytes |
|---|---|
| Header + six compactSizes | 26 |
| 2 × 820-byte actions | 1,640 |
| Flags, valueBalance, anchor | 41 |
| Proof `2720 + 2272 × nActions` (+3-byte length) | 7,264 (+3) |
| 2 × 64 spend-auth signatures + 64 binding signature | 192 |
| **Total, 2 actions** | **9,166** |

- Each additional pair of actions adds 6,312 bytes. A signed tx without its proof is about 1,902 bytes.
- 1 spend + 2 outputs pads to 2 actions. Base64 for the mesh is about 12.3 KB.
- Source: [zcash_ironwood_testnet.md Q3](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/zcash_ironwood_testnet.md).

### Anchors ([ZIP 258](https://zips.z.cash/zip-0258), [ZIP 229](https://zips.z.cash/zip-0229))

- `anchorIronwood` "MUST refer to some earlier block's final Ironwood-pool treestate". There is no age bound. Zebra accepts any historical root (`zebra-state/src/service/check/anchors.rs` L120–160; local checkout, commit not recorded). The anchor block must stay on the best chain; Zebra's `MAX_BLOCK_REORG_HEIGHT` is 1,000.
- In v6 the anchor is **authorizing data**. Re-anchoring keeps the txid, and the proofs still bind the anchor. Caches therefore key on txid plus auth digest, as Zakura does ([reconciliation §1](../../archive/reports/Design%20review%20reconciliation.md)).
- Funds received in a block cannot be spent in that same block, because the anchor must come from an earlier block ([ZIP 326](https://zips.z.cash/zip-0326)).

### Expiry ([ZIP 203](https://zips.z.cash/zip-0203), [ZIP 218](https://zips.z.cash/zip-0218))

- `nExpiryHeight` is in 1..499,999,999, or 0 for no expiry. Expiry N means the tx must be included in block N or earlier.
- Expiry is effecting data: it is fixed at signing and committed by the txid ([ZIP 374](https://zips.z.cash/zip-0374)).
- ZIP 218 recommends a 40 → 120 block default at NU7. Zakura Common 2.2.0 still has `DEFAULT_TX_EXPIRY_DELTA = 40` (`crates/zcash_primitives/src/transaction/builder.rs` L54, [zakura-core/common@b6cf4ce](https://github.com/zakura-core/common/blob/b6cf4cefe38fdccc6d2b0dc868f7d6ef01683413/crates/zcash_primitives/src/transaction/builder.rs)); Common 4.0.0 uses 120. At the 25 s target, 40 blocks is about 17 min and 120 blocks about 50 min.
- 3,456 blocks/day at the 25 s target. The old report's example fleet delta (daily uplink + 2-day margin) is 3 × 3,456 = 10,368 blocks.

### Branch-ID binding

- The v6 txid and auth digests are personalised with the branch ID (`ZcashTxHash_` / `ZTxAuthHash_` ‖ branch, [ZIP 229](https://zips.z.cash/zip-0229)). Re-anchoring cannot rescue a tx signed under the wrong epoch.
- Zebra 7.0.0-rc.0 does not penalise peers that relay the previous or next branch ID within about 50 min of an activation (Zebra CHANGELOG L69).

### `lock_time`

- Zakura v1.6.0 (`ebda15e`) `zakura-chain/src/transaction.rs` L620–644 returns `None` unless a transparent input has a non-final sequence. Zebra main does the same ([reconciliation §3](../../archive/reports/Design%20review%20reconciliation.md)).

### Fees ([ZIP 317](https://zips.z.cash/zip-0317))

| Source | Per logical action (minimum 2 actions) |
|---|---|
| ZIP 317 text | 5,000 zats (what the Zakura wallet emits: 10,000 for 2 actions) |
| Zebra ≥ 6.4.0 mempool minimum (zips#1352) | 1,000; mainnet `getstandardfee` reports 5,000 until height 3,590,000, then 1,000 |
| [ZIP 2009](https://zips.z.cash/zip-2009) (Draft) | lowers ZIP 317 to 1,000 |
| Zakura 1.6.0 | accepts 400 |

- A non-standard fee is a fingerprint, so the whole fleet uses one value: whatever the wallet emits.

### Mainnet step

- Common 2.2.0's `MAIN_NETWORK` has `Nu7 => None`. Before mainnet, move to Common 4.x (which sets 3,543,405) or supply custom mainnet parameters with the NU7 height. This does not block MVP v1.
- A tx signed under NU6.3 and still unmined at activation becomes invalid.
- Ironwood itself arrived with NU6.3: mainnet 3,428,143, testnet 4,134,000 (branch `0x37A5165B`). Testnet NU7 activated at 4,465,026 ([ZIP 258](https://zips.z.cash/zip-0258), Zebra releases). Public testnet is not a project target.
- Mainnet stage plan ([MVP v1 design, Stages](../../archive/reports/MVP%20v1%20design.md)): same builder and transport, Common 4.x or custom params, gateway over Tor ([gateway.md](gateway.md)), real ZEC.

## Open

- [unverified] A mainnet-like chain (NU6.3 active, NU7 not) needs a patched `ths` (issue log #1). It is not needed for MVP v1.
- [unverified] Zebra 7 accepts v6 bytes built with Zakura 2.2.0 (gap G7; [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md)). Test: submit the same bytes to a Zebra 7 Regtest node.
- [unverified] Zebra and Zakura behaviour with `nExpiryHeight = 0` and with very large expiries. Test both on `ths`.
- [unverified] A stale-anchor, long-expiry tx is mined days later. Test: build against an old anchor, mine past it, then submit.
- [unverified] Upstream librustzcash (`zcash_primitives` 0.31) transactions are accepted by Zakura, and the reverse (issue log #4).
- [unverified] Whether another upgrade after NU7 is scheduled; this caps the safe expiry.
- Test: `ths reorg` deeper than a payment's anchor block, to confirm the tx is rejected and re-issued.
