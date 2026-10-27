# Trust minimisation: research log

How [design.md](design.md) was reached: the passes, the sources read and the options rejected. 27 October 2026. Five parallel literature and code reads covered chain verification, mesh privacy, private sync and uplink, identity and Sybil resistance, and distribution and device storage. Each pass then re-examined the weakest remaining row.

Labels: **[V]** read in the primary source at the cited revision; **[S]** secondhand (abstract, search excerpt, press); **[M]** measured in an orb; **[estimate]** our arithmetic.

## Pass 1: survey every row against existing work

Starting point: the v0.7.0 tables in [privacy-and-threats.md](../privacy-and-threats.md), the component docs and the code at v0.6.0 (`1586ccb`). Findings that changed the plan:

- **Verified compact blocks are possible on v6.** ZIP 244 splits the Orchard digest into compact (nullifier, cmx, ephemeralKey, `encCiphertext[..52]`), memos (`[52..564]`) and noncompact (cv, rk, `[564..]`, outCiphertext) digests. The compact one is "intended to be included in an updated version of the ZIP-307 CompactBlock format" [V: zips.z.cash/zip-0244]. ZIP 229 repeats this for Ironwood with `ZTxIdIrnActCH_v6`, `ZTxIdIrnActMH_v6` and `ZTxIdIrnActNH_v6` [V: zips.z.cash/zip-0229, zips `2bdf394`]. Nobody uses it: lightwallet-protocol v0.6.0 (`ec53075`) says CompactBlock `header` "should always be unset"; PR #15 added a header and was reverted; PR #21 (FlyClient) is open; Zaino 0.10.1 builds the header as `Vec::new()` (`compact_block.rs` L1152); lightwalletd v0.6.1 `parser/block.go` L110–117 [V].
- **v6 txids omit the anchor.** ZIP 229: "`orchard_digest_v6`, and `ironwood_digest_v6` omit the anchor" [V]. Anchor binding needs the auth-digest path; the design now carries it.
- **FlyClient** ([Bünz, Kiffer, Luu, Zamani, ePrint 2019/226](https://eprint.iacr.org/2019/226) [V]; the brief named other authors). Proof O(L + λ·log_{1/c}(n)·log₂ n). It is "only heuristically secure" under moving-average difficulty, as ZIP 221 L784–800 also says [V]. ZIP 221 L772–776 asks for "a more in-depth security analysis" before a FlyClient ecosystem [V]. Later work: Mining in Logarithmic Space (ePrint 2021/623, static difficulty [V]), variable-difficulty NIPoPoW claimed at CCS 2025 [S], Catching the Fly (arXiv 2604.26736, a Zcash FlyClient prover on zebrad, ≈ 1.2 MiB gzipped at 3 M blocks [V excerpts]).
- **NU7 averaging window is 102** (ZIP 218) and testnet minimum difficulty applies above 450 s, "strictly greater" [V]. The repo's docs say 17.
- **Equihash verify: 234 µs** with the `equihash` 0.2.0 crate on a 4-vCPU Xeon [M]; the docs' 96 µs came from a different orb.
- **No nullifier accumulator.** ZIP 221 L660–663 considered a nullifier-vector commitment and did not add it [V]. Tachyon's "shared evidence" design (29 September 2026) has per-epoch nullifiers and untrusted non-membership proofs but is not in NU7 (ZIP 259) and has no ZIP [V: seanbowe.com, tachyon.z.cash/roadmap].
- **Zebra proof RPCs.** Discussion #9196 (2025): "no plans to do so in the near term"; PR #9693 "Flyclient prover" draft, ZF 5 March 2026 "we want to get this in" [V]. Zakura v1.7.0 excludes FlyClient ("LC-SCOPE-05") [V].
- **Mesh privacy prior art** [V at the revisions below]:
  - BitChat `04995f6`: peer ID "is **not ephemeral**" (WHITEPAPER L39); the rotation code exists but is unused (announceV2 "Parsed and ignored on purpose", `BLEService.swift` L5982–5989); no cover traffic (`BitchatProtocol.swift` L44–45).
  - Briar `623b8b29`: 16-byte tags rotated every D + L with D = 86,400 s; old keys deleted (BTP L193–201).
  - Reticulum v1.5.5 `e40191b`: no source addresses; MTU 500; ratchets every 30 min; IFAC 8–16 B.
  - Meshtastic 2.8.2 `eaba8b9`: cleartext to/from, static X25519, no forward secrecy.
  - GAEN v1.2.2: identifier and address rotate "synchronously" every 10–20 min.
  - Becker et al. (PETS 2019): asynchronous payload rotation is trackable.
  - Givehchian et al. (S&P 2022): PHY fingerprinting from 10 packets, not fixable in firmware.
- **Private sync** [V unless marked]:
  - OMR ([ePrint 2021/1256](https://eprint.iacr.org/2021/1256) [S]; PerfOMR [2024/204](https://eprint.iacr.org/2024/204); UnifOMR [2026/910](https://eprint.iacr.org/2026/910), 31 MB detection key [S]).
  - FMD ([2021/089](https://eprint.iacr.org/2021/089); privacy critique, Seres et al. FC 2022 [S]).
  - PIR (YPIR [2024/270](https://eprint.iacr.org/2024/270), InsPIRe [2025/1352](https://eprint.iacr.org/2025/1352), valargroup `vote-nullifier-pir` `7c68579`).
  - ZIP 307: broadcast "leaks no information about which transactions the light client is interested in".
  - Zashi's Tor: str4d would "likely still not fetch CompactBlocks over Tor by default, as that's a broadcast channel".
- **Uplink** [V]: Arti 2.7.0 (1 October 2026), no stable mobile API; Iran January 2026 shutdown broke obfs4, Snowflake and WebTunnel (Tor forum); Nym mixnet live for Zingo and Zkool (24 September 2026). ZIP 204 `getheaders` returns ≤ 160 headers; Zebra serves peers; `zebra-network` has an Arti-based `isolated` API.
- **Identity** [V]: frost-core 3.0.0 `2016e44` DKG, refresh and repair; RFC 9591; ROAST (ePrint 2022/550); Briar BQP and introductions; Matrix v1.16 SAS and cross-signing; ZRTP RFC 6189; SybilLimit, Viswanath et al. SIGCOMM 2010, Mohaisen et al. IMC 2010; IETF KT architecture draft-09; MLS RFC 9420 §14; Ruffing–Kate–Schröder CCS 2015; zcash_script `d587822` (CLTV yes, CSV no).
- **Distribution and storage** [V]:
  - TUF 1.0.37; Uptane 2.1.0; SLSA (v1.2 current); GitHub artifact attestations (offline verification with a custom trust root); Zebra 6.0.0 reproducible images.
  - logos-package-manager `8e20ab6`: `trustedSigners` is a "SELF-ASSERTION"; the default policy is WARN.
  - KeyMint IKeyMintDevice.aidl: StrongBox lacks curve 25519; Secure Enclave only P-256.
  - Ledger shielded ZEC on Ironwood (23 September 2026); Keystone firmware 3.0.2 Ironwood PCZT [S].

Rows improved in pass 1: all chain rows (proofs), lightwalletd rows (P2P), all mesh privacy rows (rotation, link encryption, no sender, hints), organiser rows (on-device keys, threshold log), distribution rows (signing, rebuilds, TUF), storage rows (keystore wrap).

## Pass 2: weakest row was chain trust at BLE cost (B1, B3, L1)

Problem: the decided design needs either every header (≈ 5.1 MB per day) or a full FlyClient proof (the docs model 8.55 MB for 21 days) and still relies on two gateways agreeing, which a Sybil swarm defeats and one honest gateway cannot satisfy.

Steps:

1. **Count no sources; keep the heaviest verified chain.** One honest source suffices (the FlyClient and fraud-proof models both assume at least one honest full node: ZIP 221 Security; Al-Bassam, Sonnino, Buterin, arXiv 1809.09044 [V]). Dishonest sources only cost bandwidth.
2. **Optimistic ticks.** A tick (header + ancestry path, ≈ 7.5 KB [estimate]) is accepted only if it extends the last accepted tick. A forged tick costs one block of real work. FlyClient runs only on conflict or catch-up, so its cost is paid rarely.
3. **Work pricing for value.** Because FlyClient on Zcash is heuristic, value decisions require a contiguous suffix whose work exceeds κ × value. A forger must mine each suffix header; ≈ 150 headers protect the 3 ZEC pending ceiling at κ = 10 [estimate; it assumes the post-NU7 subsidy and an efficient hash-rental market, both unverified].
4. **Nullifier completeness is not needed.** A hidden prior spend helps only a double-spending payer, which is the accepted residual. Inclusion is provable by whoever wants it. This removes the main argument for agreement between gateways.
5. **Receive without scanning.** Mesh payments deliver the tx bytes to the payee, so only off-mesh discovery needs a chain scan. At 116 B per action (no nullifiers) a BLE device scans ≈ 8,000 actions per day per MB [estimate].
6. **Remove lightwalletd from the trust path.** Its compact blocks carry no header and `GetTreeState` returns only a frontier. A P2P header-and-block fetcher computes everything a proof needs and can use many peers. Testnet has one lightwalletd operator, so P2P also improves testnet diversity.

Result: ≈ 0.1–0.4 MB per device per day for chain trust in the common case [estimate], down from the 1.2 MB sync target plus unverified data.

## Pass 3: weakest rows were mesh metadata (A9, PO1–PO3, R3, S4)

Problem: removing the sender ID breaks per-sender quotas (v0.7.2) and the v0.6 rule that acks authenticate their sender; stable identifiers survive in the BLE hint, beacons and Hello.

Steps:

1. **Quotas by link and by recipient hint** instead of by sender. A flooder can exhaust only the stranger share near it, which is equivalent to jamming that share.
2. **Preimage acks** (Bitmessage's embedded ack pattern [V: protocol wiki]). Only the recipient can reveal the preimage, so acks stay unforgeable; they name nobody. This conflicts with the current AGENTS.md wording, so it goes to the owner.
3. **Rotation synchronous with the BLE address.** GAEN's rule and Becker et al.'s attack show payload and address must change together; links are dropped at rotation. Whether an Android advertising-set restart forces a new resolvable private address is unverified and becomes an M5 test.
4. **Secret-derived hints only.** BitChat's courier tag is computable from public keys (`PeerIDRotation.swift` L28–34 [V]); ours come from pair secrets (Briar BTP).
5. **Forward secrecy without round trips.** 12-hour signed epoch receive keys (Reticulum ratchets, Briar rotation) rather than X3DH prekeys (need a server to ration them) or Double Ratchet (needs round trips to heal).

## Pass 4: weakest rows were organiser custody and Sybil admission (O1–O3, G4, S2)

Steps:

1. **On-device keys and in-person pinning** (BQP-style QR, 128-bit commitment security).
2. **Credit decoupled from membership.** Pending goes only to payers the payee chose, so any admission authority, even a corrupt one, cannot create financial exposure.
3. **Gateways need no admission** once chain facts are self-verifying and submission goes to every gateway.
4. **Optional threshold fleet log** with t > 2n/3, so forking it needs 2t − n colluding members, and fork proofs that fit one frame. Global split-view detection across partitions is impossible by gossip alone (IETF KT draft-09: gossip is "only secure if … gossipping users … form a connected graph" [V]); because membership grants no credit, a split view harms only mesh priority. Anchoring log heads in a shielded memo (Keybase-style) was considered and left optional.

## Pass 5: weakest rows were distribution freshness offline and device storage (D1–D4, OS2, OS6)

Steps:

1. **Freshness by chain height.** TUF needs a clock to detect freeze attacks; Uptane says ECUs "SHALL have a secure source of time" and dropped its Time Server [V]. T1 gives every device a verified block height and median time past, so metadata can expire by height and a freeze becomes visible after N blocks.
2. **Mesh relay of metadata** (Uptane's online Primary pattern): gateways carry the metadata chain; devices verify root continuity and thresholds.
3. **Keystore wrapping** is the most any platform offers for Pallas keys; a hardware signer is the only way to keep the spending key off a general-purpose device.

## Final pass: what cannot be improved further

Each remaining residual was checked against "needs the accepted double-spend residual" or "needs an impossible property". The argument for each is in [design.md, Accepted residuals](design.md#accepted-residuals-and-why-each-is-unavoidable). In brief:

- **Hash-majority:** light verification cannot prove that no heavier chain exists.
- **Availability:** delivery through a medium where every node drops or jams is impossible.
- **Physical radio:** transmitting reveals presence and PHY identity.
- **Endpoint at signing:** someone must hold the authority to sign.
- **Software trust:** the user trusts whoever reviews the code.
- **RNG:** every key depends on it.
- **First entry into the internet:** some machine must send the transaction first.
- **Counterparty knowledge:** a person you pay knows you.
- **Optional fleet authority:** limited to admissions that confer no credit.

The one row whose improvement is blocked by missing research rather than impossibility is the **heuristic FlyClient bound**. The design avoids relying on it for value. If a proven variable-difficulty NIPoPoW or a recursive SNARK of the Zcash header chain becomes available, catch-up proofs could shrink to a few kilobytes (Plumo: 1.2–6.4 KB for one to five years of Celo, verified in about 0.5 s on a 2014 phone, ePrint 2021/1361 [V]). No such Zcash circuit exists; Equihash in a circuit would be expensive to prove but cheap to verify, and the prover would need no trust.

## Rejected options

| Option | Why rejected | Source |
|---|---|---|
| Two gateways must agree on the tip | Fails open under Sybils, fails closed with one honest gateway; replaced by heaviest verified chain | Pass 2 |
| Every header over BLE | ≈ 5.1 MB per day | 1,487 B × 3,456 [estimate] |
| Daily full FlyClient for every device | 0.8–1.3 MB per day each; used only on conflict or catch-up | FlyClient formula [estimate] |
| Trusting lightwalletd TLS (v0.7.0 interim) | No headers, frontier-only tree state, one testnet operator | lightwalletd `909dee8`, Hosh [V] |
| Running a full node on the gateway | Owner ruled out; not needed for proofs | Owner decision during v0.7.0 planning |
| OMR | Needs sender clues (a consensus change), 31 MB+ detection keys, 7–40 ms per message per recipient on the server | PerfOMR, UnifOMR, ZF evaluation [V/S] |
| FMD | Needs sender clues; with low traffic the anonymity set is near zero | Beck et al.; Seres et al. [V/S] |
| PIR from devices | 0.25–2 MB per lookup; scanning still needs every action | YPIR, InsPIRe, Hornby on ZIP 314 [V] |
| Sphinx / onion routing on the mesh | Flooding already shows every packet to every relay; no stable mixes; 1.5–1.9× airtime | Sphinx 2009; Nym `sphinx@fd64a73` [V] |
| Loopix-rate cover traffic | ≈ 1.5 MB per device per day before flood amplification | Loopix USENIX Sec 2017 [V] |
| Dandelion++ stem in the mesh | Weak against colluding relays (≈ 8 candidates at 15 % adversarial nodes); flooding plus no sender gives similar origin hiding | NDSS critique [S] |
| MLS group keys | Needs ordered commits from a delivery service; partitions break it | RFC 9420 §14 [V] |
| DCGKA | Sound for partitions but unnecessary for pairwise payments | ePrint 2020/1281 [V abstract] |
| X3DH/PQXDH prekeys | Need a server to ration one-time prekeys; PQ ciphertexts ≈ 1.6 KB; revisit before mainnet | Signal PQXDH spec [V] |
| Meshtastic-style DMs | Static keys, no forward secrecy | Meshtastic docs [V] |
| SybilGuard / SybilLimit / SybilRank / SybilInfer | Need the global graph and fast mixing; real graphs mix slowly | SybilLimit; Mohaisen et al.; Viswanath et al. [V] |
| BrightID, Encointer, Idena | Online graph analysis or global chains; only meetups and invitation quotas borrowed | [V excerpts / S] |
| Transitive web of trust | Graph leaks, certificate spamming (CVE-2019-13050); one-hop introductions only | [V] |
| Bonds with penalties | Need a CSV-style deposit; Zcash has CLTV only, transparent, linkable | zcash_script `d587822`; RKS CCS 2015 [V] |
| Witness quorums, transparent CLTV bonds (earlier research) | Do not stop an online self-sweep | [double-spend.md](../components/double-spend.md) |
| Storing Pallas keys in a secure element | No platform keystore supports Pallas | IKeyMintDevice.aidl; Apple SE docs [V] |
| Rotating the public APK test key | Anyone can forge a lineage from a public key; use a new applicationId | Android v3 docs [V] |
| Relying on Logos `trustedSigners` | Ignored by design; the keyring is the anchor | logos-package-manager `8e20ab6` [V] |
| BitChat 255-byte padding cap, public-key day tags | Leak size and are computable by anyone | BitChat `04995f6` [V] |

## Open questions carried into the plan

- Phone timing for Equihash, FlyClient verification and trial decryption on low-end Android.
- Mainnet proof sizes (MMR paths at the real leaf count, FlyClient sample count with and without DAA recomputation).
- Android behaviour when the advertising set restarts versus RPA rotation; iOS background limits on rotation.
- Whether Zebra peers accept Tor exits; testnet P2P peer count.
- Post-NU7 block subsidy and hash-rental prices, which set κ.
- Whether ZIP 318-style expiry and anchor buckets actually blend with other wallets' traffic.
- Logos IPC caller identity for the allow-list.
- Keystone and Ledger PCZT compatibility with our builder versions.
- One research snapshot (Hosh, about 26 September 2026) put testnet below the NU7 activation height; confirm activation before relying on testnet vectors.
