# Chain: public testnet, Regtest harness, Ironwood v6 rules, fees

**v0.7.7 / Common 4.0.0.** New desktop/CLI sessions default to testnet;
missing `network` in old fleets means Regtest. Independent chain proofs and
the proposed `regtest-no-pow` validator remain unbuilt.

## v0.7.0 public-testnet operation (27 October 2026)

Public testnet: NU7 at **4,465,026**, branch `0x77190AD9`, ~25-second blocks.
Reorgs are expected; ten confirmations are UI policy, not finality. Fleet
`network` selects parameters/addresses: RPC `test` also names Zakura Regtest.

The gateway uses TLS lightwalletd, not a full node. Defaults:
`https://testnet.zec.rocks:443` and
`https://zaino.testnet.unsafe.zec.rocks:443`; configure `fleet.json`'s `servers`
or the CLI's comma-separated `--rpc` list. Basecamp displays the fleet servers.
The secondary returned gRPC `UNAVAILABLE: Received http2 header with status: 503`
on 27 October. One reachable source produces a warning. Reachable sources are
compared at common-height hashes, tree states and transaction answers;
disagreeing answers are withheld. Since v0.7.5 compact ranges come only from the
primary; secondary checks compare batch-end roots, not ciphertexts/nullifiers
([sync details](mesh.md#v070-compact-sync)). Agreement is not proof.

**v0.7.0 trust exception:** state, `included` (1 confirmation) and `mined` (10)
are gateway-reported. TLS authenticates the server, not its chain. PoW, sampled
headers, MMR and inclusion proofs below describe the unbuilt mainnet design.

### Dated service checks

27 October 2026, 18:31–20:14 UTC, `testnet.zec.rocks:443`: ECC lightwalletd
v0.5.4 (`c22ca9b`), Zebra 7.0.0 (`e8db87e`), branch `77190ad9`.

| Check | Observed result |
|---|---|
| GetBlockRange | Compact headers are **empty**. Ironwood actions contain 32-byte nf/cmx/epk and 52-byte compact ciphertext |
| GetTreeState | `ironwoodTree` contains the serialized frontier. Decode with Common 4's `read_frontier_v0`; its computed root matches the canonical anchor encoding without byte reversal |
| GetTransaction | Mined height 4,548,314 for `25122ba33a82eac6a4d947c3cc6e8ae6824ebedc355c1483d6fab8d60280e6d8`; raw bytes returned |
| Duplicate mined tx | SendTransaction code −25, `failed to validate tx: WtxId("private"), error: transaction is already in state` |
| Expired fixture | Code −25, `transaction must not be mined at a block Height(4548566) greater than its expiry Height(4548564)` inside the consensus-validation error |
| Conflicting-nullifier fixture | Code −25, `ironwood double-spend: duplicate nullifier: Nullifier(Nullifier(0x…)), in finalized state: false` inside the contextual-validation error |
| Malformed encoding | Code −22, `io error: failed to fill whole buffer`; empty input is gRPC INVALID_ARGUMENT `bad transaction data` |

Expiry/conflict mutations had invalid signatures: they establish rejection
ordering/wording, not a funded double-spend. Queue-full/download/behind-tip
classification comes from pinned source and local tests, not induced public errors.

Mainnet sends require explicit approval. `ths` bugs/workarounds are in the
[issue log](../../AGENTS.md).

## Decided

| Item | Value | Status |
|---|---|---|
| Target chain | Zcash mainnet, NU7 rules. Every check in the design is specified for mainnet | [decided] |
| Harness | `ths` v0.4.1 ([zcashlabs/thus-spoke-zakura](https://github.com/zcashlabs/thus-spoke-zakura)), node Zakura v1.6.0 (`ebda15e`), local Regtest. Used for development and the demo only | [decided] |
| Regtest gaps | No PoW (`with_disable_pow(true)`, `zakura-network/src/parameters/testnet.rs` L1424–1425 at `ebda15e`), synthetic timestamps, one miner. Planned `regtest-no-pow` skips only Equihash/difficulty/timestamps and needs valid/corrupted mainnet vectors. All other proof checks remain. Never infer network from solution length | [decided] |
| Upgrades | NU7 (and everything before it) at height 1, hard-coded in `ths` (issue log #1) | [decided] |
| Branch ID | `0x77190AD9`; on the wire the header is `06000080` and the branch bytes are `d90a1977` | [decided] |
| Tx format | v6, Ironwood actions only, sent to ordinary unified addresses | [decided] |
| Endpoints (`ths endpoints --json`) | Zakura RPC `127.0.0.1:18232`, lightwalletd `127.0.0.1:9067` (plaintext), dashboard `127.0.0.1:32805`; loopback only (issue log #3) | [decided] |
| Chain name | The harness reports `"chain": "test"` (Zakura Regtest); mainnet reports `"main"`. Configuration, not this string, selects the network and the PoW flag | [decided] |
| Health check | `getblockchaininfo` shows `consensus.chaintip = 77190ad9`, NU7 active at height 1, and an `ironwood` value pool | [decided] |
| Dev accounts | Accounts 1–5 come from a fixed public mnemonic; `ths` prints each account's unified spending key (hex). Account 1 starts with 5 Ironwood ZEC. `ths wallet faucet` and `ths wallet send` move funds between them | [decided] |
| Reorg tools | `ths reorg --depth N`, `ths rollback --depth N` | [decided] |
| Configuration | Branch ID, expiry delta and fee come from configuration, never hard-coded | [decided] |
| Fleet fee | 10,000-zat floor for 2 actions; larger splits and Max use the ZIP 317 padded-action fee | [built] |
| Branch epoch | A tx is bound to its branch ID. Anything unmined at an upgrade dies and is re-issued. Expiry must never straddle a known activation height | [decided] |
| `lock_time` | Ignored on fully shielded v6. Transparent P2SH + CLTV is enforced on v6 (measured 26 Oct); CSV is not implemented. Neither stops a self-sweep (see Findings) | [decided] |
| Mainnet | NU7 at height **3,543,405** (~5 Nov 2026, [ZIP 259](https://zips.z.cash/zip-0259)), so mainnet uses the same branch ID. Common 4 already has this height, but the app does not support mainnet. Each real-value send needs explicit approval | [decided] |

25 October, ths v0.4.1: `ths wallet send --from 1 --to 2 --amount 1` produced
two Ironwood actions, **9,166 bytes**, **10,000 zat** fee (5,000/action), mined
at 106 with expiry 146. v0.3.0/NU6.3 gave the same size and fee.

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

- `anchorIronwood` "MUST refer to some earlier block's final Ironwood-pool treestate". There is no consensus age bound. Zebra accepts historical roots (`zebra-state/src/service/check/anchors.rs` L120–160 at [1e03ee7](https://github.com/ZcashFoundation/zebra/tree/1e03ee753fbc72d70882cdfd3c8bdd58b80f0f97)). The root must remain a valid historical root on the selected chain; the same root may occur in several blocks. Zebra's 1,000-block rollback window is local node policy, not consensus finality (`zebra-chain/src/parameters/constants.rs` L20–33 at that revision).
- In v6 the anchor is **authorizing data**. Re-anchoring keeps the txid, and the proofs still bind the anchor. Caches therefore key on txid plus auth digest, as Zakura does ([reconciliation §1](../../archive/reports/Design%20review%20reconciliation.md)).
- Funds received in a block cannot be spent in that same block, because the anchor must come from an earlier block ([ZIP 326](https://zips.z.cash/zip-0326)).
- Measured on `ths` v0.4.1: anchor at 1,113 mined at 11,488, **10,375 blocks old**, not three elapsed days. After a reorg removed another root, the node returned `unknown Ironwood anchor`; fresh witnesses/anchor/proof with **all old ordinary signatures** mined with the same txid. Retained-FROST-signature re-anchoring remains unexecuted. If the input note itself disappears, a new witness cannot repair it. [Evidence and pinned source lines](expiry-reissue.md#old-roots-reorgs-and-re-anchoring).

### Expiry ([ZIP 203](https://zips.z.cash/zip-0203), [ZIP 218](https://zips.z.cash/zip-0218))

- `nExpiryHeight` is in 1..499,999,999 **inclusive**, or 0 for no expiry. Expiry N permits inclusion in N. Pinned Zakura accepted tip+1 and mined exactly at expiry (no three-block expiring-soon refusal in this path); it evicts resident txs when `tip >= N`, because the next block is too late.
- Measured on `ths` v0.4.1: expiry 0 survived 1,000 deliberately empty blocks and mined; 499,999,999 mined; 500,000,000 was rejected. Zebra source agrees; live Zebra remains untested. No relative-delta cap or resident-age TTL was found in the inspected paths; long expiry does not guarantee retention/mining. [Evidence and source lines](expiry-reissue.md#consensus-expiry-versus-mempool-policy).
- Expiry is effecting data: it is fixed at signing and committed by the txid ([ZIP 374](https://zips.z.cash/zip-0374)).
- ZIP 218 recommends a 40 → 120 block default **after target height**, normally tip+1, at NU7. It preserves about 50 minutes of ordinary-wallet lifetime, not offline carriage. Zakura Common 2.2.0 still has `DEFAULT_TX_EXPIRY_DELTA = 40` (`crates/zcash_primitives/src/transaction/builder.rs` L54, [zakura-core/common@b6cf4ce](https://github.com/zakura-core/common/blob/b6cf4cefe38fdccc6d2b0dc868f7d6ef01683413/crates/zcash_primitives/src/transaction/builder.rs)); Common 4.0.0 uses 120. At the 25 s target, 40 blocks is about 17 min.
- 3,456 blocks/day at the 25 s target. Fleet profile **shutdown, 72,576 blocks (~21 days)**, assuming ≤7-day uplink (decided 26 Oct); the courier delta 10,368 was not chosen. These preserve already accepted payments, not new-acceptance freshness. [Problem #3 proposals 1 and 6](expiry-reissue.md#proposals-and-owner-decisions-26-october).

### Headers, proof of work and the history tree

- Mainnet header: 1,487 B, Equihash (200,9), 1,344-byte solution ([Zebra header length](https://doc-internal.zebra.zfnd.org/zebra_chain/block/header/constant.BLOCK_HEADER_LENGTH.html)). Regtest header on `ths` v0.4.1: 177 B (`getblockheader h false`), with all-zero 36-byte solutions and nonces and timestamps from a 2011 epoch, because `with_disable_pow(true)` skips both the difficulty filter and Equihash (issue log #8). A payee can therefore exercise PoW and timestamp checks only with mainnet header vectors; Equihash (200,9) verify measured at 96 µs/header on an orb Xeon ([`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md) `eqbench`) [unverified on Mac/iPhone].
- `hashBlockCommitments = BLAKE2b-256("ZcashBlockCommit", historyRoot(N−1 tree) ‖ authDataRoot(N) ‖ 0³²)` (Zakura `block/commitment.rs` L301–320, `zakura-state/src/service/check.rs` L321–335; [ZIP 244](https://zips.z.cash/zip-0244)). The verbose `getblockheader` includes `blockcommitments` (`methods.rs` L2209–2227).
- The ZIP 221/258 history tree has V3 leaves with `start_ironwood_root`, `end_ironwood_root` and `ironwood_tx` (`zcash_history` 0.5.0 `node_data.rs` L229–256, `version.rs` L183–220). Zakura Common 2.2.0 ships no history crate; Zakura v1.6.0 pins upstream `zcash_history = "0.5.0"` (Zakura `Cargo.toml` L54). The crate only appends, truncates and returns the root; it has no proof API. Nodes keep only MMR peaks (Zakura `history_tree.rs` L71–85, `prune()` L373–445; Zebra `zebra-chain/src/history_tree.rs` L282, L353), and no RPC returns a Merkle path (issue log #9). The tree resets at each upgrade activation (`history_tree.rs` L313–321), so an anchor from before an activation is proved against the earlier tree.
- Measured on `ths` v0.4.1 (tip 132): 131/131 rebuilt history roots matched header N+1; txid Merkle roots matched 132/132 headers. How the gateway rebuilds and serves paths is in [gateway.md](gateway.md); how the payee checks them is in [payee-verifier.md](payee-verifier.md).

### Branch-ID binding

- The v6 txid and auth digests are personalised with the branch ID (`ZcashTxHash_` / `ZTxAuthHash_` ‖ branch, [ZIP 229](https://zips.z.cash/zip-0229)). Re-anchoring cannot rescue a tx signed under the wrong epoch.
- Zebra 7.0.0-rc.0 does not penalise peers that relay the previous or next branch ID within about 50 min of an activation (Zebra CHANGELOG L69).
- No NU8-or-later activation height was found in the pinned ZIP/Zebra revisions as of **25 October 2026**. This is not a future validity promise. The proposed signing/renewal rule caps at next known activation − 1 and requires authenticated, fresh fleet configuration; `with_expiry_height` does not enforce this itself ([expiry-reissue.md](expiry-reissue.md#lifetime-freshness-privacy-and-upgrades)).

### `lock_time`

- Zakura v1.6.0 (`ebda15e`) `zakura-chain/src/transaction.rs` L620–644 returns `None` unless a transparent input has a non-final sequence. Zebra main does the same ([reconciliation §3](../../archive/reports/Design%20review%20reconciliation.md)).
- v6 carries transparent bundles (`transaction.rs` L149–170); P2SH|CHECKLOCKTIMEVERIFY enabled (`zakura-script/src/lib.rs` L173–177; Zebra 7 `zebra-script/src/lib.rs` L177–182); opcode 0xb2 (CSV) is still NOP3. Measured on `ths`: a CLTV-120 spend was rejected at tips 106/119 and mined at 121 (bond research).
- zcashd (v6.20.0/master) has no Ironwood/NU7 support.

### Fees ([ZIP 317](https://zips.z.cash/zip-0317))

| Source | Per logical action (minimum 2 actions) |
|---|---|
| ZIP 317 text | 5,000 zats (what the Zakura wallet emits: 10,000 for 2 actions) |
| Zebra ≥ 6.4.0 mempool minimum (zips#1352) | 1,000; mainnet `getstandardfee` reports 5,000 until height 3,590,000, then 1,000 |
| [ZIP 2009](https://zips.z.cash/zip-2009) (Draft) | lowers ZIP 317 to 1,000 |
| Zakura 1.6.0 | accepts 400 |

- A non-standard fee is a fingerprint, so the whole fleet uses one value: whatever the wallet emits.

### Stack compatibility (#7, 26 October)

- **Measured:** official Zebra 7.0.0 (`e8db87e`) on a fresh NU7 Regtest accepted to its mempool, and mined at height 103, a Common 2.2.0 payer + 3-of-4 FROST 9,166-byte v6 spend (txid `72beac5d82834891b0d286a05af0272487774e3fe960519309476385e3bd49d7`). All 5 saved transactions verify identically under Common 2.2.0, Common 4.0 and Zebra's upstream crates (`orchard` 0.16, `zcash_primitives` 0.31, `zcash_protocol` 0.11), and corruptions are rejected. Weighted DKG passes all 5 valid signer subsets on both 2.2.0 and 4.0. Not tested: 4.0-built transactions mined, public networks. #7 research
- **Testnet needs no library change:** Common 2.2.0 `TEST_NETWORK` already sets NU7 = 4,465,026 (`zcash_protocol/src/consensus.rs` L491–536 at `b6cf4ce`); only `MAIN_NETWORK` has `None`.
- **4.0 migration completed in v0.7.0:** the app pins Common 4.0.0 and client-backend rc9.5; PCZT rc9 is research-only, not linked by the app. Research FROST moves to `zakura-reddsa-frost` 4.0 (`pallas-pasta` feature), still frost-core 3.0. The `use_qsk` FVK helper stays ours. Default expiry changes 40 → 120, so the app keeps explicit fleet expiry and fee. Types from different families cannot be mixed directly; serialized transactions cross between them. Funded v0.7.5 Common 4 payments ran on public testnet ([evidence](../open-work.md#built)); the earlier FROST research above is a separate experiment.
- **Testnet difficulty:** a block more than 450 s after its parent may use the PoW-limit difficulty (ZIP 218). The testnet verifier must allow that rule; testnet results never stand in for mainnet sampled-header security.
- Testnet endpoints: the configurable public TLS lightwalletd list above; no VPS or own node. Funding and the live-run limitations are in [install.md](../install.md#public-testnet-v070).

### Mainnet step

- Common 4 sets mainnet NU7 at 3,543,405; the application still does not support mainnet.
- A tx signed under NU6.3 and still unmined at activation becomes invalid.
- Ironwood itself arrived with NU6.3: mainnet 3,428,143, testnet 4,134,000 (branch `0x37A5165B`). Testnet NU7 activated at 4,465,026 ([ZIP 258](https://zips.z.cash/zip-0258), Zebra releases). Public testnet is the v0.7.0 experiment target.
- Mainnet needs network support, chain proofs, Tor, independent gateways and security/hardware gates ([gateway](gateway.md)). This supersedes the [archive's later Tor/header stage](../../archive/reports/MVP%20v1%20design.md).

## Open

- [unverified] A mainnet-like chain (NU6.3 active, NU7 not) needs a patched `ths` (issue log #1). It is needed for two tests: a tx unmined across activation dies and is re-issued, and an anchor from before the activation is proved against the pre-activation history tree.
- [unverified] `regtest-no-pow` test vectors: a real mainnet header passes the Equihash (200,9) and difficulty check and a corrupted one fails, both with the flag off; with the flag on, a live 177-byte Regtest header is accepted and the rest of the bundle checks still run.
- [unverified] Zakura's history-tree commitment in the mainnet NU7 activation block; Zebra 7 RPC behaviour for `getblock h 0`, `z_gettreestate` and verbose `getrawtransaction`.
- [unverified] Live Zebra expiry boundaries/mempool and old-anchor acceptance with the same bytes; Zakura results are in Findings, not open. Real multi-day wall-clock residence, restart/congestion policy and mainnet reorg risk remain untested.
- [unverified] Upstream librustzcash (`zcash_primitives` 0.31) transactions are accepted by Zakura, and the reverse (issue log #4).
- Accepted repair decisions, remaining research items and ten renewal/reorg/activation gates: [expiry-reissue.md](expiry-reissue.md#open). Recheck upgrade schedules at signing/renewal; no activation-crossing experiment was run.
