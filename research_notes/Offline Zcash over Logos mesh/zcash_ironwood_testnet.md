# Zcash Ironwood pool and the current Zcash testnet (Oct 2026): offline construction, delivery and settlement

Scope note: research date 2026-10-24. Most protocol facts come from primary sources that were read directly: the local ZIPs repo snapshot (`/Users/megabyte/Developer/zcash/zip-tools/submodule/zips`, HEAD `eaccf9a9`, 2026-09-30, "Merge pull request #1382") and the local Zebra source (`/Users/megabyte/Developer/zcash/zebra`, HEAD `98303adb`, 2026-10-08, `v7.0.0-rc.0-58`). Each local citation gives a file path and line, plus the public URL where one exists. Hosted zips.z.cash pages may differ slightly from the local snapshot. Press and exchange blogs are treated as secondary sources and labelled as such.

## Q1. What exactly is "Ironwood"? (upgrade, cryptography, keys and addresses, notes, nullifiers and commitments, relationship to Orchard, Sapling and Tachyon, status)

### Takeaway
Ironwood is real and deployed. It is the new shielded pool introduced by network upgrade **NU6.3**, not NU7. It activated on **Testnet at height 4,134,000** and on **Mainnet at 3,428,143** (about 2026-07-28). Ironwood is a *second value pool of the Orchard protocol*. It reuses the same Action encoding, the same Halo 2 proof system and the same addresses and keys. It has its own note commitment tree, nullifier set, anchor and value-pool balance, plus a new "quantum-recoverable" note plaintext format (lead byte `0x03`, ZIP 2005). From NU6.3 the Orchard pool is frozen against new inflows, so Ironwood is the only Orchard-protocol pool that can receive payments between users. The planned post-quantum Recovery Protocol, Tachyon, ZSAs and memo bundles are all still proposals. None of them is part of Ironwood as deployed.

### Cited Findings
**Identity and deployment**
- ZIP 258 "Deployment of the NU6.3 Network Upgrade": "This proposal defines the deployment of the NU6.3 network upgrade, which introduces the Ironwood shielded pool." Constants: `CONSENSUS_BRANCH_ID 0x37A5165B`; `ACTIVATION_HEIGHT (NU6.3) Testnet: 4134000, Mainnet: 3428143`; `MIN_NETWORK_PROTOCOL_VERSION 170160` on both networks. — [ZIP 258](https://zips.z.cash/zip-0258) (local `zips/zip-0258.md:65-75`)
- Zebra hard-codes the same values: testnet `NU6_3: Height(4_134_000)` and `NU7: Height(4_465_026)`; mainnet `NU6_3: Height(3_428_143)`. Branch IDs are `Nu6_3 0x37a5165b` ("The NU6.3 (Ironwood) consensus branch id, matching zcash_protocol's BranchId::Nu6_3") and `Nu7 0x77190ad9`. — local `/Users/megabyte/Developer/zcash/zebra/zebra-chain/src/parameters/constants.rs:74,76,104`; `zebra-chain/src/parameters/network_upgrade.rs:64-66,241-244`
- Zebra 6.0.0-rc.0 (2026-07-02) added "Support for the NU6.3 'Ironwood' shielded pool and v6 transaction format, activating on Testnet at height 4,134,000". Zebra 6.0.0 (2026-07-10) added "NU6.3 (Ironwood) now activates on Mainnet at block height 3,428,143". — local `zebra/CHANGELOG.md:353,402`; [Zebra releases](https://github.com/ZcashFoundation/zebra/releases)
- The Zaino 0.6.0 release notes say it "adds full support for the Zcash Ironwood network upgrade (NU6.3)" and give mainnet activation at 3,428,143 (about 2026-07-28). — [Zaino releases](https://github.com/zingolabs/zaino/releases)
- Secondary sources also report mainnet activation on 2026-07-28 at 3,428,143. — [KuCoin blog](https://www.kucoin.com/blog/zcash-ironwood-nu6-3-shielded-pool-migration) (secondary)
- Motivation: an Orchard Action circuit soundness bug was reported on 2026-05-29 by Taylor Hornby (GHSA-ww9q-8r59-xv46). It "could have allowed balance violation and theft of funds". Orchard was disabled, then fixed in NU6.2. — [ZIP 257](https://zips.z.cash/zip-0257) (local `zips/zip-0257.md`, Motivation). ZIP 229 states: "The main purpose of the NU6.3 network upgrade is to bolster confidence in Zcash's supply integrity, after the discovery and remediation of the Orchard soundness vulnerability". — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:82-90`)
- ZODL (the wallet formerly called Zashi; "Zashi is now Zodl") describes Ironwood as a new pool using the existing Orchard protocol, backed by formal verification. It says "a turnstile verifies the total supply moved from Orchard to Ironwood" and that "the new pool uses the user's existing Orchard receiver, so previously used addresses will keep working". Post dated 2026-06-06. — [ZODL: Ironwood, a new shielded pool](https://zodl.com/ironwood-a-new-shielded-pool-for-zcash/)
- The Ironwood Book is the formal verification project, written in Lean 4 over Mathlib. It covers "at least the soundness of the Action circuit" used by both Orchard and Ironwood. — [The Ironwood Book](https://zcash.github.io/ironwood/). A secondary source reports "more than 2,700 theorems" machine-checked. — [KuCoin blog](https://www.kucoin.com/blog/zcash-ironwood-nu6-3-shielded-pool-migration) (secondary, unverified)

**Cryptography and relationship to Orchard**
- ZIP 229: Ironwood is "an Orchard-protocol successor to the Orchard pool". A v6 transaction holds "a second Orchard-protocol shielded component that commits to, and spends from, the Ironwood pool rather than the Orchard pool. The Orchard Action encoding and the Halo 2 proof system are reused unchanged. The circuit is also shared between these pools, changed only slightly from the pre-NU6.3 circuit [ZIP 2006]." — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:50-64`)
- ZIP 229 continues: "The addition of the Ironwood pool does not change address structures or encodings. At the protocol level, Orchard spending-key and viewing-key material grants authority to spend or view notes in both the Orchard and Ironwood pools." The pools "are distinguished by their note commitment trees, nullifier sets, value pool balances, and component position, not by separate circuits." — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:65-72,508-521`)
- Zebra's module documentation reads: "The Ironwood pool reuses the Orchard Action + Halo2 proof system … it has its own note commitment tree, nullifier set, and chain value pool". It also says "the Ironwood and Orchard nullifier sets are disjoint even when their bit patterns coincide". — local `zebra/zebra-chain/src/ironwood.rs:1-11,18-24`
- "Every Ironwood-pool output note uses the quantum-recoverable note plaintext format (lead byte `0x03`) defined in ZIP 2005; no Orchard-pool output note uses that format. This is the only note-level distinction between the two pools." — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md`, after the format table)
- The Orchard-protocol circuit is Halo 2 with `const K: u32 = 11` (2^11 rows). The Merkle depth is `MERKLE_DEPTH_ORCHARD: usize = 32`. — local `~/.cargo/registry/src/index.crates.io-*/orchard-0.16.0/src/circuit.rs:76`, `.../src/constants.rs:14`
- ZIP 2005 "Ironwood Quantum Recoverability" (status in repo: Proposed; Owners Daira-Emma Hopwood and Jack Grigg; Credits Sean Bowe, Dev Ojha, Kris Nuttycombe) defines the lead-byte-`0x03` format. In it, "pre_rcm … includ[es] all of the note fields" so that "the overall commitment scheme is post-quantum binding". For `0x03`, rcm = H^rcm,Orchard_rseed(g_d\*, pk_d\*, v, ρ, ψ). For `0x02`, the old derivation ToScalar(PRF^expand_rseed([0x05] || ρ)) still applies. — [ZIP 2005](https://zips.z.cash/zip-2005) (local `zips/zip-2005.md:196-205,771-773`)
- ZIP 2005 is "not by itself mak[ing] the protocol secure against quantum adversaries". If discrete-log-based protocols are later disabled, "it will be possible to use a Recovery Protocol to recover funds from the Ironwood pool". That Recovery Protocol "is described in outline but not in detail" and "It is not required to add support for the Recovery Protocol to consensus rules now." — [ZIP 2005](https://zips.z.cash/zip-2005) (local `zips/zip-2005.md`, Abstract and Non-requirements)
- Note commitment and nullifier: Ironwood keeps Orchard's NoteCommit (Sinsemilla) and DeriveNullifier_nk(ρ, ψ, cm). Only the rcm derivation changes for `0x03`, per ZIP 2005's flow diagram. — [ZIP 2005](https://zips.z.cash/zip-2005) (local `zips/zip-2005.md:262-330`)

**Key hierarchy and address format**
- Orchard-protocol key components: sk gives ask (spend authorising key) and ak = [ask]G, nk and rivk. The full viewing key is (ak, nk, rivk). ivk = Commit^ivk_rivk(ak, nk). Addresses are (d, pk_d = [ivk]·g_d). ZIP 2005 adds two branches for rivk_ext:
  - Legacy keys: rivk_ext = H^rivk_legacy(sk).
  - FROST and hardware-wallet keys (`use_qsk = true`): a "quantum spending key" qsk and qk = H^qk(qsk), with rivk_ext = H^rivk_ext_qk(ak, nk).
  - The internal rivk is H^rivk_int_rivk_ext(ak, nk).
  — [ZIP 2005](https://zips.z.cash/zip-2005) (local `zips/zip-2005.md:200-240,330-380`)
- ZIP 326: "the same ivk is used to trial-decrypt both Orchard-pool and Ironwood-pool note ciphertexts". Addresses and ivk depend on `use_qsk`, so a wallet restoring from seed may need to trial-decrypt with up to four ivks: {external, internal} × {use_qsk false, true}. Wallets MUST use one `use_qsk` value per ZIP 32 account, and "MUST NOT send funds to any external receiver (including its own) in the Orchard pool after NU6.3 activation". — [ZIP 326](https://zips.z.cash/zip-0326) (local `zips/zip-0326.md:111-160`)
- Testnet encodings follow ZIP 316, with the suffix "test" appended to the prefix:
  - Unified Addresses: `utest…` (Revision 0), or `zutest…` / `tutest…` (Revision 2, Draft).
  - Unified Incoming Viewing Keys: `uivktest…`.
  - Unified Full Viewing Keys: `uviewtest…`.
  - Sapling HRP: `ztestsapling`. TEX HRP: `textest`.
  — [ZIP 316](https://zips.z.cash/zip-0316) (local `zips/zip-0316.rst:350-378`); protocol spec (local `protocol/protocol.tex:12815`); [ZIP 320](https://zips.z.cash/zip-0320) (local `zips/zip-0320.rst:138`)

**Consensus changes at NU6.3 (Orchard freeze)**
- From NU6.3: coinbase transactions must not contain Orchard actions; every Orchard-pool Action must have `enableCrossAddress = 0`; and "No new value may enter the Orchard pool" (v^OrchardPoolBalance ≥ 0). The result is that "newly created shielded value is directed into the Ironwood pool". — [ZIP 258](https://zips.z.cash/zip-0258) (local `zips/zip-0258.md:85-103`). The same rules are implemented in local `zebra/zebra-consensus/src/transaction/check.rs:173-255`.
- `flagsIronwood` uses bit 0 `enableSpends`, bit 1 `enableOutputs` and bit 2 `enableCrossAddress`. Only the Ironwood pool permits bit 2. In a v6 coinbase, `enableSpendsIronwood` MUST be 0. Coinbase Ironwood outputs must use lead byte `0x03`. — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:238-350`); [ZIP 258](https://zips.z.cash/zip-0258)
- ZIP 2006 ("Restricting Transfers into the Orchard Pool") has status **Reserved** in the repo, so its text is not published there. Its restriction is enforced through the circuit verifying key chosen by consensus branch. — local `zips/zip-2006.md` header; [ZIP 258](https://zips.z.cash/zip-0258)

**Spec status flags (as of the 2026-09-30 repo snapshot)**
- Several specs still show Draft or Proposed status even though NU6.3 is live on both networks:
  - Draft: ZIP 229, ZIP 258, ZIP 318 and ZIP 326.
  - Proposed: ZIP 2005.
  - ZIP 317: "[Revision 0] Active, [Revision 1: NU6.3] Draft, [Revision 2] Draft".
  - ZIP 374 (PCZT): "[Revision 0] Draft".
  — local headers of `zips/zip-0229.md`, `zip-0258.md`, `zip-0318.md`, `zip-0326.md`, `zip-2005.md`, `zip-0317.rst`, `zip-0374.md`
- ZIP 229 lists as non-requirements "ZSAs, the `zip233Amount` field, the explicit `fee` field … or … ZIP 248". ZIP 231 (Memo Bundles) is Draft. Neither is part of v6 or Ironwood. — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:154-163`); local `zips/zip-0231.md` header

**Relationship to Tachyon**
- Tachyon is Sean Bowe's proposal of 2025-04-02 for out-of-band payments and oblivious synchronization: "I am proposing several protocol changes in Zcash". — [Sean Bowe, "Tachyon"](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/)
- Ironwood v6 transactions still carry on-chain note ciphertexts (`encCiphertext`/`outCiphertext` inside each 820-byte Action). — [ZIP 229](https://zips.z.cash/zip-0229)
- A secondary report says Tachyon components are being moved into "Zakura Common" (first release "Udon"). — [EdgeX/Coindoo](https://pro.edgex.exchange/en-US/news/article/zcash-integrates-tachyon-into-zakura-common) (secondary, undated)

### Inferences
- "Ironwood ONLY" in practice means v6 transactions whose only shielded component is `vActionsIronwood`. The keys and addresses are ordinary Orchard-protocol unified addresses (`utest…` with an Orchard receiver) on testnet. The pool is selected by the transaction component, not by the address.
- Because Orchard-protocol nullifiers are deterministic from note data and nk, a sender cannot make two different spends of the same Ironwood note look unrelated. Every double-spend attempt reveals the same nullifier, and consensus mines at most one of them.
- For a new testnet deployment there is no legacy Orchard balance to migrate. A fresh account created after NU6.3 never needs to scan the Orchard pool (ZIP 326 scan-range table).

### Gaps
- I could not read ZIP 2006's normative text (it is "Reserved" in the repo snapshot). The exact circuit diff for `enableCrossAddress` is known only through ZIP 229, ZIP 258 and the Zebra code.
- The z.cash website, Electric Coin Co, Zcash Foundation, Shielded Labs, QEDIT and forum posts were not fetched directly for Ironwood. The ZODL post and the ZIPs are the primary sources used here.
- I did not confirm the formal-verification claim of "more than 2,700 theorems" from a primary source.

## Q2. Current testnet state (activation heights, branch IDs, block time, nodes, indexers, wallets, faucets, explorers)

### Takeaway
Public testnet is past both NU6.3 (Ironwood, 4,134,000) and **NU7 (4,465,026, activated around 4–6 Oct 2026)**. The tip was 4,491,325 at 2026-10-24 12:26 UTC.
- **Branch ID:** new transactions mined today must use `nConsensusBranchId = 0x77190AD9` (NU7).
- **Block time:** NU7 sets a 25 s target (it was 75 s).
- **Transaction versions:** v4 is invalid from NU7; v5 and v6 are allowed, and Ironwood requires v6.
- **Peers:** nodes must advertise protocol version ≥ 170180.
- **Software:** the working stack is the "Z3" set: zebrad 7.0.0-rc.0+ (or the Zakura fork), Zaino 0.10.x or Zebra's built-in lightwalletd gRPC, and Zallet 0.1.0-beta.x.
- **zcashd:** deprecated.
- **Coverage gaps:** consumer light-wallet support for testnet plus Ironwood is unverified, and so are the faucets.

### Cited Findings
**Heights and branch IDs (Testnet)**

| Upgrade | Testnet height | Branch ID | Source |
| --- | --- | --- | --- |
| Overwinter | 207,500 | 0x5ba81b19 | local `zebra-chain/src/parameters/constants.rs:56`, `network_upgrade.rs:232` |
| Sapling | 280,000 | 0x76b809bb | same, `:58`, `:233` |
| Blossom | 584,000 | 0x2bb40e60 | `:60`, `:234` |
| Heartwood | 903,800 | 0xf5b9230b | `:62`, `:235` |
| Canopy | 1,028,500 | 0xe9ff75a6 | `:64`, `:236` |
| NU5 | 1,842,420 | 0xc2d6d0b4 | `:66`, `:237` |
| NU6 | 2,976,000 | 0xc8e71055 | `:68`, `:238` |
| NU6.1 | 3,536,500 | 0x4dec4df0 | `:70`, `:239` |
| NU6.2 | 4,052,000 | 0x5437f330 | `:72`, `:240` |
| **NU6.3 (Ironwood)** | **4,134,000** | **0x37a5165b** | `:74`, `:242`; [ZIP 258](https://zips.z.cash/zip-0258) |
| **NU7** | **4,465,026** | **0x77190ad9** | `:76`, `:244`; Zebra CHANGELOG |

- Testnet network magic is `[0xfa, 0x1a, 0xf9, 0xbf]`. The v6 version group ID is `0xD884_B698`. — local `zebra/zebra-chain/src/parameters/constants.rs:42`; `zebra-chain/src/parameters/transaction.rs:16`
- Zebra 7.0.0-rc.0 (2026-10-01) "supports the NU7 network upgrade on Testnet, which activates at height 4,465,026, expected around October 6th". Its ZIP 259 peer-version checks "require 170180 on Testnet … Zebra disconnects peers that advertise an earlier version, including Zebra 6.x". No Mainnet NU7 height is set in that release. — local `zebra/CHANGELOG.md:8-13,31,44`
- Conflict on dates and status:
  - The repo's ZIP 259 (snapshot 2026-09-30) still shows "Testnet: TBD (To be set on OCT 5)" and "Mainnet: TBD (To be set on OCT 20)". It also says each NU7 activation height "MUST be a multiple of 3". 4,465,026 is a multiple of 3. — local `zips/zip-0259.md:110-118`
  - Secondary sources say NU7 activated on testnet on **4 Oct 2026** at 4,465,026. — [macmyths](https://macmyths.com/how-to-set-up-a-zcash-nu7-testnet-wallet/) (secondary)
  - The user's local facts file records 2026-10-06, and "Mainnet height assigned 2026-10-20, activation target 2026-11-05". — local `/Users/megabyte/Developer/zcash/zcash-skills/docs/versioned-facts.md:14` (secondary)
- The testnet explorer shows the latest block **4,491,325 at 2026-10-24 12:26:35 UTC**. It lists an "Orchard Pool" balance of 238,070.348 TAZ and shows **no Ironwood pool**. — [testnet.zcashexplorer.app](https://testnet.zcashexplorer.app/)

**Block time and NU7 rules that matter for transaction construction**
- `POST_BLOSSOM_POW_TARGET_SPACING = 75` and `POST_NU7_POW_TARGET_SPACING = 25` (seconds). — local `zebra/zebra-chain/src/parameters/network_upgrade.rs:253,260`
- ZIP 218 (25 s blocks, Draft) gives 3,456 blocks per day. Per-block limits are 330 Orchard actions, 330 Ironwood actions and 300 Sapling inputs plus outputs, under a global budget of 330. The Testnet minimum-difficulty rule now uses "eighteen 25-second target spacings", meaning a gap of more than 450 s. — [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md`); local `zebra/CHANGELOG.md:45,57`; `zebra/zebra-consensus/src/block.rs:191-195`
- ZIP 2003: "[NU7 onward] The transaction version number MUST be 5 or 6." — [ZIP 2003](https://zips.z.cash/zip-2003); local `zebra/zebra-consensus/src/transaction.rs:1070-1089`
- Mempool admission: Zebra 6.4.0+ "accepts transactions that pay at least 1000 zatoshis per ZIP-317 logical action instead of 5000 (2000 instead of 10000 for a minimal transaction)". `getstandardfee` "Test networks report 1000 at every height". — local `zebra/CHANGELOG.md:65,110`
- From NU7, ZIP 235 sends 60% of block fees to the NSM reserve. This affects miners, not transaction builders. — local `zebra/CHANGELOG.md` (7.0.0-rc.0 "Added"); [ZIP 235](https://zips.z.cash/zip-0235)

**Node implementations and zcashd status**
- zebrad: 7.0.0-rc.0 (2026-10-01) is required for testnet after NU7. Pre-built Linux x86_64 and aarch64 binaries are attached to releases since 6.0.0-rc.0. — local `zebra/CHANGELOG.md:8-13,~415`
- zcashd is deprecated. ZODL cites "zcashd end-of-support at block height 3,417,100" (mainnet). Zebra provides an EXPERIMENTAL "zcashd-compat" mode that runs a hard-locked zcashd wallet as a P2P sidecar. — [ZODL post](https://zodl.com/ironwood-a-new-shielded-pool-for-zcash/); local `zebra/CHANGELOG.md` (6.2.0 "Added")
- Zakura is a consensus-compatible Rust node forked from Zebra (`zakura-core/zakura`), with its own `zakura-*` crate family. Zakura 1.3.0 and "Zakura Common" were released on 2026-08-29. — local `/Users/megabyte/Developer/zcash/zcash-skills/skills/zakura/SKILL.md:8`; [CoinInsider](https://www.coininsider.org/news/zcash-proving-times-reach-around-200ms/) (secondary)

**Light-wallet servers and indexers**
- Zebra 6.4.0 adds an experimental built-in gRPC server implementing lightwalletd's `CompactTxStreamer`, enabled with `rpc.lightwalletd_listen_addr`. — local `zebra/CHANGELOG.md:98`
- Zebra's compact-block proto adds `ironwoodCommitmentTreeSize = 3` and `repeated CompactOrchardAction ironwoodActions = 9`, with the comment "Ironwood reuses the Orchard action encoding … Fields 7 and 8 are `vin` and `vout` upstream; they are left free here". — local `zebra/zebra-rpc/proto/compact_formats.proto:17,60-62`
- Zaino: latest release 0.10.1 (29 Sep). 0.6.0 (13 Jul) has "full support for … Ironwood". In 0.10.1 "The lightwalletd protocol is unchanged". NU7 is not mentioned in its release notes. — [Zaino releases](https://github.com/zingolabs/zaino/releases)
- A known compatibility hazard: the new `ironwood` entry in `valuePools` broke pinned older Zaino releases against Zebra 6. — local `zcash-skills/skills/zcash-protocol/references/protocol-consensus.md:35` (secondary)

**Full wallet (Zallet)**
- Zallet versions:
  - v0.1.0-beta.1 (12 Jul) delivered "Integrate Ironwood (NU6.3) wallet support: treestate, RPC surface, and sync plumbing".
  - v0.1.0-beta.3 (26 Aug, latest) added "JSON-RPC methods for working with PCZTs" and implemented `z_sendfromaccount` on the PCZT pipeline.
  - Backends are zebra-state and Zaino.
  - The release notes mention neither NU7 nor testnet. Regtest is explicitly supported.
  — [Zallet releases](https://github.com/zcash/wallet/releases)

**Faucets and explorers**
- Explorer: [testnet.zcashexplorer.app](https://testnet.zcashexplorer.app/) is live and tracking the tip (see above).
- The Zcash testnet guide says to get TAZ "by asking in the Discord support channel or by using a faucet". — [Zcash testnet guide](https://zcash.readthedocs.io/en/latest/rtd_pages/testnet_guide.html)
- The ZecHub wiki lists a testnet faucet. — [ZecHub faucets](https://zechub.wiki/using-zcash/faucets) (page undated, not verified as live)
- NU6.3 allows shielded coinbase into Ironwood: v6 coinbase may create Ironwood outputs with lead byte `0x03` and `enableSpendsIronwood = 0`. Zebra supports mining to a shielded miner address. — [ZIP 258](https://zips.z.cash/zip-0258); local `zebra/zebra-consensus/src/transaction/check.rs:295-304`; local `zebra/CHANGELOG.md:~137` (shielded coinbase proof caching)

### Inferences
- **Observed testnet block rate is well below target.** From NU7 activation (4,465,026, around 4–6 Oct) to 4,491,325 (24 Oct 12:26 UTC) is about 26,300 blocks in about 18.5–20.5 days, an average of roughly **60–67 s per block**, not 25 s. All wall-clock estimates of expiry or confirmation on testnet should use observed rates. This is my own arithmetic from two cited data points.
- The explorer showing an "Orchard Pool" but no "Ironwood" pool most likely means the explorer has not been updated for NU6.3. It should not be treated as evidence about Ironwood state. The authoritative source is a local zebrad `getblockchaininfo` call, whose `valuePools` include `ironwood` (local `zebra/CHANGELOG.md:~440`).
- An offline basecamp can run zebrad 7.x alone with `rpc.lightwalletd_listen_addr` and serve compact blocks to local wallets, without a separate lightwalletd or Zaino. That feature is marked experimental.
- Zebra's own proto uses field 9 for `ironwoodActions`. Wallet clients must use a proto revision that matches the server (Zebra built-in vs Zaino vs lightwalletd). I did not verify that upstream lightwalletd uses the same field number.

### Gaps
- I found no primary announcement confirming the exact date NU7 activated on testnet (4 Oct vs 6 Oct), and no forum post on the Ironwood testnet activation date.
- No confirmed live testnet faucet, and none known to pay directly into Ironwood rather than transparent.
- Which consumer light wallets (ZODL, Zingo, YWallet) support testnet plus Ironwood plus NU7 is unverified (see Q5).
- I did not verify whether Zakura's release line follows the testnet NU7 branch (peer version ≥ 170180).
- Whether any further testnet upgrade is scheduled after NU7 is unknown.

## Q3. Offline transaction construction requirements (witness and anchor, anchor age, expiry, fees, branch ID, PCZT, proving keys, proving cost, transaction size)

### Takeaway
An offline spender needs six things:
1. **Keys:** the Orchard-protocol spending key (ask) and full viewing key.
2. **Note data:** the plaintext of each spent note (d, pk_d, v, ρ, rseed).
3. **Witness:** a 32-level Merkle path to *any* past final Ironwood treestate root. **There is no maximum anchor age** in consensus or in Zebra, as long as the anchor block stays on the best chain.
4. **Branch ID:** the branch ID of the upgrade that will be active when the transaction is mined (NU7 `0x77190AD9` on testnet now).
5. **Expiry height:** default 120 blocks after NU7, maximum 499,999,999, or 0 to never expire.
6. **Fee:** ZIP 317 gives 2 logical actions × marginal fee. That is 2,000 zats at Zebra's testnet mempool minimum, or 10,000 zats under ZIP 317's still-published 5,000/action.

Halo 2 needs **no downloaded parameters**: the proving key is generated locally from the K=11 circuit. A 1-input/2-output Ironwood-only v6 transaction is **9,166 bytes**, of which 7,264 bytes is the proof. The biggest offline-relevant change in v6 is that anchors are *authorizing* data. A transaction can be fully signed before choosing an anchor, re-anchored without changing its txid, and can even spend a note from a parent transaction that is not yet mined. PCZT v2 (ZIP 374) carries all of these intermediate states.

### Cited Findings
**Anchor and witness**
- Consensus: "The `anchorIronwood` field … MUST refer to some earlier block's final Ironwood-pool treestate (as for `anchorOrchard`)." No age bound is stated. — [ZIP 258](https://zips.z.cash/zip-0258) (local `zips/zip-0258.md`, "Changes to the Protocol Specification")
- Zebra accepts an Ironwood anchor if it is in `parent_chain.ironwood_anchors` or `finalized_state.contains_ironwood_anchor(&anchor)`, meaning any historical final Ironwood root. There is no age check. — local `/Users/megabyte/Developer/zcash/zebra/zebra-state/src/service/check/anchors.rs:120-160`
- Recommended anchor depth stays at 3 blocks after NU7 (about 1.25 min at 25 s): "Anchor height depth is not intended to protect against" consensus splits or hashpower attacks. — [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md:586-606`)
- Zebra's `MAX_BLOCK_REORG_HEIGHT` is 1000. ZIP 218 recommends raising `MAX_REORG_LENGTH` from 99 to 600. — local `zebra/zebra-chain/src/parameters/constants.rs:33`; [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md:553`)
- Merkle depth is 32. — local `orchard-0.16.0/src/constants.rs:14`
- ZIP 326 notes in passing that "Incoming shielded funds cannot be spent in the same block in which they are received", because the anchor must be an *earlier* block's treestate. — [ZIP 326](https://zips.z.cash/zip-0326) (local `zips/zip-0326.md`, first-funds algorithm)

**v6 anchors are authorizing data (pre-authorization, re-anchoring, spending unmined notes)**
- "In version 6, for Sapling, Orchard, and Ironwood components, the anchor is instead part of the authorizing data … This lets the anchor be updated (re-anchored to a more recent note commitment tree root) without changing the transaction ID, while the proofs still bind to the specific anchor used." — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:375-395`)
- PCZT v2 rules:
  - "A shielded bundle anchor MAY be absent until proving … Setting or replacing a bundle's anchor MUST clear any proof fields."
  - "Signatures … do not commit to the anchors or witnesses … Signers MAY sign while they are absent."
  - Two workflows are enabled: **Re-anchoring** ("The transaction identifier is unchanged by re-anchoring") and **Spending unmined notes** ("every field of a v6 signature hash is computable for a transaction that spends an Orchard-protocol note created by a parent transaction that has not yet been mined, because Orchard-protocol nullifiers depend only on note data … Such a transaction can be constructed and fully signed before the parent transaction is even broadcast, with its witnesses, anchor, and proofs supplied after the parent is mined").
  - "Fields that remain effecting data — notably `Global.expiry_height`, `Global.fallback_lock_time` … and all values — are committed to by signatures … and can never change after signing."
  — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1365-1422`)
- The PCZT abstract says "Participants such as signers can be offline, as all necessary information is provided in the format". PCZT v2 "support[s] … v6 workflows in which a transaction is fully signed before its anchors, Merkle witnesses, and proofs are known". — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:45-58`)
- PCZT roles: Creator, Constructor, IO Finalizer, Updater, Redactor, Verifier, Prover, Signer, Combiner, Spend Finalizer, Transaction Extractor. `IronwoodBundle` has the same structure as `OrchardBundle`, with `note_version` MUST be `V3` and `flags` including cross-address. — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1297-1316,1474-1785`)
- The Prover "requires, per action: the spend's explicit note data (`recipient`, `value`, `rho`, `rseed`), `fvk`, `witness`, and `alpha`; the output's `recipient`, `value`, and `rseed`; and the action's `rcv` … creating them does not require spending key material". The Signer "MAY sign a PCZT whose shielded bundle anchors, witnesses, or proofs are absent … a spend whose witness is absent may be of a note that does not yet exist on chain". — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1594-1660`)
- PCZT privacy: "Delegating the Prover role reveals the private data of the notes being spent or created (but not spending authority) to the prover." — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:112-146`)
- ZIP 318 uses exactly this pattern for migration: "Migration transactions are pre-signed, persisted, and then have their proofs updated and are broadcast by best-effort background tasks". It also notes that proofs anchored to "shared, network-wide boundary heights" avoid "linking transactions via anchor selection". — [ZIP 318](https://zips.z.cash/zip-0318) (local `zips/zip-0318.md`, Abstract)

**Expiry (ZIP 203 and ZIP 218)**
- `nExpiryHeight` is a uint32 in {1 .. 499,999,999}, or 0 to disable expiry. "If the expiry is at block height N, then the transaction must be included in block N or earlier." Default: 20 blocks (pre-Blossom), then 40 blocks (Blossom). "At NU7 activation, ZIP 218 recommends … change the default expiry delta from 40 to 120 blocks" (about 50 min at 25 s). — [ZIP 203](https://zips.z.cash/zip-0203) (local `zips/zip-0203.rst:34-110`); [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md:529-540`)
- `TX_EXPIRING_SOON_THRESHOLD` stays at 3 blocks after NU7. — [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md:554`)

**Fees (ZIP 317)**
- conventional_fee = marginal_fee × max(grace_actions, logical_actions), with grace_actions = 2. Revision 1 (NU6.3) adds contribution_Ironwood = nActionsIronwood. The ZIP text still lists marginal_fee = 5000 zats. — [ZIP 317](https://zips.z.cash/zip-0317) (local `zips/zip-0317.rst:108-196`)
- Zebra 6.4.0+ mempool minimum is 1000 zats per logical action, per zcash/zips#1352. Mainnet `getstandardfee` reports 5000 until height 3,590,000, then 1000. Test networks report 1000. — local `zebra/CHANGELOG.md:65,110`

**Consensus branch ID binding**
- v6 txid and auth digests are personalised with the branch ID: `BLAKE2b-256("ZcashTxHash_" || consensusBranchId, …)` and `BLAKE2b-256("ZTxAuthHash_" || consensusBranchId, …)`. — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:358-373`)
- Zebra 7.0.0-rc.0: "Mempool peers are no longer penalized for relaying transactions with the previous or next network upgrade's consensus branch ID within about 50 minutes of any activation height". — local `zebra/CHANGELOG.md:69`

**Proving keys and parameters, cost on constrained devices**
- The Orchard and Ironwood circuit is Halo 2 at K = 11. — local `orchard-0.16.0/src/circuit.rs:76`
- zakura-orchard benchmarks: each fresh proving key involves "roughly 29 MiB of commitment-table preparation under the default no-orbits build (roughly 25 MiB with `orbits`)". — local `~/.cargo/registry/src/index.crates.io-*/zakura-orchard-2.2.0/benches/README.md` ("Prover" section)
- Measured proving on a Mac Studio M4 Max (dashpay fork PR):

  | Actions | Threads | Stock `halo2_proofs` 0.3.2 | Optimised fork | Verify (stock → fork) |
  | --- | --- | --- | --- | --- |
  | 2 | 2 | 1,349 ms | 646 ms | 11.5 → 7.3 ms |
  | 2 | 6 | 617 ms | 307 ms | 5.2 → 3.8 ms |
  | 6 | 2 | 3,612 ms | 1,613 ms | 18.0 → 8.9 ms |
  | 6 | 6 | 1,629 ms | 642 ms | 8.5 → 5.1 ms |

  — [dashpay/orchard PR #13](https://github.com/dashpay/orchard/pull/13)
- Mobile figures:
  - "Mobile proving … can currently take more than three seconds on modern iPhones."
  - Zakura Common (2026-08-29) claims "below 200 milliseconds in many cases". Sean Bowe cited "roughly 200 milliseconds" for a 2-input, 2-output transaction on an iPhone.
  — [CoinInsider, 2026-08-31](https://www.coininsider.org/news/zcash-proving-times-reach-around-200ms/) (secondary, vendor claim)
- A forum grant proposal states a baseline of "approximately 0.2–0.3 proofs per second" for k=11 Halo 2 on iPhone. That is an unsourced estimate, not a measurement. — [Zcash forum, 2026-07-19](https://forum.zcashcommunity.com/t/apple-silicon-zero-knowledge-proving-mobile-halo2-and-tachyon-acceleration-on-m-series-and-a-series-chips/56725/2)

**Serialized size**
- v6 field sizes:
  - Header block: 4+4+4+4+4 bytes.
  - `vActionsIronwood`: 820 bytes per action.
  - `flagsIronwood` 1, `valueBalanceIronwood` 8, `anchorIronwood` 32.
  - `sizeProofsIronwood` = "2720 + 2272 * nActionsIronwood".
  - Signatures: 64 per action, plus a 64-byte `bindingSigIronwood`.
  — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:167-236`)
- Ciphertexts: the Orchard-protocol note plaintext is `COMPACT_NOTE_SIZE` (lead byte 1 + d 11 + v 8 + rseed 32 = 52) plus a 512-byte memo. `ENC_CIPHERTEXT_SIZE` adds a 16-byte AEAD tag, giving 580. — local `orchard-0.16.0/src/note_encryption.rs:30-37`
- Maximum block size is 2,000,000 bytes. — [ZIP 218](https://zips.z.cash/zip-0218)

### Inferences
- **Size of a 1-input/2-output Ironwood-only v6 transaction.** One spend and two outputs need max(1,2) = 2 actions. My arithmetic from ZIP 229:

  | Part | Bytes |
  | --- | --- |
  | Header fields | 20 |
  | Six empty or count compactSizes (tx_in, tx_out, Sapling spends, Sapling outputs, Orchard actions, Ironwood actions = 2) | 6 |
  | 2 × 820 actions | 1,640 |
  | Flags | 1 |
  | valueBalance | 8 |
  | Anchor | 32 |
  | compactSize(7,264) | 3 |
  | Proof (2,720 + 2 × 2,272) | 7,264 |
  | 2 × 64 spend-auth signatures | 128 |
  | Binding signature | 64 |
  | **Total** | **9,166** |

  Without the proof, a signed but unproven transaction body is about 1,902 bytes. A PCZT adds private fields on top of that, and I found no size figure for PCZTs. Each further 2 actions adds 2 × (820 + 2,272 + 64) = 6,312 bytes. In ciphertexts alone, each action carries 660 bytes (580 + 80), so about 1.3 KB of the 9.2 KB is on-chain ciphertext that a Tachyon-style out-of-band design would remove.
- **Anchor staleness is not a validity problem for offline spenders.** A spender who last synced days ago can still make a valid proof against that old anchor, as long as the spent note was mined at or before the anchor block. That block must not have been reorged, which is negligible for blocks deeper than about 1000.
- **The real costs of an old anchor are privacy and freshness.** The anchor height reveals roughly when the wallet last synced, and may link transactions. ZIP 318 recommends shared boundary heights. A stale anchor also means the wallet cannot know whether its notes were already spent after that point (see Q4).
- **The branch ID is the hard epoch constraint.** A transaction signed under NU7 (`0x77190AD9`) becomes permanently invalid if a later upgrade activates before it is mined, and it must be re-signed. The txid and sighash commit to the branch ID, so re-anchoring cannot fix this. The offline signer must know the upgrade schedule. On testnet, no post-NU7 height is known.
- **Expiry choice trades three things against each other:**
  - The default of 120 blocks is about 50 min at the 25 s target, but about 2 h at the observed ~60 s testnet rate. Multi-hop mesh delivery could easily take longer.
  - `nExpiryHeight = 0` (no expiry) or a large value keeps the transaction valid for long store-and-forward delays. It also lengthens the period in which the sender's notes are "in limbo", and may fingerprint the wallet, since most wallets use the default delta.
  - A *cancellation* path still exists. The sender can spend the same note elsewhere, and the shared nullifier makes the two transactions mutually exclusive.
- **Fees:** use 2 × 1,000 = 2,000 zats if broadcasting only via Zebra 7.x testnet nodes, or 10,000 zats to satisfy the published ZIP 317 text and any stricter relays. A non-standard fee can itself be a fingerprint.
- **Proving on a Raspberry Pi or phone:**
  - No measured Raspberry Pi figure exists (see Gaps).
  - Desktop single-thread-ish proving of 2 actions is about 0.65–1.35 s on an M4 Max.
  - Phones are about 3 s or more today, or about 0.2 s with the newest Zakura stack (vendor claim).
  - A Raspberry Pi 4 or 5 is likely in the multi-second to tens-of-seconds range for 2 actions. This must be benchmarked; `cargo bench --bench orchard_k11_prover` and `ironwood_k11_prover` exist.
  - Memory is small, with key-preparation tables of about 25–29 MiB, but no full-process RSS figure was found.
- **Proof delegation needs care.** Keeping proving on the signer's side avoids leaking the FVK and note data. Delegating proving to a mesh-connected "prover node" would reveal the spender's note data and FVK, and an FVK reveals the account's full history.
- **Offline construction checklist:**
  - Spending key, or a signer holding ask.
  - FVK (ak, nk, rivk).
  - Each spent note's plaintext (d, pk_d, v, ρ, rseed) and its position.
  - A 32 × 32-byte Merkle path (about 1 KB) plus the matching anchor.
  - The recipient's Orchard-protocol receiver (from a UA or ZIP 321 URI).
  - Current height estimate (for expiry).
  - Branch ID.
  - Fee.
  - The Halo 2 proving key (generated locally, nothing to download).
  - A fresh OS RNG for rseed, alpha, rcv, esk and the dummy actions.

### Gaps
- No primary benchmark of Orchard or Ironwood proving time or memory on a Raspberry Pi or Android phone. The iPhone figures are vendor or estimate claims.
- No figure for the serialized PCZT v2 size of a 2-action Ironwood transaction, before or after redaction.
- I did not verify Zebra's mempool handling of `TX_EXPIRING_SOON_THRESHOLD`, or of transactions with `nExpiryHeight = 0`. That is zcashd-era behaviour.
- The upstream `pczt` crate version with PCZT v2 or Ironwood support was not confirmed. Locally only `zakura-pczt 0.1.0-rc3/rc4` is present.

## Q4. Recipient-side verification without chain access; out-of-band delivery (Tachyon), ZIP 321, ZIP 324

### Takeaway
Fully offline, a recipient can verify a lot:
- **Ownership and amount:** trial-decrypt the Ironwood output with their ivk, recompute and check `cmx`, and read the amount and memo.
- **Signatures:** verify the spend-authorization and binding signatures.
- **Proof:** if the transaction is proven, verify the Halo 2 proof against the locally generated verifying key.

What the recipient **cannot** verify offline:
- that the anchor is a real Ironwood treestate root on the best chain (this needs chain data or at least an authenticated header and history commitment);
- that the spent note's nullifier is not already on chain;
- above all, that the sender will not later broadcast a conflicting spend. That is a future event that no offline check can rule out.

So an offline Ironwood payment is a conditional claim until it is mined. Existing out-of-band designs do not solve this:
- ZIP 324 (Draft, Sapling-based) still requires on-chain "finalization".
- Tachyon's out-of-band payments and oblivious sync are an unshipped proposal.

ZIP 321 payment requests (Active) work offline for conveying the recipient's address.

### Cited Findings
- ZIP 326: the same ivk trial-decrypts both pools' ciphertexts. ZIP 218 notes wallets trial-decrypt with both internal and external ivks. — [ZIP 326](https://zips.z.cash/zip-0326) (local `zips/zip-0326.md:~95`); [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md`, bandwidth section)
- A PCZT Signer "SHOULD verify … that values match value commitments via `rcv`, and that ciphertexts of non-fabricated outputs decrypt to the explicit note data". The same consistency checks are available to a recipient that is given the PCZT or the note opening. — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1622-1660`)
- Proof public inputs bind the anchor: "proofs commit to the anchor as a circuit public input". — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1380-1388`)
- Consensus requires `anchorIronwood` to be an earlier block's final Ironwood treestate, and rejects a duplicate Ironwood nullifier: Zebra error `DuplicateIronwoodNullifier`, "ironwood double-spend: duplicate nullifier". — [ZIP 258](https://zips.z.cash/zip-0258); local `zebra/zebra-consensus/src/error.rs:200-201`; `zebra-state/src/service/check/anchors.rs:120-160`
- ZIP 221 history tree: from NU6.3 each history leaf carries `hashEarliestIronwoodRoot` and `hashLatestIronwoodRoot` (the final Ironwood treestate root) and `nIronwoodTxCount`. — [ZIP 258](https://zips.z.cash/zip-0258) (local `zips/zip-0258.md`, "Changes to ZIP 221"); local `zebra/zebra-chain/CHANGELOG.md:182` ("`primitives::zcash_history::V3` (the ZIP-221 Ironwood history node)")
- PCZT warns that signing without a witness means "a spend whose witness is absent may be of a note that does not yet exist on chain — the signature authorizes the spend regardless of whether the note is ever created". — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:1655-1660`)
- **Tachyon** (proposal, 2025-04-02):
  - "out-of-band payments where the sender and recipient use a separate channel for secret distribution. The on-chain ciphertexts can then be removed from the protocol entirely."
  - An "oblivious syncing service … isn't trusted with private information or secrets and learns nothing about the notes in the user's wallet". It can keep progressing "even when the user's wallet software is offline".
  - Wallets attach proof-carrying data that "effectively attaches evidence that the transaction is valid up until a certain recent point", so validators check only "that the most recent block(s) do not contain the revealed nullifier".
  - The post also notes the "payment request model that is supported by most Zcash wallets" already accommodates out-of-band payments, and links ZIP 324.
  — [Sean Bowe, Tachyon](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/)
- No source confirms Tachyon out-of-band payments or oblivious sync are live. One secondary report mentions Tachyon components migrating into Zakura Common. — [EdgeX/Coindoo](https://pro.edgex.exchange/en-US/news/article/zcash-integrates-tachyon-into-zakura-common) (secondary)
- **ZIP 324 "URI-Encapsulated Payments"** (Status: **Draft**, created 2019-07-17, owner Jack Grigg):
  - Format: `https://pay.withzcash.com:65536/v1#amount=…&desc=…&key=…`.
  - "The values of key and amount deterministically imply a unique payment note … which is a Zcash **Sapling** note".
  - "The sender's Zcash wallet app creates an ephemeral spending key, sends ZEC funds to the payment address derived from that key". The payment "is not deemed 'received' by the recipient until they execute a 'finalization' process".
  - The wallet "SHOULD convey … that the amount … may change (possibly to zero), until the finalization process has been completed".
  - Ephemeral keys are derived at `m_Sapling/324'/coin_type'/payment_index'`.
  - The testnet variant uses `TAZ` and `testzcash.com`.
  - The spec still contains "FIXME/TODO" items and does not mention Ironwood or Orchard.
  — [ZIP 324](https://zips.z.cash/zip-0324) (local `zips/zip-0324.rst`)
- **ZIP 321 "Payment Request URIs"** is Status **Active**. — [ZIP 321](https://zips.z.cash/zip-0321) (local `zips/zip-0321.rst` header); local `zcash-skills/docs/versioned-facts.md:25`

### Inferences
- **What a recipient can verify offline, given the full transaction bytes and the branch ID:**
  1. Decrypt `encCiphertext` with their ivk, recompute NoteCommit, and check it equals `cmx`. This proves the output pays them value v with memo m.
  2. Verify each spend-auth signature under `rk`, and the binding signature under the bvk derived from the `cv`s and `valueBalanceIronwood`. This proves value balance and that the holder of ask authorised the transaction.
  3. If proven, verify the Halo 2 proof. The verifying key is derived from the public circuit with nothing to download, and takes about 4–11 ms per 2-action bundle on desktop-class hardware.

  Together these prove *"if anchor A is a real Ironwood root, then the spent note(s) existed in the tree at A, the nullifiers are correctly derived, and value balances"*.
- **What the recipient cannot verify offline:**
  - (a) That A is a real root. An attacker can build a proof against a fake tree containing a fabricated note.
  - (b) That the nullifier is not already on chain, which needs the nullifier set as of now.
  - (c) That no conflicting spend will be mined first. This is inherently unverifiable: the sender holds ask and can sign another transaction with the same nullifier at any time.
- **A possible partial mitigation for (a) (my inference, not an existing tool):** if the recipient holds a PoW-validated header chain and the ZIP 221 MMR (relayed over the mesh), a sender could supply an MMR inclusion proof that A equals the `hashLatestIronwoodRoot` of some block. That gives SPV-style evidence that A existed on the best chain. It does nothing for (b) or (c).
- **Mitigating (b) and (c) needs economic or trusted mechanisms outside Zcash consensus.** Examples: amount caps, reputation or known parties, collateral or escrow, a trusted co-signer, or secure-element signers that refuse to sign two transactions spending the same note. Other researchers own this area.
- **Detecting a double-spend after reconnecting is easy.** The recipient knows the nullifiers in the transaction they were given. If any appears on chain in a different txid, the payment failed.
- **Offline payment chains (A→B→C) are possible with v6 and PCZT.** B can sign a transaction spending a note received in A's unmined transaction, because the nullifier is computable from the note data. But B cannot *prove* it until A's transaction is mined and a witness exists. C then holds a doubly conditional claim, and the risk compounds along the chain.
- **ZIP 324 does not suit offline settlement as written.** It is Sapling-only and Draft. The sender knows the ephemeral key and can sweep the funds back before the recipient finalises, and finalisation needs chain access. Its idea of carrying a note opening in a message is conceptually what a mesh payment would do.
- **Tachyon's out-of-band design maps well onto mesh delivery, but it is not available.** Today Ironwood still puts 660 bytes of ciphertext per action on chain. The recipient can still be given the note opening out-of-band, but scanning would find it anyway.
- **ZIP 321 URIs (`zcash:<utest-UA>?amount=…&memo=…`) are a natural offline invoice format** that fits in a QR code or a small mesh message.

### Gaps
- I found no existing tool or library that verifies an Ironwood anchor against a header chain or ZIP 221 MMR for offline or SPV use.
- No Tachyon specification or ZIP number, and no deployment timeline, was found beyond the 2025 blog post.
- Whether any wallet implements ZIP 324 (for any pool) in 2026 is unknown.

## Q5. Wallet libraries, SDKs and hardware that support Ironwood; compact-block sync bandwidth

### Takeaway
Ironwood support sits in the librustzcash "NU6.3 cohort":
- **Released versions:** orchard 0.15.3, zcash_primitives and zcash_proofs 0.30.0, zcash_keys 0.16.0, zcash_transparent 0.10.0.
- **Zebra's current pins:** orchard 0.16.0, zcash_primitives and zcash_proofs 0.31.0-pre.1, zcash_protocol 0.11.0-pre.0.
- **Zakura fork:** an equivalent `zakura-*` 2.2.0 crate family, including zakura-pczt 0.1.0-rc4.

The Ironwood bundle is built by the `orchard` crate with `ValuePool::Ironwood` / `BundleVersion::ironwood_v3()`. There is no separate `ironwood` crate. Downstream support:
- **Zallet** (beta, with PCZT RPCs), **Zaino** (≥0.6.0) and **ZODL**: confirmed.
- **Keystone:** firmware ≥ v3.0.2, Cypherpunk edition (reported).
- **Zingo and YWallet:** not confirmed.

Worst-case compact sync after NU7 is 169.10 MB/day per ZIP 218. The floor for empty blocks is about 0.31 MB/day of headers.

### Cited Findings
- Zebra 6.2.3 upgraded "the librustzcash crate cohort (`orchard` 0.15.3, `zcash_keys` 0.16.0, `zcash_primitives` 0.30.0, `zcash_proofs` 0.30.0, `zcash_transparent` 0.10.0) to the released NU6.3 versions". The NU6.3 pre-release wave was `orchard` 0.15.0-pre.1, `zcash_primitives` 0.29.0-pre.0, and so on. — local `zebra/CHANGELOG.md` (6.2.3 and 6.0.0-rc.0 sections)
- Current Zebra pins (Cargo.toml and Cargo.lock at HEAD 98303adb):
  - `orchard = "0.16.0"`
  - `zcash_primitives = "=0.31.0-pre.1"`, `zcash_proofs = "0.31.0-pre.1"`
  - `zcash_protocol = "=0.11.0-pre.0"`, `zcash_keys = "0.17.0-pre.1"`
  - `zcash_address = "0.14.0-pre.1"`, `zcash_transparent = "0.11.0-pre.1"`
  - `halo2_proofs 0.4.0`, `halo2_gadgets 0.6.0`, `sapling-crypto 0.9.0`, `zcash_note_encryption 0.5.1`
  - Comment in Cargo.toml: "Used to build Orchard/Ironwood bundles in test helpers; must match orchard's version".
  — local `/Users/megabyte/Developer/zcash/zebra/Cargo.toml:31-45`, `Cargo.lock`
- The orchard crate is multi-pool: `ValuePool::Ironwood` ("The Ironwood value pool") and "Uses the post-NU6.3 circuit for both the Orchard and Ironwood value pools". The note module defines the "quantum-recoverable Ironwood note plaintext version". — local `~/.cargo/registry/src/index.crates.io-*/zakura-orchard-2.2.0/src/lib.rs:180-209`, `src/note.rs:38`, `src/builder.rs:2494-2549`. Upstream `orchard-0.16.0` is also present locally.
- User-environment pins (Thus Spoke Zakura):
  - `zakura-{keys,primitives,proofs,orchard,sapling-crypto,transparent,zip321} = 2.2.0`
  - `zakura-client-{backend,sqlite} = 0.1.0-rc7`
  - `zakura-pczt = 0.1.0-rc4`
  - The shielded pool is "Ironwood; `orchard` rejected as destination pool post-NU6.3".
  — local `/Users/megabyte/Developer/zcash/zcash-skills/docs/versioned-facts.md:37,40` (user's own notes)
- Zallet v0.1.0-beta.1 added Ironwood (NU6.3) wallet support. v0.1.0-beta.3 (26 Aug) added PCZT JSON-RPC methods. — [Zallet releases](https://github.com/zcash/wallet/releases)
- Zaino 0.6.0 has full Ironwood support. The latest is 0.10.1 (29 Sep), which says "breaking changes take a minor bump under 0.x". — [Zaino releases](https://github.com/zingolabs/zaino/releases)
- ZODL: "Updating the ZODL wallet and SDKs for Ironwood" and one-click migration. — [ZODL post](https://zodl.com/ironwood-a-new-shielded-pool-for-zcash/)
- Keystone:
  - "If your Keystone firmware is below v3.0.2, you must upgrade it for Ironwood (NU6.3)". The Cypherpunk firmware is required for shielded ZEC with Zodl. — [ZODL support: Upgrading your Keystone for Ironwood](https://support.zodl.com/article/49-upgrading-keystone-firmware)
  - A third-party report says "3.0.2 added Ironwood support and batch PCZT signing", with 3.0.4 on Aug 12. — [Cointribune](https://www.cointribune.com/en/after-orchard-the-ironwood-hard-fork-of-zcash-already-receives-keystone-support/) (secondary)
  - ZIP 229 required that Keystone "continue to be able to sign v5 tx format Orchard → Transparent unshielding transactions, without firmware update". — [ZIP 229](https://zips.z.cash/zip-0229) (local `zips/zip-0229.md:150-152`)
- Compact sync bandwidth (ZIP 218, after NU7):
  - 148 bytes per compact Orchard or Ironwood action; about 90 bytes per compact block header; 3,456 blocks per day.
  - Worst case 330 × 148 × 3,456 + 90 × 3,456 = **169.10 MB/day** (Orchard or Ironwood bound), against about 270.5 MB/day before.
  - Headers alone: 0.31 MB/day. Worst-case trial decrypts: about 2.3 M per day.
  - Normal 2-action throughput is 165 transactions per block, or 6.6 TPS.
  — [ZIP 218](https://zips.z.cash/zip-0218) (local `zips/zip-0218.md:608-690`)

### Inferences
- In practice the "pool's own crate" is `orchard` (≥0.15.3), or `zakura-orchard` 2.x, used with `BundleVersion::ironwood_v3()`. Transaction assembly and serialization are in `zcash_primitives` (≥0.30) using the v6 format. Wallet scanning and storage are in `zcash_client_backend` and `zcash_client_sqlite` (or the zakura equivalents). Offline signing is via the PCZT crate, which must support PCZT **v2** for Ironwood and deferred anchors.
- Actual testnet compact-block bandwidth is probably far below the 169 MB/day worst case, because testnet shielded activity is low. Blocks are also arriving at about 60 s rather than 25 s, which gives about 1,400 headers per day, or about 0.13 MB/day. A basecamp that syncs once and relays only headers, the Ironwood commitment-tree frontier and its own witnesses over the mesh would need much less than full compact blocks. These are estimates, not measurements.
- Keystone's batch PCZT signing shows that air-gapped Ironwood signing via QR-transported PCZTs works in production. Its exact QR or UR payload limits were not researched here.

### Gaps
- I did not confirm Ironwood support in Zingo (zingolib or zingo-cli), YWallet, or the ZODL Android and iOS SDKs specifically, and no testnet builds of these were found.
- The upstream `zcash_client_backend`, `zcash_client_sqlite` and `pczt` crate version numbers that first supported Ironwood were not found; the local registry has only zakura variants.
- I found no measured daily compact-block bandwidth for the current testnet.

## Q6. Zcash-specific offline, mesh, LoRa or SMS payment proposals, grants or prototypes

### Takeaway
I found **no Zcash-specific mesh, LoRa, Meshtastic or SMS payment project, grant or prototype**. The closest Zcash-native work is:
- **ZIP 324**, URI-encapsulated payments over any messaging channel (Draft, Sapling, 2019);
- **Tachyon's** out-of-band payment and oblivious sync proposal (2025, not deployed);
- the **PCZT (ZIP 374) and ZIP 318** pre-sign, re-anchor and deferred-broadcast machinery, which is deployed in the migration tooling and Keystone.

Non-Zcash offline-mesh payment prototypes exist (for Bitcoin Lightning and Ethereum), but they are out of scope here and covered by other researchers.

### Cited Findings
- ZIP 324's goal: "sending Zcash payments via any unmodified (secure) messaging service … The sender need not know the recipient's address". It requires that "The protocol must not require the sender of a payment to stay online until the recipient receives the URI". It is still Draft. — [ZIP 324](https://zips.z.cash/zip-0324) (local `zips/zip-0324.rst`)
- Tachyon out-of-band payments and oblivious sync are a proposal. — [Sean Bowe, Tachyon](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/)
- ZIP 318 migration: pre-signed transactions are persisted, then proofs are updated and broadcast later by background tasks, with "Synchronization … kept decoupled in time from broadcast". — [ZIP 318](https://zips.z.cash/zip-0318) (local `zips/zip-0318.md`, Abstract)
- PCZT "Long-lived PCZTs (for example, pre-authorized transactions awaiting proving and broadcast)" must be protected like note databases, potentially "for days". — [ZIP 374](https://zips.z.cash/zip-0374) (local `zips/zip-0374.md:94-100,140-146`)
- A web search found no Zcash plus Meshtastic, sneakernet or LoRa project. Hits were for other chains: LNMesh (offline Lightning over BLE and WiFi mesh, FIU), ETHastic (ETHGlobal Cannes 2026, signed intents over Meshtastic to a gateway), MeshPay (Devpost), and BTC Mesh (Bitcoin Core plus Meshtastic). — [ETHastic](https://ethglobal.com/showcase/ethastic-0x22r), [MeshPay](https://devpost.com/software/meshpay), [Spark comparison](https://www.spark.money/tools/bitcoin-satellite-node-comparison), [FIU LNMesh](https://discovery.fiu.edu/display/pub279552)
- A Zcash forum grant proposal (2026-07-19) targets Apple-silicon and A-series Halo 2 and Tachyon proving acceleration, which is relevant to on-device proving at basecamps. Its milestones are proposed for 2026-10-15 to 2026-12-30. — [Zcash forum](https://forum.zcashcommunity.com/t/apple-silicon-zero-knowledge-proving-mobile-halo2-and-tachyon-acceleration-on-m-series-and-a-series-chips/56725/2)
- Zakura's experimental P2P v2 transport (iroh/QUIC) targets "mempool aggregation aimed at Project Tachyon" and "bandwidth-bound sync". It is off by default on Mainnet. — local `/Users/megabyte/Developer/zcash/zcash-skills/skills/zakura/SKILL.md:17`, `skills/zakura/references/node.md:71` (user's notes, secondary)

### Inferences
- This project appears to be novel within the Zcash ecosystem. The most reusable Zcash-native building blocks are:
  1. PCZT v2 as the wire format for partially built, pre-signed Ironwood transactions;
  2. v6 deferred anchors, so a basecamp with stale chain state can still produce valid transactions;
  3. ZIP 321 URIs for invoices;
  4. the ZIP 324 idea of sending a note opening (or a full transaction) to the recipient in a message.
- A 9.2 KB proven 2-action transaction is large for LoRa-class links. It is very likely to need fragmentation, and compression will not help because proofs and ciphertexts are high-entropy. Sending the unproven but signed form (about 1.9 KB plus private data) moves proving to whoever holds the FVK, with the privacy cost explained in Q3.

### Gaps
- I did not search the Zcash Community Grants and ZCG committee records in depth for mesh, offline or SMS grants. A broader web search found none, but a targeted search of forum.zcashcommunity.com grant threads is still needed.
- No Zcash forum threads on "offline payments", "sneakernet" or "SMS" were found with the search tool used.
