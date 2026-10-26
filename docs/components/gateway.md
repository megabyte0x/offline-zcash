# Gateway submitter

The gateway is the one device with internet. It submits the transaction bytes it receives, returns a broadcast receipt, and later sends signed chain bundles back over the mesh: the tip header, MMR paths for anchors, txid inclusion branches and conflict notices.
It holds no spending or viewing key and sees only raw tx bytes. The design treats it as untrusted (ADV-G in [privacy-and-threats.md](../privacy-and-threats.md)): it can withhold, delay or lie about chain state, so every chain fact the payee acts on is checked against proof of work and Merkle paths, never against the gateway's word.
The design target is mainnet. In the harness D is the Mac that runs `ths`; on mainnet it is any fleet device with backhaul, submitting over Tor. Payee-side use of the bundles is in [payee-verifier.md](payee-verifier.md), and the chain endpoints are in [chain.md](chain.md).

## Decided

| Item | Rule | Status |
|---|---|---|
| Trust | Untrusted on every network (ADV-G). The payee verifies every bundle fact itself; running on the local chain is never a reason to skip a check. The one harness exception is the `regtest-no-pow` flag ([chain.md](chain.md)), which disables only the Equihash, difficulty and timestamp check and is itself tested with mainnet header vectors | [decided] |
| Submission (harness) | lightwalletd `SendTransaction` (`127.0.0.1:9067`, plaintext) or Zakura JSON-RPC `sendrawtransaction` (`127.0.0.1:18232`) | [decided] |
| Submission (mainnet) | Over Tor, fresh circuit per transaction, to a public lightwalletd or Zebra endpoint; every send approved by the operator. Tor is part of the design, built after the loopback path works; it is not an optional later hardening | [decided] |
| Raw RPC | `ths rpc` is not in v0.4.1, so call JSON-RPC directly with `curl` (issue log #5 in [AGENTS.md](../../AGENTS.md)) | [decided] |
| Input | Raw tx bytes only. No intent, payment details or payer identity reach relays or the gateway (gap G1, [reconciliation §5](../../archive/reports/Design%20review%20reconciliation.md)) | [decided] |
| Identity | Exact-byte resends idempotent; cache/delivery dedupe on **(txid, auth_digest)**, because re-anchoring keeps txid. Family obligations are proposed (#3), not D's invoice/identity data | [decided, clarified] |
| Returns | (1) a broadcast receipt for the txid; (2) signed chain bundles (contents below) carrying the tip, anchor paths, inclusion proofs and conflict notices | [decided] |
| Bundle contents | Tip (height, hash, full header); `authDataRoot(N+1)` (32 B) for each anchor block N; MMR path from leaf N to `hashBlockCommitments` of header N+1 for each anchor in use; recent-anchor set (height → Ironwood root) for the fleet window; inclusion proofs (header + txid Merkle branch) for every txid the payee forwarded; targeted conflict notices; signature. Every slot is present from step 1 of the build order, empty when nothing applies | [decided] |
| Signing key | The gateway signs every bundle with its enrollment payment key, the same per-device Ed25519 scheme as payer and payee (role bit 4 in the enrollment record, [pending-tier.md](pending-tier.md)). Two conflicting signed tips from one key are portable evidence of misbehaviour | [decided] |
| Nullifier delivery | Targeted conflict notices by default: `nf → T′` with T′'s raw bytes (~9.2 KB) and its txid branch to a verified header, so the payee checks it like any inclusion proof. The full nullifier list for the fleet window on request, for audits and for payees that were out of range. Absence of a notice is not proof of absence | [decided] |
| History MMR | The gateway rebuilds the full ZIP 221/258 history tree from `getblock h 0` plus `z_gettreestate` per height, persists the leaves, rebuilds after a reorg (`ths reorg` / `ths rollback` in the harness) and computes anchor paths itself, because nodes keep only MMR peaks (issue log #9) | [decided] |
| Inclusion proofs | Txid branch from the `getblock h 1` txid list to `hashMerkleRoot` of a verified header; `authdigest` from verbose `getrawtransaction`. No authorizing-data path is sent: the v6 txid commits nullifiers, outputs, value balance and expiry ([ZIP 229](https://zips.z.cash/zip-0229)), which is all the payee needs | [decided] |
| Two gateways | The design runs ≥ 2 independent gateways. Where two are reachable, the payee requires both tips to agree. With one, the payee still verifies PoW and paths and enters `checked (single-source)` with the stricter cap ([payee-verifier.md](payee-verifier.md)) | [decided] |
| Keys | No Zcash keys. Never broadcast from a node that also hosts a wallet | [decided] |
| Proving | Never on the gateway: a prover sees the note data ([offline-builder.md](offline-builder.md)) | [decided] |
| Loopback (harness) | `ths` binds every host service to `127.0.0.1` (issue log #3). Phones reach it through `socat TCP-LISTEN:9067,fork,bind=<lan-ip> TCP:127.0.0.1:9067`, `adb reverse tcp:9067 tcp:9067`, or an SSH tunnel, on a trusted network only (the dev keys are public) | [decided] |
| Transport | The payee forwards the bytes to the gateway as a separate BitChat private message; a packet addressed to the payee stops there ([transport-bitchat.md](transport-bitchat.md)) | [decided] |

Build order (a sequence, not a security downgrade; each step has gating tests in [open-work.md](../open-work.md)): (1) bundle format with every slot, bundle signature, anchor-in-window, expiry margin, txid branch and the MMR anchor path (verifier exists: 15 lines in [`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md)); (2) Equihash and difficulty checker with the pinned floor and mainnet vectors; (3) two-gateway agreement and single-source degradation. The earlier "Mainnet (v2)" row that deferred Tor and the operator approval to a later stage is superseded by the rows above.

## Findings

### Trust points (gap G4, [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md))

| Power | Effect | Mitigation |
|---|---|---|
| Withhold | The tx is never broadcast | Payer outbox resends; any other device with internet can submit the same bytes; expiry bounds the damage |
| Delay / time-shift | Releases the tx late, within expiry. With `nExpiryHeight = 0`, arbitrarily late | Finite fleet expiry; never use 0 |
| Forge chain state | Fake tip, fake anchors or a fake "included" | The payee checks Equihash and difficulty against a pinned floor from a setup checkpoint, MMR paths for anchors, txid branches for inclusion; ≥ 2 gateways must agree on the tip. Headers still cannot prove heaviest chain ([payee-verifier.md](payee-verifier.md#chain-state-checks-no-trust-in-the-gateway-decided)) |
| Omit a conflict | Hides that a nullifier was spent elsewhere | Not detectable from one gateway (completeness). ≥ 2 independent gateways for the same window, payee-to-payee nullifier sharing ([pending-tier.md](pending-tier.md)), loss caps |
| Equivocate | Different tips to different payees | Signed bundles: two conflicting tips under one enrollment key are portable evidence; payees compare tips when they meet |
| First spy | Sees every tx first, with timing and its mesh origin | Fresh Tor circuit per tx, random delay, batching |
| Sees all txs | Can link fleet traffic (stale anchor, long expiry, bursts: gap G6) | Shared anchor heights; uniform fee |

- Projects tied to one operator's relay died with that operator (goTenna, Samourai pushTx). Any device with backhaul should be able to act as a gateway, and the design runs two. txTenna gave the sender no confirmation path, so receipts are mandatory ([report, Gateways settle](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).
- Bundles are broadcast rather than queried, which keeps the [ZIP 307](https://zips.z.cash/zip-0307) property that sync does not reveal interest. Compact blocks carry every Ironwood nullifier and commitment (`ironwoodActions`, [compact_formats.proto](https://github.com/zcash/lightwallet-protocol/blob/main/walletrpc/compact_formats.proto) L74, L131–132).
- Consensus rejects a duplicate Ironwood nullifier (Zebra `DuplicateIronwoodNullifier`, `zebra-consensus/src/error.rs` L200–201), so at most one of two conflicting versions is mined.
- The fresh-circuit-per-transaction rule is **our design requirement**, not verified behavior of every wallet broadcast. Reuse SDK isolation/fail-closed plumbing but test D's actual send/retry path; Zashi's documented per-txid isolation concerned enhancement requests ([prior art](../prior-art.md)). Tor does not hide bytes/ingress timing from D itself.
- Pinned Zakura rejected later-expiry same-input renewal while the original sat in its mempool; it accepted/mined the replacement after expiry eviction. Long expiry guarantees neither resident retention nor mining. Compatible re-anchoring keeps txid/signatures but needs new witnesses/proof on A; D cannot refresh expiry/branch without authorization ([expiry/re-issue findings](expiry-reissue.md)).

### Bundle sizes (mainnet, 3,456 blocks/day at the 25 s target)

| Item | Size | Basis |
|---|---|---|
| Mainnet header | 1,487 B ([Zebra header length](https://doc-internal.zebra.zfnd.org/zebra_chain/block/header/constant.BLOCK_HEADER_LENGTH.html)); Regtest 177 B | `getblockheader h false`, measured on `ths` v0.4.1 |
| All headers for one day | ≈ 5.14 MB; ≈ 5.6 MB with leaf extras. ~4.5–5 min per BLE hop at 19 KB/s; ~125 private messages of 45 KB, while BitChat's outbox holds 100 | Arithmetic from header size; BLE rate from [transport-bitchat.md](transport-bitchat.md) |
| Tip + paths for 3 anchors | ≈ 15–25 KB | [unverified: modelled at 290 B/node, not measured on mainnet] |
| MMR anchor path | 3–9 KB per anchor on mainnet [unverified, modelled]; 1,109–2,778 B measured on Regtest | [`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md) |
| Inclusion proof | header + txid branch, 5–11 KB on mainnet [unverified] | branch depth depends on block size |
| Conflict notice | ≈ 10 KB (T′ bytes 9,166 B + branch) | [chain.md](chain.md) tx size |
| Signed checkpoint | ≈ 300 B | header + signature |

So the payee receives the tip header plus the work since the last checkpoint sampled, not every header. Daily nullifier volumes used in planning (2k / 20k / 200k actions a day) are assumptions, not measurements.

### RPC and library facts (Zakura 1.6.0 `ebda15e`, lightwalletd, measured against `ths` v0.4.1 on 2026-10-25)

- No RPC returns a Merkle path: `gettxoutproof` is "Method not found", and nothing returns a ZIP 221 history path or leaf list. Zakura keeps only MMR peaks (`zakura-state/src/service/finalized_state/history_tree.rs` L71–85, `prune()` L373–445); Zebra likewise (`zebra-chain/src/history_tree.rs` L282, L353). `zakura-header-chain` has a test that forbids a flyclient surface (`lib.rs` L160–247). Hence the gateway rebuilds the tree itself (issue log #9).
- `zcash_history` has no proof API: `tree.rs` only appends, truncates and returns the root. Zakura Common 2.2.0 has no history crate; Zakura v1.6.0 pins upstream `zcash_history = "0.5.0"` (Zakura `Cargo.toml` L54), which has V3 Ironwood leaves (`node_data.rs` L229–256: `start_ironwood_root`, `end_ironwood_root`, `ironwood_tx`; `version.rs` L183–220 defines `V3`). Upstream 0.6.0 is the same; Zebra main uses 0.6.0 with `IronwoodOnward` (`zebra-chain/src/history_tree.rs` L21, L55).
- Leaf construction: Zakura `zcash_history.rs` L494–508. `anchorIronwood` of a tx equals the leaf's `end_ironwood_root` for block N (start equals end in a single leaf, [ZIP 258](https://zips.z.cash/zip-0258) L176–211). `z_gettreestate` returns the Sapling root byte-reversed; only that combination reproduced the committed roots.
- `hashBlockCommitments = BLAKE2b-256("ZcashBlockCommit", historyRoot ‖ authDataRoot(N+1) ‖ 0³²)`: Zakura `block/commitment.rs` L301–320 and `zakura-state/src/service/check.rs` L321–335 (the check uses the previous block's tree). The `ZcashAuthDatHash` tree is in `merkle.rs` L374–405. The history tree resets at each upgrade activation (`history_tree.rs` L313–321).
- `getblockheader h false` returns 177 B on Regtest and 1,487 B on mainnet; the verbose form includes `blockcommitments` (`methods.rs` L2209–2227). Verbose `getrawtransaction` returns `authdigest` (`zakura-rpc/src/methods/types/transaction.rs` L70–73).
- lightwalletd never fills `CompactBlock.header` (proto L28–41; `parser/block.go` L110–125) and its `GetTreeState` returns only the frontier string, not the root (`service.proto` L178–185; `frontend/service.go` L429–436). The gateway therefore needs node JSON-RPC, not only lightwalletd. [unverified: lightwalletd read in code only, not run]
- Measured: all 131 rebuilt history roots matched header N+1's `hashBlockCommitments`; the txid Merkle root matched the header for 132/132 blocks; two real Ironwood anchors proved by MMR path. The prototype is 277 lines (payee verifier 15, gateway MMR with paths 75, leaf fetch and `authDataRoot` 54). Production estimate [unverified]: payee 100–150 lines, gateway 300–500 lines.

### Logos phase facts

These are for later; the logos-co modules play no part in MVP v1 ([logos_zcash_modules.md](../../archive/research_notes/Offline%20Zcash%20over%20Logos%20mesh/logos_zcash_modules.md)).

- `logos-zebrad-module` ([`a681945`](https://github.com/logos-co/logos-zebrad-module/tree/a68194507241dd03c633752480f9a8e39cbe1b79)) runs upstream Zebra **v7.0.0** in-process, plus lightwalletd v0.5.5, built by `logos-zebra-nix` (`0b5e2ac`, `nix/zebra-src.nix` L6–13). Four patches, none in consensus, chain, state or network code.
- `grpc()` forwards every `CompactTxStreamer` path except `GetMempoolStream`, so `SendTransaction` passes (`zebrad-c/src/grpc.rs` L64–111). A relay module must be added to `callers.json`; by default only the wallet core is allowed (README L87–92).
- It is a full node on the gateway. Storage is about 300 GB [unverified: no source in the archive; the notes say chain storage size was not researched]. Basecamp needs 4 GB RAM minimum, 8 GB recommended ([Install Basecamp](https://docs.logos.co/basecamp/install-logos-basecamp)).
- The wallet core (`21b9f06`) combines signing and broadcast in one `sign_and_send` job (`send.rs` L326–382, `engine.rs` L495–545). It returns only `{txid, accepted, server, error}`, exports no raw tx or PCZT, accepts no raw tx to broadcast, hard-codes expiry at target height + 40, and broadcasts to its configured remote lightwalletd servers in turn.
- `logos-zcash-node-module` routes calls (LAN direct, Tor by default, fail closed) but does not broadcast. Its two-operator `confirm_elsewhere` cross-check is a pattern to reuse.
- The wallet core accepts a server only if it reports chain `regtest` or `"test"` and a branch matching tip or tip+1 (`sync/driver.rs` L462–472). Zakura reports `"test"`, so it should pass; untested.
- These modules use upstream librustzcash (`zcash_primitives` 0.31, `orchard` 0.16). Cross-stack acceptance is unverified (issue log #4).

## Open

- **Next design problem #4:** specify gateway wire protocol and signature coverage, submission ACK versus inclusion, retry/idempotency and ordinary tip lag versus competing forks. Problem #3 repair/family/config proposals remain unratified; no such protocol is implemented. Gateway duties from problem #2 stay decided ([open-work.md](../open-work.md)).
- [unverified] A phone gateway through `socat` or an SSH tunnel to `ths` lightwalletd. Test: `SendTransaction` from an iOS build over Wi-Fi.
- [unverified] Gateway behaviour on resubmitting an already-mined or already-pooled txid (expected: a harmless error). Test both RPC paths on `ths`.
- [unverified] Payee restarts, then the gateway appears hours later and submits (open check 6, [MVP v1 design](../../archive/reports/MVP%20v1%20design.md)).
- Bundle wire format (field order, encoding, signature coverage) is not written down; the contents and the signing key are decided above. Write it before build-order step 1.
- [unverified] Packet capture shows no plaintext amounts, payees or keys between the payee and the gateway (open check 5).
- [unverified] Tor client on iOS (Arti was named in the old plan), random-delay range, and public endpoint choice. Needed for the mainnet submission path, not blocked by the harness.
- [unverified] Difficulty floor pinning: how the setup checkpoint fixes the minimum `nBits` the payee accepts, and how often it is refreshed. An attacker who can mine above the floor but below mainnet difficulty could still fake a tip; the floor must track mainnet closely.
- [unverified] Equihash (200,9) verify time on Mac and iPhone. 96 µs/header on an orb Xeon ([`poc/mmr-anchor-poc`](../../poc/mmr-anchor-poc/README.md) `eqbench`), ≈ 0.33 s per day of headers; Apple hardware not measured.
- [unverified] Mainnet proof sizes (MMR path, txid branch); only Regtest sizes are measured.
- [unverified] Zakura's history-tree commitment in the NU7 activation block on mainnet (the tree resets at activation, `history_tree.rs` L313–321); whether an anchor from before activation can be proved needs a mainnet-like chain (issue log #1).
- [unverified] Zebra 7 RPC behaviour for `getblock h 0`, `z_gettreestate` and verbose `getrawtransaction`; only Zakura was run.
- [unverified] Full-node disk and RAM needs for a Zebra 7 mainnet gateway.
- Gating tests for the gateway: rebuilt history roots match every header's `hashBlockCommitments`, including after `ths reorg` and `ths rollback`; the rebuilt root agrees with `zcash_history::Tree`; a txid branch verifies for a block with 3 transactions and fails for the wrong txid or position; a tampered bundle and a bundle signed by an unknown key are rejected by the payee; one lying gateway of two is detected.
