# Trust minimisation: implementation plan

Plan for [design.md](design.md). Status: proposal, 27 October 2026; nothing here is built. It starts after v0.7.2 and assumes v0.7.0–v0.7.2 land as briefed: Common 4.0 and the testnet light-client gateway (v0.7.0), the UI pass (v0.7.1), and the direct fixes (v0.7.2: 0600 files, `verify_strict`, store bounds and quotas, authenticated inventory, reorg-aware `mined`, SPKI pinning, `Sub` quotas, request-nonce `notify`, SHA-pinned Actions, Android cleartext off). Where a milestone builds on one of those, it says so.

Effort is in person-weeks for one engineer familiar with the code, **[estimate]**, excluding review and field tests unless stated. Paths: `core/` = `crates/zcash-offline-core/src/`, `mesh/` = `crates/ozm-mesh/src/`, `node/` = `crates/ozm-node/src/`, `kt/` = `android/app/src/main/java/org/zcash/offline/`.

## Order and what ships first on testnet

```diagram
M0 specs + vectors
 │
 ├──▶ M1 chain proofs (verifier) ──▶ M2 P2P light gateway ──▶ M3 ticks, FlyClient, work pricing ──┐
 │                                                                                                 │
 ├──▶ M4 social identity (on-device keys, pinning) ──▶ M7 bounded credit (pending to pinned only) ─┤
 │                                                                                                 ├──▶ M10 mainnet gate
 ├──▶ M5 quiet mesh wire v2 ──▶ M6 forward secrecy (epoch keys) ───────────────────────────────────┤
 │                                                                                                 │
 └──▶ M8 verifiable distribution ──▶ M9 sealed device + hardware signer spike ─────────────────────┘
```

**First testnet release (proposed v0.8.0):** M1, M2 (laptop gateway), M4 and M5 phase A (link encryption and rotating identifiers). These remove the three largest v0.7.0 risks: one lightwalletd as the whole chain, organiser key custody and static mesh identifiers. **Second (v0.9.0):** M3, M5 phase B, M6, M7. **Third (v0.10.0):** M8 and M9. **M10** is the mainnet gate.

Total ≈ 39–55 person-weeks for M0–M9 **[estimate]**, plus the existing v1.1 co-signer work and field tests; three engineers working on independent tracks (chain, mesh/identity, distribution/device) ≈ 5–6 months to M10.

## M0. Specifications and test vectors (2–3 weeks)

- Write `docs/trust-minimisation/wire-v2.md` with byte layouts for: packet v2, Noise link framing, tick, FlyClient proof, work-priced suffix, inclusion + auth-data proof, verified compact tx, epoch receive key, preimage ack, person/device certificate, fleet log head and fork proof, TUF-lite `release.json`.
- Collect vectors: real testnet and mainnet headers (Equihash pass/fail), NU7 testnet history leaves with Ironwood roots, a block with ≥ 3 txs for txid and auth-data branches, ZIP 244/229 txid recomputation from the compact form for a 2- and a 4-action tx.
- **Gate:** each vector reproduces with an independent implementation: `zcash_history`, `zcash_primitives` txid code, or Zebra RPC output.
- **Risk:** testnet NU7 history leaves; one research snapshot (Hosh, about 26 September 2026) showed testnet below the activation height, so confirm NU7 is active on testnet before relying on vectors.

## M1. Chain proofs in the verifier (5–7 weeks)

Files: new `core/chain/` module (`header.rs`, `equihash.rs`, `daa.rs`, `mmr.rs`, `txid.rs`, `proofs.rs`); `core/verify.rs` (anchor and inclusion checks); `core/wallet.rs` (birthday frontier check, witness root check); `core/messages.rs` (new bodies); `node/app.rs` (`on_sync_res` stops applying unverified blocks).

- Header parsing for testnet/mainnet (1,487 B) and Regtest (177 B), Equihash (200,9) with the `equihash` crate, `regtest-no-pow` flag (decided earlier, [chain.md](../components/chain.md)).
- DAA with the NU7 averaging window of 102 and median-time span 11 (ZIP 218), testnet minimum-difficulty rule (> 450 s), and the half-difficulty floor.
- MMR node format V3 (277–317 B, ZIP 258 fields), path verification (from the 15-line PoC verifier), ancestry and anchor proofs.
- txid recomputation from the compact form with the Ironwood sub-digests; txid branch; auth-digest branch to `hashAuthDataRoot` and `hashBlockCommitments`.
- Payee: anchor check (check 5), `included` from an inclusion proof, never from a gateway result. Payer: received notes accepted only with inclusion; birthday frontier checked against the leaf's `end_ironwood_root`.
- **Wire:** new `Body` variants `Proof { kind, bytes }`; `SyncRes` survives only as an untrusted scanning hint.
- **Tests:** a fabricated-anchor tx is refused (open test 1); a flipped leaf, peak, `authDataRoot`, txid or position fails; a corrupted mainnet header fails Equihash; a testnet min-difficulty block is accepted only under the testnet rule; a forged `SyncRes` note never becomes spendable.
- **Gate:** tests 8–16 of [open-work.md](../open-work.md) pass on Regtest, plus the testnet header vectors.
- **Risk:** Common 4.0 type changes from v0.7.0; history-tree reset at activation means proofs start at NU7 (testnet 4,465,026), with the release checkpoint after it.

## M2. P2P light gateway (6–8 weeks)

Files: new crate `crates/ozm-zcash-p2p` (minimal Zcash P2P client: `version`/`verack`, `ping`, `getheaders`/`headers`, `getdata`/`block`/`tx`, `inv`, `addr`); `node/gateway/` (header store, frontier and history-leaf builder, proof server, submitter); `node/app.rs` (replace the lightwalletd client from v0.7.0 as authority).

- Headers-first from ≥ 8 peers chosen across address groups; keep the heaviest verified chain; ban peers that send invalid PoW.
- Whole blocks since the checkpoint; maintain Sapling, Orchard and Ironwood frontiers; build history leaves and check them against `hashBlockCommitments` (as the PoC did against node RPC: 131/131 roots matched on Regtest).
- Proof server: inclusion, auth-data, anchor, verified compact tx, frontier; later ticks and FlyClient (M3).
- Submission to ≥ 3 random peers; rebroadcast every 3 blocks; detect inclusion from fetched blocks.
- Tor: `arti-client` with one `IsolationToken` per peer connection and per submission; Tor mandatory on mainnet, optional on testnet (owner decision 5 decides the blocked-Tor behaviour).
- lightwalletd/Zaino stays as an optional block source behind the same verification.
- **Why a new crate rather than `zebra-network`:** `zebra-network` pulls `zebra-chain`, a different type family from Zakura Common; serialized bytes cross families (measured in #7), types do not. Re-evaluate if a slim build is possible.
- **Tests:** a peer serving a forged header chain is out-voted by work; a withheld block from one peer is fetched from another; history roots match headers on testnet for ≥ 10,000 blocks; a submitted tx is seen mined via blocks only.
- **Gate:** a laptop gateway with no lightwalletd and no full node syncs testnet from the checkpoint, serves proofs to two phones over BLE, and a payment reaches `included` from proofs alone.
- **Risk:** testnet P2P peer count; whether Zebra rate-limits or bans Tor exits [unverified]; block download volume on mainnet during spam waves (internet side only).

## M3. Ticks, FlyClient and work-priced decisions (4–6 weeks)

Files: `core/chain/flyclient.rs`, `core/chain/tick.rs`, `core/verify.rs` (work pricing), `node/gateway/` (prover), `mesh` broadcast class for ticks.

- Tick broadcast (header + ancestry path), accepted by extension only; conflict detection; FlyClient request on conflict or after > 1 day offline.
- FlyClient prover and verifier over the ZIP 221 tree with Fiat–Shamir sampling, λ = 64, 100-header suffix, labelled heuristic (owner decision of 26 October); the DAA option (recompute from predecessor leaves) behind a flag for measurement.
- Work pricing: `included` requires a contiguous suffix with work ≥ κ × value; testnet min-difficulty blocks count zero.
- **Tests:** an eclipsed device fed fake ticks switches to the honest chain when one honest tick arrives; a cheaper fork loses a FlyClient comparison; a payment above the work-priced bound stays `submitted` until enough work accrues.
- **Gate:** measured proof sizes and phone verification time (Android) recorded; per-device daily bytes ≤ 0.4 MB with ticks every hour.
- **Risk:** heuristic security (ZIP 221 L784–800); mainnet proof sizes unmeasured until a mainnet run; Equihash time on low-end phones [unverified].

## M4. Social identity (4–5 weeks)

Files: `core/fleet.rs` (person key, device certificates, fleet log), `node/lib.rs` (`fleet-init` and `join` without private keys), new `core/pairing.rs` (QR commitment + BLE confirmation), `kt/` pairing screen, Logos Organiser app (`logos/zcash_offline_organiser/qml`), module API.

- On-device key generation everywhere; invites carry public keys and a certificate request.
- Person key signs device keys; revoke a device without a new person key.
- In-person pairing: QR commitment (BQP pattern) with a SAS fallback; pins stored with petnames.
- One-hop introductions, labelled until confirmed in person.
- Optional fleet: single organiser signing key first (contained), then FROST DKG with t > 2n/3 and ROAST signing, a hash-chained log and fork proofs.
- **Wire:** enrollment record v2 (no private keys), certificate 190 B, log head 112 B, fork proof 224 B.
- **Tests:** an invite file contains no secret (grep test); a stranger cannot obtain pending acceptance; a forged certificate and a forked log head are rejected and the fork proof propagates; introductions show their label.
- **Gate:** a three-person fleet formed at a meetup with no organiser key custody pays over BLE.
- **Risk:** UX of QR pairing on laptops without cameras (SAS fallback); migration of v0.7.x fleets (re-enroll required).

## M5. Quiet mesh, wire v2 (6–8 weeks, two phases)

Files: `mesh/link.rs` (Noise via the `snow` crate), `mesh/packet.rs` (v2 header), `mesh/node.rs` (quotas by link and hint, preimage acks, jitter), `mesh/discovery.rs` (rotating tags), `kt/BleLink.kt` and `crates/ozm-ble/src/linux.rs` (rotating service data, reconnect at rotation), `core/envelope.rs` (hints, size classes).

**Phase A (testnet first, 3–4 weeks):** Noise XX on TCP and GATT; rotating BLE hint, beacon tag and Hello ID every 10–20 minutes with links dropped at rotation; fleet header mask.

**Phase B (3–4 weeks):** packet v2 without `sender`, with random initial TTL and an expiry bucket instead of `created_ms`; recipient hints (pairwise tags, trial-decrypt class); preimage acks; fixed 512 B cells and two size classes; jitter; link-local decoys.

- **Packet v2 header:** version 1, type 1, ttl 1, flags 1, msg_id 16, hint 4, expiry bucket 1, ack_hash 32 (messages only), length 2, then payload. 27 or 59 B against today's 48 B.
- **Tests:** a packet capture over TCP and over BLE (B7/D5 sniffer) shows no stable identifier across two rotations; a relay cannot link two messages to one recipient across epochs; a forged ack (wrong preimage) deletes nothing; a stranger flood is confined to its quota while contact traffic still delivers; existing mesh/LAN/FFI benches pass.
- **Gate:** the bench suite plus a two-phone BLE run with Android RPA rotation observed. Whether restarting the advertising set forces a new Android address is [unverified] and is tested here.
- **Risk:** reconnect cost every rotation on BLE (≈ 15 s cold, [estimate]); iOS background advertising cannot change service data, which limits rotation on iOS.

## M6. Forward secrecy (2–3 weeks)

Files: `core/envelope.rs`, `core/fleet.rs` (epoch keys), `node/app.rs` (publish and retire keys).

- Signed X25519 receive key per 12-hour epoch inside presence; private halves deleted after two epochs; envelopes seal to the recipient's current epoch key; stale-key fallback to the static key only for first contact, labelled.
- **Tests:** an envelope captured in epoch e cannot be opened after the key for e is deleted; a payee offline for several epochs still receives (payer keeps the last two epoch keys).
- **Risk:** epoch key distribution to payers that met the payee long ago (fallback above).

## M7. Bounded credit (3–4 weeks, plus the existing v1.1 co-signer work)

Files: `core/verify.rs` (policy), `node/app.rs` (per-payer caps and circle gossip), new `core/gossip.rs`.

- Pending only for pinned or explicitly accepted payers; caps from [pending-tier.md](../components/pending-tier.md) tasks 5–6; circle nullifier gossip sealed to pinned payees; conflict evidence bundle.
- Escrow co-signer group as planned for v1.1, with co-signers acceptable to both parties.
- **Tests:** the pending-tier demo script; a conflict across the circle is refused with reason 8; a stranger's payment waits for work-priced inclusion.

## M8. Verifiable distribution (4–6 weeks)

Files: `.github/workflows/release.yml`, `android.yml`, `ci.yml`, `scripts/install.sh`, `rust-toolchain.toml`, `android/app/build.gradle.kts`, `logos-repo.json`, new `release/` metadata and keys documentation.

- Cache-free release builds, `publish` behind a reviewed environment, immutable releases, no `--clobber` (SHA pins come from v0.7.2).
- Reproducibility: pinned toolchain, `SOURCE_DATE_EPOCH`, path remapping, deterministic tar/gzip, `nix build --rebuild` for `.lgx`, APK signature-copy check; two independent rebuilders.
- GitHub artifact attestations; `install.sh` verifies attestation and a t-of-n signed `release.json`.
- `lgx sign` and published DID; Basecamp install docs add the keyring step.
- New Android `applicationId` with an offline release key.
- TUF-lite metadata relayed over the mesh, expiring by verified block height (needs M1).
- **Tests:** two clean rebuilds match byte for byte; a tampered tarball or unsigned `release.json` is refused by `install.sh`; a device given expired metadata keeps its version and shows "update status unknown since height H"; a downgrade is refused.
- **Risk:** macOS binary reproducibility [unverified]; apksigner ≥ 35 output issue reported by F-Droid.

## M9. Sealed device and hardware-signer spike (3–5 weeks)

Files: `node/lib.rs` (`write_private` with `OpenOptions::mode`, encrypted store), `core/wallet.rs`, `kt/` (Keystore wrapper), Logos module `rust-lib/src/lib.rs` (caller allow-list and confirmation).

- One database key wrapped by Android Keystore, macOS Keychain, Linux Secret Service or `systemd-creds`; passphrase fallback.
- Chain clock replaces wall time for mesh expiry, gossip and metadata.
- Pruning of settled history beyond the dispute window.
- Spike: sign one Ironwood PCZT on a Keystone and on a Ledger from our builder; report compatibility.
- **Tests:** files at rest contain no key material; a module outside the allow-list cannot call `pay`; a device with a wrong clock still expires mesh packets by block height.

## M10. Mainnet gate

All of: M1–M9 gates; mainnet header vectors and a mainnet FlyClient proof measured; Tor submission measured from at least two networks; an external review of T1 proofs, the T3 wire format and the address/receiving proof; the pending-tier and co-signer failure-model tests; the release signed by the named signers and reproduced by two rebuilders. Mainnet sends still need explicit approval each time (AGENTS.md).

## Wire-format changes, summary

| Change | Milestone | Breaks v0.7.x peers? |
|---|---|---|
| Proof bodies (inclusion, anchor, auth-data, compact tx, frontier) | M1 | No (new `Body` variants; old nodes drop them) |
| Ticks and FlyClient proofs | M3 | No (new broadcast type) |
| Enrollment v2, certificates, fleet log | M4 | Yes for enrollment: v0.7.x fleets re-enroll |
| Noise links and rotating identifiers | M5 A | Yes: link Hello changes |
| Packet v2, preimage acks, hints, size classes | M5 B | Yes |
| Epoch receive keys | M6 | No if old static keys remain a fallback |
| Circle gossip | M7 | No (new) |
| `release.json` metadata | M8 | No |

## Top risks

1. **Heuristic chain proofs.** FlyClient on Zcash's moving-average difficulty is unproven (ZIP 221). Mitigation: work pricing for every value decision; FlyClient only to pick between sources.
2. **P2P gateway scope creep.** Building a P2P client is new work and must stay a header/block fetcher, not a node. Mitigation: no mempool, no validation beyond what proofs claim; adopt Zebra prover RPCs if they ship.
3. **BLE costs unmeasured.** Every byte figure is modelled. Mitigation: the M3 and M5 gates measure on two phones before mainnet.
4. **Wire breaks.** M4 and M5 force a coordinated upgrade. Mitigation: ship both in one testnet release with a clear changelog entry.
5. **Tor in shutdowns.** May be unavailable exactly when needed. Mitigation: owner policy (decision 5) and a Nym backend later.
6. **Testnet thinness.** One lightwalletd operator and few P2P peers make testnet a weak stand-in for mainnet source diversity.
