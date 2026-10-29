# Pending tier: statement, receipt, nullifier sharing, policy

**v0.7.7:** known invited payers can obtain an IOU receipt before mining;
open/unknown payers cannot. Admission caps are 3 outstanding payments / 1 ZEC
per payer and 3 ZEC per payee. The verifier rejects amounts **above** the
configured 0.5-ZEC floor; the future design requires escrow **at or above** it.
Caps bound new admission at one honest payee, not fleet loss or reorg-restored
debt, and do not prevent double-spends.

Shipped statements/receipts are signed JSON using mesh Ed25519 identities and
sealed `ozm1` envelopes. Everything below is the **25–26 October MVP v1 design**:
device-keystore enrollment, binary formats, invoice matching, nullifier sharing
(required for complete v1) and escrow remain unbuilt. [Problem #3](expiry-reissue.md)
records the accepted family, A-initiated repair and expired-unresolved policies;
B-initiated repair is deferred. Checks: [payee-verifier.md](payee-verifier.md);
builder: [offline-builder.md](offline-builder.md); co-signers: [double-spend.md](double-spend.md).

## Decided

### Summary [decided]

Device payment keys sign statement bodies (**276 B**) and receipt bodies (**260 B**);
payees share nullifiers, not statements. The sections below define enrollment,
bytes, admission policy and durable state. The BitChat carrier must be
re-specified for `ozm-mesh`.

Planned harness: four Macs, A payer, B and C offline payees, D online gateway
signing bundles with its enrollment payment key. v1.1 co-signer processes run
across them ([double-spend.md](double-spend.md), open 6).

### Keys [decided]

- **Payment key:** install-generated Ed25519, one per device, never seed-derived. Store as `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`; [Apple](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly) says these items "do not migrate to a new device". Restored devices re-enroll and retire the old key. This resists cloning, not on-device malware. Enroll in person, cross-signed by BitChat's Ed25519 key, which BitChat binds to its Noise static key.
- **Rejected signers:** Noise X25519 (cannot sign; transcripts prove nothing to third parties); BitChat Ed25519 (ephemeral fallback when keychain is locked/denied, panic deletion, chat signing: `NoiseEncryptionService.swift` L289–323, L575–578 at `04995f68`); seed-derived keys (restore forks sequences; seed thieves can sign); Ironwood spend-auth signatures (rerandomized `rk` identifies nobody).
- Reserved, not built: `sig_alg = 0x02` for a non-exportable Secure Enclave P-256 key.
- Hash `H(tag, x) = SHA-256(tag16 ‖ x)`; signature `Ed25519(sk, tag16 ‖ body)`; all integers little-endian. Verify strictly (dalek `verify_strict` or equivalent).

### Enrollment record (exchanged in person at setup) [decided]

```
tag "OZC_Enroll_v1___" ‖ body (125 B), signed twice
off len field
  0   1 version = 1
  1   1 sig_alg = 1 (Ed25519)
  2   1 roles: 1 payer, 2 payee, 4 gateway, 8 co-signer
  3   1 reserved = 0
  4  16 fleet_id
 20  32 payment_pk
 52  32 noise_static_pk        BitChat peer identity
 84  32 bitchat_signing_pk     BitChat announce and group signing key
116   8 not_before_ms
124   1 reserved = 0
sig_payment (64) = Ed25519(payment_sk, tag ‖ body)
sig_bitchat (64) = Ed25519(bitchat_signing_sk, tag ‖ body)
```

Peers pin received records; harness records come from the setup fleet file. Mainnet needs a fleet-admin signature (open). Gateway role bit 4 identifies its chain-bundle signing key ([gateway.md](gateway.md)).

### PaymentStatement v1 (A → B only) [decided]

```
tag "OZC_PayStmt_v1__"
off len field                 payee check
  0   1 version = 1
  1   1 sig_alg = 1
  2   1 flags                 bit0 re-issue; bit1 escrow tier (v1.1); other bits 0
  3   1 reserved = 0
  4  16 fleet_id              == configured fleet
 20   4 consensus_branch_id   == 0x77190AD9 (config) and == tx header
 24  32 payer_pk              enrolled payer; its noise_static_pk == the Noise peer that sent the message
 56  32 payee_pk              == own payment_pk
 88  32 txid                  == txid recomputed from the bytes (ZIP 229/244)
120  32 auth_digest           == auth digest recomputed (commits to anchor, proofs, signatures)
152  32 invoice_hash          == H("OZC_Invoice_v1__", exact UTF-8 ZIP 321 URI B sent); invoice open and unpaid
184   8 amount_zat            == sum of outputs that decrypt to B's ivk
192   4 expiry_height         == nExpiryHeight in the tx
196   4 payer_tip_height      informational
200   8 created_at_ms         informational, never trusted
208   4 seq                   > last seq seen from this payer (an exact resend with the same stmt_hash is allowed)
212  32 prev_stmt_hash        H(tag, previous body); zeros when seq = 1
244  32 supersedes_txid       zeros unless flags.bit0
276     end of body; +64 signature
stmt_hash = H("OZC_PayStmt_v1__", body)
```

- The v6 txid commits to nullifiers, outputs, value balance, expiry and branch; the anchor is auth data ([ZIP 229](https://zips.z.cash/zip-0229)). Read nullifiers from tx bytes rather than repeat them here. `auth_digest` identifies the bytes B checked; re-anchoring keeps txid but needs a new statement.
- A nonce in the ZIP 321 `memo` makes the invoice unique; check 6 compares the decrypted memo.
- **Re-issue** (`flags.bit0`): retain `supersedes_txid`, parties, amount and invoice binding. Adjacent overlap (`{a} → {a,b} → {b}`) does **not** ensure family-wide exclusion. Fresh rebuilds change dummy nullifiers; retained state can preserve actions/padding. Accepted proposal #2 preserves canonical effects for ordinary renewal; a relaxed rule needs a common nullifier across **all** versions. Count the obligation once without resetting exposure; any version's inclusion settles it. Repair fields and same-txid refresh semantics remain open ([decisions](expiry-reissue.md)).
- **Fee:** Ironwood-only `valueBalanceIronwood` must equal the action-count fleet fee, 10,000 zats for 2 actions; v6 has no fee field. Refuse deliberately unmineable underpayment.
- Original BitChat framing: `privateMessage` text `"ozc1:pay:" + base64(kind 0x01 ‖ u32 tx_len ‖ tx ‖ body ‖ sig)`, ~12.7 KB. Stock text's 255-byte cap required fork `privateMessageV2` ([archived transport plan](../../archive/docs-2026-10-27/transport-bitchat.md)). [ozm-mesh](mesh.md) instead fragments JSON `ozm1` payloads.

### Receipt v1 (B → A) [decided]

```
tag "OZC_Receipt_v1__", signed by the payee payment key
off len field
  0   1 version = 1
  1   1 sig_alg = 1
  2   1 state: 1 checked, 2 submitted, 3 included, 4 confirmed, 5 conflicted, 6 expired-unresolved, 0x10 rejected;
        bit 0x80 = single-source qualifier (chain view from one verified gateway) [decided 26 Oct]
  3   1 reason: 0 none, 1 over floor, 2 payer count cap, 3 payer value cap, 4 payee total cap,
        5 stale chain view, 6 anchor unknown, 7 expiry too near, 8 known pending conflict,
        9 bad statement, 10 invalid tx, 11 unknown/paid invoice, 12 seq regression,
        13 payer blocked, 14 bad fee
  4  16 fleet_id
 20  32 payee_pk
 52  32 payer_pk
 84  32 stmt_hash
116  32 txid
148  32 auth_digest
180   4 known_height H          B's last chain bundle
184  32 known_block_hash at H
216   4 conflict_height         0 if none
220  32 conflict_txid           zeros if none
252   8 created_at_ms
260     end of body; +64 signature
```

- `checked` renders as **"passes checks at height H; pending"** and proves B accepted the IOU. Send only after fsyncing tx, statement and counters. Framing: BitChat `privateMessage`, `"ozc1:rcpt:" + base64`.
- State 6 means the verified tip reached expiry with no known inclusion, not non-inclusion or cancellation; debt/reservations remain. Repair fields are open. The single-source qualifier occupies bit 0x80 without enlarging the body (decided 26 Oct).

### Nullifier sharing between payees [decided]

- **Carrier:** BitChat fleet-payee **group message** (`MessageType.groupMessage = 0x25`), ChaCha20-Poly1305 under the group key. Relays see group ID, epoch, nonce and outer `senderID` (`BLEService.swift` L2839–2846, corrected 26 Oct). Encrypted BitChat Ed25519 key/signature map to enrollment via `bitchat_signing_pk`. Groups: at most 16 members, 6 h gossip retention, iOS/macOS only (`GroupProtocol.swift` L350–510, `GossipSyncManager.swift` L73–94, `TransportConfig.swift` L391 at `04995f68`).
- **Payload:** `"ozc1:nf:" + base64(version u8 ‖ op u8 (1 add, 2 remove) ‖ count u8 ‖ entries)`, entry = `txid 32 ‖ expiry_height u32 ‖ n u8 ‖ n × nullifier 32`. A 2-action payment is 104 B. No payer key, amount or payee address.
- **Lifecycle:** post `add` after check 7, before receipt; re-post the open set every 4 h. Claim removal at inclusion/expiry is **not** settlement or proof of non-inclusion. Family retention and same-sharer renewal (#2, #4–5) must preserve debt/evidence beyond gossip expiry.
- **Acceptance:** refuse a nullifier matching **another** sharer's different txid (reason 8). With another fleet payee connected, wait **5 s** after posting before signing (Satoshi's "listen a few seconds"; Karame et al., CCS 2012).
- **Coverage:** one partition after propagation; misses faster presentations, partitioned/non-sharing payees and direct gateway spends. Cross-partition bound: partitions × limit.
- **Abuse:** outsiders cannot post; an insider needs a valid transaction to claim its nullifiers. False claims cause refusal, never penalties or hot-listing. Rate-limit sharers; cap at 200 entries; expire claims at `expiry_height`.
- **Evidence (after MVP v1):** privately request `"ozc1:evq:" + txid`; sharer returns `"ozc1:evr:" + tx bytes`. B runs checks 1–3 and 7 and recomputes txid. Valid T1/T2 with a shared nullifier prove two authorized spends; partial digest preimages do not. Never forward the sharer's statement/receipt: they name the payer and amount.
- **Gateway extension:** the gateway includes its mempool's nullifiers in chain bundles, which catches a pre-broadcast race when B's bundle is fresh.

### Sequence number [decided]

- Refuse non-increasing `seq` (reason 12), except the exact resend allowed above. v1 does not disclose/enforce the whole hash chain or base caps on it: a cheater can fork it, unlike card-enforced EMV ATC.
- Benefits: per-payee rollback/replay detection; reconciliation audit of statements 1…n−1; 2 × 340-B fork proofs (same `seq` or `prev_stmt_hash`, different signed bodies).
- Persist `seq + 1` **before** releasing a signature. Crashes then cannot fork; restored backups lack the non-migrating key. Self-reported outstanding counts were dropped as unenforceable.

### Policy: initial defaults [decided]

Values are **provisional**, not mainnet safety bounds; harness ZEC is notional.
Freshness, validity, reservation and confirmation use separate clocks ([expiry-reissue.md](expiry-reissue.md#lifetime-freshness-privacy-and-upgrades)). Named 26 October decisions below remain subject to measurement.

| Parameter | Value | Why |
|---|---|---|
| Floor F | 0.5 ZEC per payment; a payment **≥ F** needs escrow (decided 26 Oct) | A 0.6 ZEC request shows a refusal |
| Escrow cap | 3.0 ZEC outstanding per payee from escrow payments; **1.0 ZEC per payer and 1.0 ZEC per certified co-signer group** at each payee (decided 26 Oct) | Limits damage if the co-signer group is broken; a group cap catches one broken group backing many payers |
| Combined ceiling | **3.0 ZEC per payee across pending and escrow** (decided 26 Oct) | Without it the two 3 ZEC pots add to 6 ZEC |
| Expiry margin M | 3,456 blocks (one target day): `E − H_B ≥ ⌈(S+U)/25 s⌉ + Q + M`, S = 30 min, Q = 10. Minimum remaining life: city 3,826, 20 km 5,266, mountains 13,906, cross-country 27,730 blocks | Planning budget, not a guarantee; #9 research |
| App retry | Immediate, then 10, 20, 40, 80, 160, 320 min, then every 6 h; reconnect skips the wait | Decided 26 Oct |
| Checkpoint and floor | Difficulty floor = checkpoint difficulty / 2; checkpoint refreshed daily, signed by the **fleet owner key** (enrolled at setup); no new acceptance if older than 7 days; gateways can never lower it | Decided 26 Oct; alarm, not a theorem |
| Loss bound | Per payee ≤ min(T, k · v) with v = min(V, C·F) for k dishonest payer identities; per payer across P payees ≤ P · v. Nullifier sharing does not share payer counters | Correction 26 Oct: not "partitions × V" |
| Per-payer outstanding count C | 3 | EMV UCOL-style cap; a 4th payment is refused |
| Per-payer outstanding value V | 1.0 ZEC | 3 × 0.3 passes; a 4th fails |
| Payee total outstanding T | 3.0 ZEC over all payers | Payee's appetite |
| Stale chain view | Refuse **new** pending acceptance if bundle older than 30 min; provisional | No community-derived Zcash bound; longer expiry does not extend acceptance freshness. Proposal #6 / open item 2 |
| Expiry margin | Historical initial `expiry_height ≥ H + 20`; **provisional, insufficient against allowed stale H** | 30 min ≈72 NU7 target blocks; replaced by lag + uplink + confirmations + margin (#6, decided 26 Oct) |
| Anchor set | Last 100 blocks for **new-payment acceptance**, provisional | Not consensus; 10,375-block-old root mined. Existing-family repair may use a historical root or re-anchor (#6) |
| Share wait | 5 s, only if another payee is connected | Nullifier sharing |
| Counter reset | `confirmed` (10 confirmations) on both harness and mainnet [decided 26 Oct] | Ten is provisional low-value demo policy, not universal finality ([prior art](../prior-art.md)) |
| `expired`, not re-issued | Stays counted; payer blocked for new obligations | Expiry does not erase debt. Proposed #5 keeps family counted through repair; proposed #6 permits valid existing-family repair/evidence despite block |
| `conflicted` | Payer blocked at this payee; unblocked by hand; evidence kept | Never hot-list keys automatically |
| Go online if possible | Past ⌈C/2⌉ outstanding, if the gateway is reachable, wait for `submitted` before handing over goods | EMV LCOL analogue |
| Single-source cap | While the chain view comes from one verified gateway (`checked (single-source)`, [payee-verifier.md](payee-verifier.md)), the ⌈C/2⌉ threshold is the hard cap: no further pending acceptance from that payer until a second gateway's tip agrees | One gateway cannot prove no conflict exists |

Planned demo: 0.6 refused; three × 0.3 accepted, fourth refused; **confirmed (10)** resets counters; A pays B, then connected C refuses the same note (reason 8); disconnected B/C both accept, one mines, the other becomes `conflicted`.

### Policy: fleet formula [decided]

Inputs: `L` = accepted/recoverable loss per enrolled payer per detection window;
`W` = worst-case acceptance-to-conflict-detection time;
`P = min(N_payees, r · W)` = reachable payees in different partitions, with r the
new-payee visit rate; `p50`, `p95` = payment-amount percentiles.

- Per payee, per payer: `V = L / P`
- Floor: `F = min(p95, V)`
- Count: `C = max(1, ⌊V / p50⌋)`; soft threshold `⌈C/2⌉`
- Per payee total `T`: the payee's own appetite, at least V
- Expiry delta: `Δ ≥ 3 × max uplink interval / block time` sizes **already-accepted settlement lifetime**, not new-acceptance freshness; stale-view limit ≤ W. The shutdown profile (72,576) was selected 26 Oct ([expiry-reissue.md #1](expiry-reissue.md#proposals-and-owner-decisions-26-october)). Multi-day new acceptance would need separately approved risk limits.

Bounds: one dishonest payer loses the fleet at most `P · V = L`; k colluding payers `k · L`; one payee at most `T`. Sharing lowers P from payees to partitions; escrow removes the bound for escrowed payments. Example: L = 100, P = 5 gives V = 20; p95 = 10 gives F = 10; p50 = 3 gives C = 6.

### What the payee persists (fsync before replying) [decided]

- **Fleet config:** fleet_id, branch ID, fee table, expiry margin, limits, enrollment records with status (active, blocked, retired), group ID and key handle.
- **Payments**, keyed by (txid, auth_digest): tx bytes, statement and signature, own receipts, invoice URI and hash, amount, nullifiers, expiry, state history with timestamps, H at check, supersedes link, conflict evidence (txid, height, inclusion proof).
- **Per payer:** last seq and stmt_hash, blocked flag and reason. Outstanding count and value are derived from the payments table.
- **Invoices:** hash → amount, memo nonce, status, so an invoice is paid once and a statement cannot be replayed.
- **Nullifier index:** own pending nullifiers plus received shares (sharer, txid, nullifiers, expiry, received_at).
- **Chain view:** last bundle height, hash and time; recent anchors; mined nullifiers since the oldest open payment.
- **Ordering:** write the tx, the statement, the counter update and the share-post intent in one transaction; then post the share, wait, send the receipt.

The payer persists write-ahead seq, reserved-notes → statement mapping and outbox.
Encrypt all state with a random database key wrapped by the platform keystore;
open fail-closed (Peers `KeystoreCrypto.kt` L12–67, Peers research). PIN/duress wipe
needs a funds-recovery policy first.

## Findings

### What the statement proves after a conflict

Evidence (~11 KB): enrollment, signed statement, T1 bytes, mined/public T2 bytes,
T2 inclusion proof and headers. It **proves** K promised B `amount_zat` for invoice
I through T1, whose nullifier N was mined in T2 ≠ T1, excluding T1. Without a
K-signed re-issue to T2, K owes B under the fleet agreement. A K-signed T2
statement to another payee attributes double commitment to the device, not just
the seed.

It **does not** prove T2's broadcaster (a seed thief lacks the device key), timing
(`created_at` is asserted; anchor/expiry give lower/upper bounds), delivery of
goods or absence of other payment. This is a debt claim, not proof of fraud.

### What the statement must not leak

- Send only A→B inside Noise, never to gateways, shares, public messages, bundles or couriers.
- Fixed-length statement and 2-action tx padding hide content size; peer IDs, timing and total size remain visible (P5).
- Give plaintext conflict evidence only to the dispute settler (P1).
- Shares link the receiving payee to the mined tx inside the fleet (P9, [privacy-and-threats.md](../privacy-and-threats.md)); removing txid cannot hide nullifier linkage. Closed fleets only.

### Card and regulator practice

| Practice | Value | Source |
|---|---|---|
| Floor limit; above it, go online | Set by acquirer or issuer | [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf) |
| Consecutive offline counters reset by an online transaction | LCOL 9F14 / UCOL 9F23 | [OpenSCDP EMV](https://www.openscdp.org/scripts/tutorial/emv/terminalriskmanagement.html) |
| Terminal sums recent amounts per card against the floor | — | OpenSCDP EMV |
| Random online selection below the floor | Threshold plus target percentage | OpenSCDP, Visa |
| Sync deadline | 24 h; 4 days for transit | Visa Rules 5.7.3.3 |
| Store-and-forward caps per terminal | Max amount, queue total, queue count | Adyen, JP Morgan |
| RBI offline framework | ₹500 per transaction, ₹2,000 total; UPI Lite ₹1,000 / ₹5,000; replenishment only online with AFA | RBI circular 4 Dec 2024 |
| UPI Lite X (NFC offline) | ₹500 / ₹2,000 balance / ₹4,000 per day | PhonePe, Moneycontrol [secondary] |
| BoE offline pound pilots | Max consecutive offline, max payment, max cumulative, max stored balance; verifiable transaction chains; checks worked only when intermediaries shared data | BoE 2025 |

Pattern: a per-transaction cap is ¼ to ⅕ of the total cap, and only an online event resets counters.

### Prior art: what transfers

| Source | Observation | Transfers? |
|---|---|---|
| BTCPay Server | "paid" at 0-conf, "settled" at the merchant's confirmation count; waits for 1 conf if the tx signals RBF | Two-tier states; refusing pending when the tx is cheap to replace or underpays (our fee check). Not mempool monitoring: B is offline |
| Bitcoin Core 28.0 | `mempoolfullrbf` defaults on; first-seen is policy only | The gateway's mempool view is a hint |
| Satoshi, bitcointalk topic 423 | Blast it out, monitor for double-spends; even a second's wait puts a double-spend "at a huge disadvantage" | The 5 s share wait inside a partition |
| Monero | `double_spend_seen`; small-amount 0-conf; kasisto#33 never-confirm attack | Small-amount acceptance and the fee check; expiry makes it deterministic in Zcash |
| GNU Taler DD 30 (abandoned) | Trust-based offline up to a threshold in an emergency mode; HSM variants | Approach 1 is the pending tier; HSM variants do not transfer |
| Grothoff & Dold 2021 | Offline payments carry inherent, severe risks | "IOU, bounded" |
| Cashu NUT-11/12, Fedimint reissue | Receiver can verify genuineness, not unspentness; P2PK locks need the sender online; "validate ≠ unspent" | Prevention needs a lock made while online: the v1.1 escrow |
| BoE digital pound offline (2025) | Reconciliation detects but "could not prevent" losses; limits; chains; data sharing between intermediaries | Limits, hash chain, the data-sharing requirement. Not the secure element |
| RBI / UPI Lite | Cap ratios; replenishment only online | Ratios; resets only online |
| OVERDRAFT (arXiv 2504.05143) | Time-limited loans weighted by reputation | Off-chain pledges at enrollment could fund L |
| Delving Bitcoin #2929 (2 Oct 2026) | Is the pre-finality race intrinsic to single-owner coins? 0 replies as of 25 Oct | No construction known; bounded trust or 2-of-2 |
| Zcash forum #50167, #54788 | "Offline to offline will always be exploitable"; asks whether FROST is the last-mile answer | Consistent with the docs |

## Open

1. [unverified] Ed25519 edge-case agreement between CryptoKit and dalek `verify_strict`.
2. [decided, unimplemented] Implement the family/canonical-effects/count-once rules above ([expiry-reissue.md](expiry-reissue.md)).
3. [unverified] BitChat group-message API, 16-member roster and 6 h gossip retention at `04995f68`; framing overhead (350–450 B estimate); whether a group message reaches 15 peers within 5 s on BLE.
4. [finding] Pinned Zakura rejected same-input renewal while the original was resident and mined it after expiry eviction. Live Zebra and application retry remain open ([expiry-reissue.md](expiry-reissue.md#consensus-expiry-versus-mempool-policy)).
5. [unverified] Secure Enclave monotonic counter availability; keychain behaviour when a Mac disk image is cloned.
6. [unverified] Courier envelope size; ZIP 304 status; UPI Lite X figures (secondary sources).
7. Measure amounts, uplink intervals, partition counts and stale-view risk before treating policy values as safety bounds.
8. Decide when evidence fetching (`ozc1:evq/evr`) ships; it is after MVP v1 by default.
9. Tests that gate MVP v1 for this tier: the demo script above runs end to end; a statement with a wrong `auth_digest`, wrong `amount_zat`, replayed `seq` or wrong fee is refused with the right reason; a receipt is only sent after the payee's store is fsynced (kill the payee between check and receipt); with one gateway the ⌈C/2⌉ cap is hard and a second agreeing gateway lifts it.
10. Resolved 26 Oct: single-source encoding is recorded in Receipt v1 above.
