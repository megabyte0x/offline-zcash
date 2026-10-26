# Pending tier: statement, receipt, nullifier sharing, policy

The pending tier is how MVP v1 accepts a payment before it is mined: the payer signs a fixed-size **PaymentStatement** with a device-bound key, the payee answers with a signed **Receipt**, payees in the same mesh partition share pending nullifiers, and card-style limits bound the loss.
Nothing here prevents a double-spend. It makes a conflict attributable to a known device key and keeps the exposure per payer and per payee small. Pending applies below the floor only; above it, prevention is the v1.1 co-signer group in [double-spend.md](double-spend.md).
Decided 25 October 2026 from R3 pending tier; the full spec with sources is kept at `.amp/in/artifacts/pending-tier-spec.md` in this thread's orb. Checks that consume these formats are in [payee-verifier.md](payee-verifier.md); the builder that emits the statement is in [offline-builder.md](offline-builder.md).

Research update: [problem #3](expiry-reissue.md) corrects assumptions below. Its family, A-initiated repair and expired-unresolved policies were **accepted 26 October** (B-initiated repair requests deferred). Counter reset is `confirmed` on both networks; single-source is a flag bit in the Receipt state byte (both decided 26 October).

## Decided

### Summary [decided]

| Question | Decision |
|---|---|
| Who signs the statement | A dedicated per-device **Ed25519 payment key**, stored in the keychain as `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, enrolled in person at setup and cross-signed by the device's BitChat Ed25519 key (which BitChat binds to the Noise static key) |
| What it covers | A 276-byte body: fleet, branch ID, payer and payee keys, txid, auth digest, invoice hash, amount, expiry, payer tip height, timestamp, sequence number, previous-statement hash, superseded txid, tier flag |
| Receipt | A 260-byte body signed by the payee's payment key: state, reason, H and block hash at H, statement hash |
| Nullifier sharing | Built in MVP v1. About 104 B per payment as a BitChat group message to a fleet-payee group, carrying txid, expiry and nullifiers only. A hit makes the payee refuse pending acceptance |
| Sequence number | `seq` and `prev_stmt_hash` in the statement; the payee enforces only that `seq` increases per payer. No cap is based on it |
| Initial policy | Floor 0.5 ZEC; per payer 3 payments and 1.0 ZEC outstanding; 3.0 ZEC outstanding per payee. Thirty-minute freshness is provisional, not a sourced safety bound. Counters reset at `confirmed` (10, provisional) on harness and mainnet |
| Harness roles | Four Macs: A payer, B payee, C second payee (both offline), D gateway (online, signs bundles with its enrollment payment key). Co-signer processes for v1.1 run across the Macs ([double-spend.md](double-spend.md) open 6) |

### Keys [decided]

- **Payment key:** Ed25519, one per device, generated at install, never derived from the Zcash seed. Stored `ThisDeviceOnly`, so a device restored from backup does not carry it ([Apple](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly): such items "do not migrate to a new device"). The restored device re-enrolls and the old key is retired. This protects against cloning, not against malware on the device.
- **Rejected:** the BitChat Noise X25519 key (cannot sign, and a Noise transcript proves nothing to a third party); BitChat's own Ed25519 key as the signer (it silently falls back to an ephemeral key when the keychain is locked or denied, panic mode deletes it, and it signs chat: `NoiseEncryptionService.swift` L289–323, L575–578 at `04995f68`); a seed-derived key (a restored seed forks the sequence and a seed thief can sign statements); the Ironwood spend-auth signature (`rk` is rerandomized, so it identifies nobody).
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

Every peer pins the records it receives. On the harness they come from a fleet file loaded at setup; a mainnet fleet adds a fleet-admin signature (open). The gateway's record (role bit 4) carries the payment key that signs chain bundles ([gateway.md](gateway.md)).

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

- The v6 txid commits to nullifiers, outputs, value balance, expiry and branch ID, and the anchor is auth data ([ZIP 229](https://zips.z.cash/zip-0229)), so the statement does not repeat the nullifiers; a verifier reads them from the tx bytes. `auth_digest` records exactly which bytes B checked; a re-anchored copy keeps its txid, so the payer sends a new statement.
- The invoice is made unique by a nonce in the ZIP 321 `memo`, which check 6 compares with the decrypted memo.
- **Re-issue** (`flags.bit0`): retain `supersedes_txid`, payer/payee, amount and invoice binding. Correction: adjacent “at least one shared nullifier” does **not** ensure family-wide exclusion (`{a} → {a,b} → {b}`). Fresh rebuilds change dummy nullifiers, but retained state can keep actions/padding identical. Proposed #2 preserves canonical effects for ordinary renewal; any relaxed family rule requires a common nullifier across **all** versions. Count the obligation once, never remove/reset its exposure on renewal; any family version's inclusion settles it. **Awaiting owner decision**, including same-txid authorization refresh semantics; no wire change approved ([findings and proposals](expiry-reissue.md)).
- **Fee check:** `valueBalanceIronwood` of an Ironwood-only tx (v6 has no fee field) must equal the fleet fee for its action count, 10,000 zats for 2 actions. This blocks a transaction built never to be mined.
- Framing: BitChat `privateMessage` text `"ozc1:pay:" + base64(kind 0x01 ‖ u32 tx_len ‖ tx ‖ body ‖ sig)`, about 12.7 KB. Needs the fork's `privateMessageV2`: stock private text is capped at 255 bytes ([transport-bitchat.md](transport-bitchat.md), approved 26 Oct).

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

- `state = checked` renders as **"passes checks at height H; pending"**. B sends it only after the tx, the statement and the counter update are fsynced. Framing `"ozc1:rcpt:" + base64`, a BitChat `privateMessage`.
- The receipt lets A prove later that B accepted the IOU.
- State 6 is `expired-unresolved` [decided 26 Oct], not proof of non-inclusion or cancellation: the verified tip reached expiry when verified tip reaches expiry with no inclusion known; debt/reservations remain. Repair reasons/state encoding are pending (#4–5). `checked (single-source)` also has no slot: new state byte versus flag bit remains undecided.

### Nullifier sharing between payees [decided]

- **Carrier:** a BitChat **group message** (`MessageType.groupMessage = 0x25`) to a fleet-payee group, ChaCha20-Poly1305 under the group key. Relays see the group ID, epoch, nonce and the outer BitChat `senderID` (`BLEService.swift` L2839–2846, corrected 26 Oct); the sender's BitChat Ed25519 key and signature are inside the ciphertext. Payees map that key to an enrolled payee through `bitchat_signing_pk`. Groups hold at most 16 members and are gossip-synced for 6 h (`GroupProtocol.swift` L350–510, `GossipSyncManager.swift` L73–94, `TransportConfig.swift` L391 at `04995f68`); iOS/macOS only.
- **Payload:** `"ozc1:nf:" + base64(version u8 ‖ op u8 (1 add, 2 remove) ‖ count u8 ‖ entries)`, entry = `txid 32 ‖ expiry_height u32 ‖ n u8 ‖ n × nullifier 32`. A 2-action payment is 104 B. No payer key, amount or payee address.
- **Lifecycle:** post `add` after check 7 passes and before the receipt; re-post the open set every 4 h because gossip keeps 6 h. Transport claims have finite expiry; removing a claim at inclusion/expiry is **not** financial settlement or proof of non-inclusion. Durable family/exposure retention and renewal of the same sharer's entry are proposed (#2, #4–5); do not discard debt or old evidence when a gossip entry ages out.
- **Acceptance rule:** any nullifier of an incoming tx that matches an entry from **another** sharer with a different txid is refused with reason 8. If at least one other fleet payee is connected, B waits **5 s** after posting before signing the receipt (Satoshi's "listen a few seconds"; Karame et al., CCS 2012).
- **Coverage:** one partition, after propagation. It misses presentations faster than propagation plus the wait, conflicts across partitions, spends sent straight to a gateway, and payees who do not share. The cross-partition bound stays at partitions × limit.
- **Abuse:** outsiders cannot post. An insider cannot forge a claim that matches a real transaction without having seen a valid one. A false claim only makes a payee refuse; there is no penalty and no hot-listing. Rate-limit each sharer and cap the store at 200 entries. Claims expire at `expiry_height`.
- **Evidence on demand (after MVP v1):** on a hit, B sends `"ozc1:evq:" + txid` privately to the sharer, who replies `"ozc1:evr:" + tx bytes`. B runs checks 1–3 and 7 and recomputes the txid. A valid T1 sharing a nullifier with T2 proves two authorized spends. Partial digest preimages are not sufficient, and the sharer's statement or receipt must never be sent as evidence (it names the payer and the amount).
- **Gateway extension:** the gateway includes its mempool's nullifiers in chain bundles, which catches a pre-broadcast race when B's bundle is fresh.

### Sequence number [decided]

- `seq` and `prev_stmt_hash` are in every statement. The payee refuses `seq` at or below the last one seen from that payer (reason 12). Nothing else is enforced: the payer does not disclose its chain in v1, and no cap is based on `seq`, because a cheater can fork the chain freely (EMV's ATC works only because the card enforces it).
- What it adds: rollback and replay detection at each payee; a completeness audit at reconciliation (ask A for statements 1…n−1); fork proofs of 2 × 340 B (two valid statements with the same `seq` or `prev_stmt_hash` and different bodies).
- The payer persists `seq + 1` **before** releasing a signature, so a crash cannot fork the chain. A restored backup cannot fork, because the key does not migrate. Self-declared outstanding-count fields were dropped: a cheater can lie in them freely.

### Policy: initial defaults [decided]

Initial values, **provisional** until measured and owner-approved; on the harness the ZEC is notional. Do not treat freshness, validity, reservation and confirmation clocks as interchangeable ([expiry-reissue.md](expiry-reissue.md#lifetime-freshness-privacy-and-upgrades)).

| Parameter | Value | Why |
|---|---|---|
| Floor F | 0.5 ZEC per payment; a payment **≥ F** needs escrow (decided 26 Oct) | A 0.6 ZEC request shows a refusal |
| Escrow cap | 3.0 ZEC outstanding per payee from escrow payments (decided 26 Oct) | Limits damage if the co-signer group is broken |
| Per-payer outstanding count C | 3 | EMV UCOL-style cap; a 4th payment is refused |
| Per-payer outstanding value V | 1.0 ZEC | 3 × 0.3 passes; a 4th fails |
| Payee total outstanding T | 3.0 ZEC over all payers | Payee's appetite |
| Stale chain view | Refuse **new** pending acceptance if bundle older than 30 min; provisional | No community-derived Zcash bound; longer expiry does not extend acceptance freshness. Proposal #6 / open item 2 |
| Expiry margin | Historical initial `expiry_height ≥ H + 20`; **provisional, insufficient against allowed stale H** | 30 min ≈72 NU7 target blocks; replaced by lag + uplink + confirmations + margin (#6, decided 26 Oct) |
| Anchor set | Last 100 blocks for **new-payment acceptance**, provisional | Not consensus; 10,375-block-old root mined. Existing-family repair may use a historical root or re-anchor (#6) |
| Share wait | 5 s, only if another payee is connected | Nullifier sharing |
| Counter reset | `confirmed` (10 confirmations) on both harness and mainnet [decided 26 Oct] | Ten is provisional low-value demo policy, not universal finality ([prior art](../prior-art.md)); no reset rule chosen here |
| `expired`, not re-issued | Stays counted; payer blocked for new obligations | Expiry does not erase debt. Proposed #5 keeps family counted through repair; proposed #6 permits valid existing-family repair/evidence despite block |
| `conflicted` | Payer blocked at this payee; unblocked by hand; evidence kept | Never hot-list keys automatically |
| Go online if possible | Past ⌈C/2⌉ outstanding, if the gateway is reachable, wait for `submitted` before handing over goods | EMV LCOL analogue |
| Single-source cap | While the chain view comes from one verified gateway (`checked (single-source)`, [payee-verifier.md](payee-verifier.md)), the ⌈C/2⌉ threshold is the hard cap: no further pending acceptance from that payer until a second gateway's tip agrees | One gateway cannot prove no conflict exists |

Demo script: (1) pay 0.6: refused by the floor; (2) pay 0.3 three times: accepted, a 4th refused; (3) mine to the **owner-selected reset threshold** (pending); (4) pay the same note to B then C while connected: C refuses with reason 8; (5) repeat with C disconnected: both accept, one mines, the other ends `conflicted`.

### Policy: fleet formula [decided]

Inputs: `L` the loss the fleet accepts per enrolled payer per detection window (what can be recovered off-chain, or the appetite); `W` the worst-case time from acceptance until a conflict is visible to payees; `P = min(N_payees, r · W)` the payees in different partitions one payer can reach within W, with r the rate of visiting new payees; `p50`, `p95` of payment amounts.

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

The payer persists the seq counter (write-ahead), the reserved-notes → statement mapping, and its outbox. Store all of it encrypted at rest with a random database key wrapped by the platform keystore, opened fail-closed (pattern from Peers `KeystoreCrypto.kt` L12–67, Peers research); a PIN or duress wipe is not adopted without a funds-recovery policy.

## Findings

### What the statement proves after a conflict

Evidence bundle, about 11 KB: enrollment record, statement and signature, T1 bytes, T2 bytes (public once mined), inclusion proof for T2 with headers.

It **proves** that the holder of payment key K promised B `amount_zat` for invoice I through T1, which spends nullifiers {N…}; some N was mined in T2 ≠ T1, so T1 can never be mined. If B holds no K-signed re-issue whose txid is T2, K's holder owes B under the fleet agreement. If a second K-signed statement exists for T2 to another payee, K committed the same note twice, attributable to the device key rather than only the seed.

It **does not** prove who broadcast T2 (a seed thief has the spend key but not the device key), when anything happened (`created_at` is self-asserted; the anchor gives a lower bound, expiry an upper bound), that goods changed hands, or that B was not paid another way. It is a debt claim, not proof of fraud.

### What the statement must not leak

- It travels only inside the A→B Noise private message, never in B's forward to the gateway, nullifier shares, public messages, chain bundles or courier envelopes.
- Fixed length, and the tx is padded to 2 actions, so size reveals nothing. Relays still see peer IDs, timing and total size (P5).
- After a conflict the evidence goes only to whoever settles the dispute; it is a plaintext payment record (P1).
- Sharing costs recipient privacy inside the fleet: once a tx is mined, group members can tell the sharer received it (P9 in [privacy-and-threats.md](../privacy-and-threats.md)). Removing the txid would not help, because the nullifiers link the share just as well. Acceptable only in a closed fleet.

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
2. [finding / pending design] Fresh rebuild randomness changes dummy nullifiers; retained construction state can preserve them. Adjacent overlap is insufficient; family-wide exclusion/canonical effects and count-once rules await owner approval ([expiry-reissue.md](expiry-reissue.md)).
3. [unverified] BitChat group-message API, 16-member roster and 6 h gossip retention at `04995f68`; framing overhead (350–450 B estimate); whether a group message reaches 15 peers within 5 s on BLE.
4. [finding] Pinned Zakura rejected same-input renewal while the original was resident and mined it after expiry eviction. Live Zebra and application retry remain open ([expiry-reissue.md](expiry-reissue.md#consensus-expiry-versus-mempool-policy)).
5. [unverified] Secure Enclave monotonic counter availability; keychain behaviour when a Mac disk image is cloned.
6. [unverified] Courier envelope size; ZIP 304 status; UPI Lite X figures (secondary sources).
7. Policy numbers remain provisional; measure amounts, uplink intervals, partition counts and stale-view risk. Owner must decide problem #3 and unify the exposure reset threshold; do not advertise the initial values as mainnet safety guarantees.
8. Decide when evidence fetching (`ozc1:evq/evr`) ships; it is after MVP v1 by default.
9. Tests that gate MVP v1 for this tier: the demo script above runs end to end; a statement with a wrong `auth_digest`, wrong `amount_zat`, replayed `seq` or wrong fee is refused with the right reason; a receipt is only sent after the payee's store is fsynced (kill the payee between check and receipt); with one gateway the ⌈C/2⌉ cap is hard and a second agreeing gateway lifts it.
10. Resolved 26 Oct: `checked (single-source)` is state bit 0x80; the receipt stays 260 bytes.
