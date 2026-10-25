# Preventing offline double-spends without a central co-signer

Scope: why a single FROST co-signer was rejected, what the literature offers instead, what ZcashFoundation/frost-tools and the "FROST and the Zcash Vault" guide provide, and the three-layer design that results. Researched 25 October 2026. Nothing was built or run. **[S]** = stated in the cited source or code, **[I]** = our inference.

---

## 1. Takeaway

No published scheme stops a fully disconnected payer from double-spending a single-owner shielded note without trusted hardware or a co-signer [S: the Delving Bitcoin thread "Is the pre-finality double-spend race intrinsic to single-owner coins?" (Oct 2026) asks exactly this and has no answer]. This is the CAP argument: a payer who can reach nobody can always sign twice. Without a central party we can still:

- **prevent** a double-spend whenever a quorum of independent signers is reachable (quorum-certified notes);
- **deter and compensate** when no quorum is reachable (accountable intents plus a bond);
- **detect** quickly everywhere (gossip of intents and fraud proofs).

Why the single co-signer was rejected: one party is a liveness bottleneck (offline co-signer means nobody can pay), a stuck-funds risk (no shielded refund path, FROST shares cannot be restored from a seed), and a single trust point.

---

## 2. Consensusless payments: prevention needs quorum intersection, not consensus

- Guerraoui, Kuznetsov, Monti, Pavlovič, Seredinschi, "The Consensus Number of a Cryptocurrency", PODC 2019, https://arxiv.org/abs/1906.05574 [S]: asset transfer with one owner per account has consensus number 1; double-spend prevention needs only Byzantine reliable broadcast, not total order.
- Baudet, Danezis, Sonnino, "FastPay", AFT 2020, https://arxiv.org/abs/2003.11506 [S]: 3f+1 authorities, each signs at most one order per (account, sequence); 2f+1 signatures form a certificate; any two quorums share an honest authority, so conflicting certificates cannot both exist. A client that equivocates locks its account, with no unlock path in the paper.
- Collins et al., "Online Payments by Merely Broadcasting Messages" (Astro), DSN 2020 [S]: asynchronous Byzantine reliable broadcast.
- Sliwinski & Wattenhofer, "ABC: Proof-of-Stake without Consensus", https://arxiv.org/abs/1909.10926 [S]: conflicting transactions may never terminate; follow-up "Consensus on Demand" runs consensus only after a conflict.
- Baudet, Sonnino, Kelkar, Danezis, "Zef", https://eprint.iacr.org/2022/083 [S]: FastPay with anonymous coins.
- Blackshear et al., "Sui Lutris", NDSS 2025, https://arxiv.org/abs/2310.18042 [S]: owned objects use consistent broadcast; equivocation-locked objects are released at epoch change through consensus.

Mapping to Ironwood [I]: split a float note's spend key with FROST (ZIP 312) among the payer and a committee of 3f+1 signers. Give the payer f+1 shares and set the threshold to (payer shares) + 2f+1, e.g. f = 1: payer 2 shares, committee 4, threshold 5 of 6. The committee alone cannot spend; the payer cannot spend without 3 of 4 committee members; every signature includes a quorum. Each signer keeps a registry nullifier → sighash and signs a nullifier only for that sighash (retries of the same transaction allowed). Under partition only the side that reaches a quorum can spend.

## 3. Collateral and statekeepers

- Mavroudis, Wüst, Dhar, Kostiainen, Čapkun, "Snappy: Fast On-chain Payments with Practical Collaterals", NDSS 2020 [S]: customer and merchant collateral in a smart contract; merchants act as statekeepers voting on payments; victims repaid from collateral.
- Avarikioti, Kokoris-Kogias, Wattenhofer, Zindros, "Brick", FC 2021, https://arxiv.org/abs/1905.11360 [S]: 3f+1 wardens, 2f+1 to close a channel, warden collateral enforced by Solidity.
- Fidelity bonds need transparent timelocked outputs [S]; shielded notes have none.
- Without scripts [I]: collateral can be held under a committee FROST key and released against a fraud proof, but enforcement is social and refunds need a live committee unless `lock_time` works (section 8).

## 4. Partition-tolerant ledgers

- Karlsson et al., "Vegvisir: A Partition-Tolerant Blockchain for the IoT", ICDCS 2018 [S]: a CRDT-like DAG that merges after partitions; good for provenance and detection, not prevention.
- "Taming Double-Spending in Offline Payments with Reputation-Weighted Loan Networks", https://arxiv.org/abs/2504.05143 [S]: probabilistic risk control without trusted hardware.
- Federal Reserve FEDS Note, "Offline Payments: Implications for Reliability and Resiliency", Aug 2024 [S]: the practical control is per-merchant offline limits.
- [I] A ledger available on both sides of a partition can detect but not prevent; quorum consensus stalls in the minority side.

## 5. Accountable double-spending

- Chaum, Fiat, Naor, "Untraceable Electronic Cash", CRYPTO '88 [S]: spending a coin twice reveals the spender's identity (detect and deter).
- Brands, "Untraceable Off-line Cash in Wallets with Observers", CRYPTO '93 [S]: prevention needs a tamper-resistant observer.
- Camenisch, Hohenberger, Lysyanskaya, "Compact E-Cash", EUROCRYPT 2005 [S]: reusing a serial reveals the key.
- Teranishi, Furukawa, Sako, "k-Times Anonymous Authentication", ASIACRYPT 2004; Camenisch et al., "How to Win the Clone Wars", CCS 2006 [S]: up to k unlinkable uses per period; a (k+1)-th use reveals identity.
- Ruffing, Kate, Schröder, "Liar, Liar, Coins on Fire! Penalizing Equivocation by Loss of Bitcoins", CCS 2015 [S]: asserting two statements in one context leaks a key that unlocks a time-locked deposit. Works only because funds sit where the payer cannot spend them alone.
- Poettering & Stebila, "Double-Authentication-Preventing Signatures", ESORICS 2014 [S]: signing two messages for one subject leaks the key.
- Androulaki et al. (IBM), "Secure and Privacy-preserving CBDC Offline Payments using a Secure Element", ePrint 2024/1746 [S]: secure element plus efficient identification after the fact.
- Grothoff & Dold, "Why a Digital Euro should be Online-first and Bearer-based", 2021 [S]: offline modes carry inherent severe risks; GNU Taler's offline design (DD 30) is marked abandoned.

On Ironwood [I]:
- **One-time-key intents alone are not enough.** They catch a payer who pays two acceptors, but not a payer who pays offline and then races a conflicting spend online with no second intent.
- **Intent plus chain fraud proof catches every case.** The payer signs `Intent(nf, txid₁, amount, expiry, counter i, bond-ref)`. If the chain shows `nf` spent in txid₂ ≠ txid₁ before tx₁'s expiry, intent plus inclusion proof is self-contained evidence anyone can check. No consensus change.
- **A per-epoch counter i ≤ k** with k-times anonymous authentication bounds total offline liability to k × floor limit even across partitions, and keeps intents unlinkable; fallback is per-epoch pseudonymous bond keys.
- **Pitfalls:** an honest user restoring an old backup can be penalised; an acceptor that sits on tx₁ past expiry muddies blame.

## 6. Offline CBDCs and transferable cash

- BIS, "Project Polaris: Handbook for offline payments with CBDC", May 2023 [S]: certified secure elements, limits, monitoring.
- ECB digital euro [S]: ECB applet in the phone's secure element, holding limits, detection afterwards. Bank of England 2025 [S]: primarily secure elements. Riksbank e-krona [S]: double-spending cannot be ruled out on unprotected phones.
- Attacks [S]: Basin, Sasse, Toro-Pozo, "The EMV Standard: Break, Fix, Verify", IEEE S&P 2021; Basin et al., USENIX Security 2023; offline-limit bypass on a Square terminal, USENIX Security 2025.
- Transferable e-cash [S]: Chaum & Pedersen EUROCRYPT '92 (coins grow per transfer); Baldimtsi et al. PKC 2015; Bauer, Fuchsbauer, Qian PKC 2021. Detect only, need an issuer.
- Cashu over BitChat [S]: receiver-locked tokens are final offline only if the sender was online to lock them; a fully offline sender can double-spend. Custodial mint.
- Ark out-of-round payments [S]: sender and server must not collude, structurally a single co-signer.
- [I] Every deployed offline-prevention design relies on trusted hardware our phones and Pis don't provide.

## 7. Reputation, credit and insurance

- Sardex mutual credit [S]: capped credit lines; defaults "cannot be entirely removed". Trustlines [S]: payments along bilateral credit paths.
- [I] Caps bound loss per partition; an insurance pool held under a FROST key spreads residual loss across acceptors but does not deter.

---

## 8. frost-tools and the "FROST and the Zcash Vault" guide

**[ZcashFoundation/frost-tools](https://github.com/ZcashFoundation/frost-tools) at `53df99c` (14 Oct 2026)** [S]:
- `frostd`, `frost-client` (trusted dealer "only for tests", DKG, coordinator, participant) and `zcash-sign`. README: "for demo purposes only and should not be used in production". MIT/Apache, Linux CI only.
- Ciphersuites: Ed25519 and re-randomized `PallasBlake2b512`. Commit `06c0dbd` (12 Aug 2026), "support Ironwood (v6) external signing", reports a 2-of-3 FROST signature authorizing a real testnet Ironwood spend.
- Key derivation uses `from_sk_ak_incompatible_with_quantum_recoverability_and_will_be_removed`, i.e. no ZIP 2005 `use_qsk`.
- Crates are upstream ECC: `frost-core 2.2.0`, `frost-rerandomized 2.1.0`, `zcash_primitives 0.29`, `pczt 0.8.0-rc.1`, `orchard 0.15.5` (patched fork). Not type-compatible with Zakura 2.2.0. [I] `frost-client` itself has no Zcash dependencies and exchanges only bytes (32-byte sighash, 32-byte randomizer α, 64-byte signature), so our Zakura builder can do the Zcash parts.
- One key package per identity; any t-of-n. [I] A weighted payer means running two identities; untested.
- Refresh and repair exist in `frost-core`, not in the tool; refresh cannot change the threshold.
- Transport: copy-paste CLI, socket, or HTTP via `frostd`; pluggable `Comms` trait. Two rounds, fresh nonces per session, messages about 100 bytes per signer. End-to-end encryption (`Noise_K_25519_ChaChaPoly_BLAKE2s`) only in HTTP mode.
- Signers see only `hex(message())` and a "(y/n)" prompt, marked "TODO: replace with callback". The coordinator's `aux_msg` is ignored by participants. [I] The nullifier registry hooks into a custom `Comms::confirm_message`: parse the transaction from `aux_msg`, recompute the sighash and require equality, check `rk = Y + α·G`, read `nf`, durably record `nf → sighash`, refuse a different sighash.

**"FROST and the Zcash Vault" (43-page guide, October 2026; it states it was written by Claude for the project owner, so treat it as secondary)** [S]:
- A generic FROST shielded multisig: DKG, `ak` = group key, coordinator wallet builds the PCZT and chooses α per spend, one FROST session per spend; signers check `rk = Y + α·G`, rebuild the sighash and check outputs, fee and expiry (pp. 28–34 of the PDF index).
- Ironwood groups should use ZIP 2005 `use_qsk = true`; ZF support is "blocked on a pending ZIP 312 update" (p. 31).
- Confidentiality is all-or-nothing within the group; α is confidential (pp. 30–31, 43).
- One silent signer aborts a session; retry with another subset; ROAST suggested (p. 39). Repair, refresh, share and `qsk` backup (pp. 23, 32, 40).
- Expiry 0 makes a signed transaction "a bearer instrument" (p. 40).
- No timelocks, bonds, fraud proofs, payee-side checks or quorum intersection. "Double spend" there means conflicting vault proposals, handled by note reservation (pp. 39–40).
- It describes `frost-rerandomized` 3.0; frost-tools uses 2.x.

---

## 9. Resulting design (inference)

| Layer | When | Mechanism | Guarantee | Main failure modes |
|---|---|---|---|---|
| 1. Quorum-certified notes | A quorum is reachable over the mesh | Payer (f+1 shares) + any 2f+1 of 3f+1 committee signers via FROST; per-nullifier registry at each signer; payee checks offline that the signature is under the committee group key (payer reveals `ak = Y` and α, payee checks `rk`) | Prevented if ≤ f signers are Byzantine | > f collusion can approve a double-spend (cannot steal without payer); minority partition cannot spend; equivocating payer locks the note; payer losing its shares, or > f signers lost before repair, strands the float; revealing α links the spend to the float key; no `use_qsk` today |
| 2. Bonded accountable intents | No quorum reachable | Signed intent per payment, per-epoch counter ≤ k, bond under a committee key that is never on the payment path, paid out on a fraud proof; acceptor floor limit and open intents ≤ bond ÷ k | Deterred and compensated | Fraud across many partitions exceeding the bond; threshold collusion steals bonds; refund needs `lock_time` or a live committee; honest backup-restore penalised |
| 3. Mesh-wide detection | Always | Gossip intents and fraud proofs as a signed log; feeds hot list and bond claims | Detected quickly | No finality on its own |

Staging: MVP v1 uses the pending tier with signed intents, fraud-proof detection and hot-list gossip (layers 2 and 3 without the bond). The bond, counters and a 4-device quorum come next; basecamp committees in the Logos phase.

## 10. Verify first

1. Upstream frost-tools sighash, α and signature bytes interoperate with a Zakura 2.2.0 Ironwood transaction on the local NU7 chain.
2. Offline payee check: the Ironwood proof binds `rk` to the note's `ak` under v6; what revealing α leaks.
3. A weighted payer (two identities) and the per-nullifier `confirm_message` hook in frost-client.
4. Whether Zakura enforces `lock_time` on fully shielded v6 transactions (bond refund path). Bitcoin-style `IsFinalTx` would ignore it with no transparent inputs.
5. Fraud-proof soundness: proving `nf` in another txid from light-client data, and that tx₁ cannot fail for innocent reasons (expiry, fee, anchor).
6. Equivocation-unlock protocol; key-loss recovery for the payer's shares.
7. How often a quorum is reachable on the real mesh.
