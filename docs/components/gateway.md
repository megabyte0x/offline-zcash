# Gateway submitter

The gateway is the one device with internet. It submits the transaction bytes it receives, returns a broadcast receipt, and later sends chain bundles back over the mesh: headers, the inclusion proof and recent anchors.
It holds no spending or viewing key and sees only raw tx bytes. It is still a trust point: it can withhold, delay or forge chain state.
In MVP v1 it is the laptop that runs `ths`, or a phone reaching that laptop through a port forward. Payee-side use of the bundles is in [payee-verifier.md](payee-verifier.md), and the chain endpoints are in [chain.md](chain.md).

## Decided

| Item | Rule | Status |
|---|---|---|
| Submission | lightwalletd `SendTransaction` (`127.0.0.1:9067`, plaintext) or Zakura JSON-RPC `sendrawtransaction` (`127.0.0.1:18232`) | [decided] |
| Raw RPC | `ths rpc` is not in v0.4.1, so call JSON-RPC directly with `curl` (issue log #5 in [AGENTS.md](../../AGENTS.md)) | [decided] |
| Input | Raw tx bytes only. No intent, payment details or payer identity reach relays or the gateway (gap G1, [reconciliation §5](../../archive/reports/Design%20review%20reconciliation.md)) | [decided] |
| Identity | Txid. Resubmitting the same bytes through any relay or gateway is harmless. Cache on txid + auth digest, because a re-anchored copy keeps the txid | [decided] |
| Returns | (1) a broadcast receipt for the txid; (2) after mining, the header + Merkle branch and later headers; (3) chain bundles with recent anchors and nullifiers for conflict checks | [decided] |
| Keys | None. Never broadcast from a node that also hosts a wallet | [decided] |
| Proving | Never on the gateway: a prover sees the note data ([offline-builder.md](offline-builder.md)) | [decided] |
| Loopback | `ths` binds every host service to `127.0.0.1` (issue log #3). Phones reach it through `socat TCP-LISTEN:9067,fork,bind=<lan-ip> TCP:127.0.0.1:9067`, `adb reverse tcp:9067 tcp:9067`, or an SSH tunnel, on a trusted network only (the dev keys are public) | [decided] |
| Transport (v1) | The payee forwards the bytes to the gateway as a separate BitChat private message; a packet addressed to the payee stops there ([transport-bitchat.md](transport-bitchat.md)) | [decided] |
| Mainnet (v2) | Submit over Tor to a public lightwalletd or Zebra endpoint; every send approved by the operator | [decided] |

## Findings

### Trust points (gap G4, [reconciliation §2](../../archive/reports/Design%20review%20reconciliation.md))

| Power | Effect | Mitigation |
|---|---|---|
| Withhold | The tx is never broadcast | Payer outbox resends; any other device with internet can submit the same bytes; expiry bounds the damage |
| Delay / time-shift | Releases the tx late, within expiry. With `nExpiryHeight = 0`, arbitrarily late | Finite fleet expiry; never use 0 |
| Forge chain state | Fake receipts, fake anchors or a fake "included" | Validate Equihash PoW on headers; cross-check bundles from ≥ 2 gateways. Headers still cannot prove heaviest chain ([payee-verifier.md](payee-verifier.md#what-headers-can-prove)) |
| First spy | Sees every tx first, with timing and its mesh origin | Fresh Tor circuit per tx, random delay, batching (mainnet) |
| Sees all txs | Can link fleet traffic (stale anchor, long expiry, bursts: gap G6) | Shared anchor heights; uniform fee |

- Projects tied to one operator's relay died with that operator (goTenna, Samourai pushTx). Any device with backhaul should be able to act as a gateway, and the design should run two. txTenna gave the sender no confirmation path, so receipts are mandatory ([report, Gateways settle](../../archive/reports/Offline%20Zcash%20over%20Logos%20mesh.md)).
- Inclusion proof size: a 1,487-byte header plus a Merkle branch, about 1.7 KB ([Zebra header length](https://doc-internal.zebra.zfnd.org/zebra_chain/block/header/constant.BLOCK_HEADER_LENGTH.html)).
- Bundles are broadcast rather than queried, which keeps the [ZIP 307](https://zips.z.cash/zip-0307) property that sync does not reveal interest. Compact blocks carry every Ironwood nullifier and commitment ([compact_formats.proto](https://github.com/zcash/lightwallet-protocol/blob/main/walletrpc/compact_formats.proto)). Full headers are about 5.1 MB/day at the 25 s target.
- Consensus rejects a duplicate Ironwood nullifier (Zebra `DuplicateIronwoodNullifier`, `zebra-consensus/src/error.rs` L200–201), so at most one of two conflicting versions is mined.
- Tor submission on mainnet follows Zodl's Tor mode: a fresh circuit per transaction ([Zodl Tor](https://support.zodl.com/article/17-enabling-tor-protection)).

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

- [unverified] A phone gateway through `socat` or an SSH tunnel to `ths` lightwalletd. Test: `SendTransaction` from an iOS build over Wi-Fi.
- [unverified] Gateway behaviour on resubmitting an already-mined or already-pooled txid (expected: a harmless error). Test both RPC paths on `ths`.
- [unverified] Payee restarts, then the gateway appears hours later and submits (open check 6, [MVP v1 design](../../archive/reports/MVP%20v1%20design.md)).
- [unverified] Receipt and bundle formats, and the key the gateway signs with, are not defined.
- [unverified] Bundle contents for v1: headers, recent anchors and nullifiers; the inclusion-proof format, including whether an auth-data path is sent ([payee-verifier.md](payee-verifier.md#what-headers-can-prove)).
- [unverified] Packet capture shows no plaintext amounts, payees or keys between the payee and the gateway (open check 5).
- [unverified] Mainnet: Tor client choice on iOS (Arti was named in the old plan), random-delay range, and public endpoint choice.
- [unverified] Full-node disk and RAM needs for a Zebra 7 mainnet gateway.
