# Card-network offline acceptance (in-flight Visa/EMV) and what transfers to offline Zcash

Scope: how airlines and other in-transit merchants accept Visa and Mastercard with no connectivity, then sync after landing. The notes cover what the offline check proves, who bears the loss, which controls limit exposure, and which of those controls can be reused for the "pending" acceptance tier of an offline Ironwood payment between basecamps.

---

## 1. How in-flight card acceptance works

### Takeaway
Card networks do not prevent offline losses. They bound them and assign them. The offline check proves the card is genuine and within limits the issuer chose in advance. It never checks the balance. The money question is settled hours later, and the merchant usually absorbs any decline. This is the same shape as the report's "pending" tier, with four extra controls: a floor limit, an offline counter enforced per payer, a deny list loaded before departure, and an explicit owner for the loss.

### Cited Findings

**Two different offline modes exist, with different risk owners**
- *EMV offline authorization.* The terminal asks the chip for approval without contacting the issuer. "If the transaction amount is below both limits [the terminal floor limit and the card's offline risk parameters], it may be approved offline and the card generates the TC" (Transaction Certificate). Otherwise the chip returns an AAC (decline). — [EMV Migration Forum / US Payments Forum, "Merchant Processing During Communications Disruptions", Apr 2016](https://www.emv-connection.com/downloads/2016/04/Merchant-Processing-during-Communication-Disruption-FINAL-April-2016.pdf)
- Liability for offline authorization: counterfeit liability follows normal EMV rules because "an authentic EMV transaction occurred". But "for networks with zero floor limits or for a merchant that approves a transaction above the network floor limits, the transaction may be cleared, but may not provide protection against insufficient funds." — same source
- *Deferred authorization.* The merchant captures the transaction while offline and sends a normal online authorization request after connectivity returns. "Ultimately the merchant may be responsible for any loss incurred if the authorization is declined by the issuer and the customer is allowed to leave with the merchandise." — same source
- Visa defines Offline Authorization as "an Issuer-controlled process that allows a Chip-initiated Authorization Request to be processed in a below-Floor Limit environment without sending the request to the Issuer", and the Floor Limit as the amount "above which Online Authorization is required". — [Visa Core Rules, 18 Apr 2026 edition, Glossary](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf)

**The card enforces its own offline exposure**
- EMV terminal risk management checks the floor limit, randomly selects some below-limit transactions for online processing, and checks velocity. Velocity uses the card's Lower Consecutive Offline Limit (tag 9F14), Upper Consecutive Offline Limit (tag 9F23), the Application Transaction Counter and the Last Online ATC Register. Past the lower limit the transaction should go online if possible; past the upper limit no more offline transactions are allowed. — [OpenSCDP EMV tutorial](https://www.openscdp.org/scripts/tutorial/emv/terminalriskmanagement.html)
- Visa's issuer duties for offline-capable cards include "Initiating Online Issuer Authorization for above-Floor Limit Transactions", "Ensuring random Online processing for below-Floor Limit Transactions" and "Performing Transaction velocity checking". — [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf)
- Contactless chip cards that support offline authorization must support DDA and must not support static data authentication (Visa Rules 4.1.17.9). Offline authentication must be dynamic, so a cloned static record cannot approve itself. — same source
- These counters live in the card's secure element. The card is the tamper-resistant enforcement point, which Zcash does not have (see section 2).

**Sync after landing has a deadline**
- A deferred authorization must carry a deferred indicator (value 5206 in Field 63.3 since 16 April 2021). — [Visa, Deferred Authorization Indicator Reminder, AI10879](https://usa.visa.com/dam/VCOM/global/support-legal/documents/deferred-authorization-indicator-reminder.pdf)
- Visa Rules 5.7.3.3: request authorization within 4 days for transit MCCs 4111/4112/4131, and "within 24 hours of the Transaction Date" for all other MCCs. In-Transit Transactions (purchases "on board a ship, boat, bus, aircraft, ferry, or train") must obtain an authorization within 24 hours. — [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf)
- Mastercard gateways mark these the same way (`transaction.deferredAuthorization = TRUE`) and describe the indicator as mandatory for Visa. — [Mastercard Gateway integration guide](https://na-gateway.mastercard.com/api/documentation/integrationGuidelines/supportedFeatures/pickTransactionCommands.html?locale=en_US)

**Airlines absorb residual loss and use pre-loaded deny lists**
- GuestLogix, which said it served about 90% of North American and 40% of global passenger airlines, pre-loads a "Hot List" of cards "suspected of being fraudulent and declined anywhere in the world within the previous several hours" into the in-flight payment system "immediately before each departure", updated several times a day. — [American Banker](https://www.americanbanker.com/payments/news/guestlogix-rolls-out-travel-industry-hot-list-to-block-fraudulent-cards) (article date not shown on the fetched page)
- "Sometimes inflight transactions that were approved offline during a flight are declined afterward for one reason or another, and the airline absorbs that loss." Resubmitting declined transactions recovered "anywhere from 15% to 40% of funds". — same source
- Payment processors put all offline risk on the merchant. Adyen requires a chip floor limit and a contactless floor limit before offline payments can be enabled, and caps store-and-forward count and total amount per terminal. A merchant guide built on it states: "You are fully liable for the risk of failed captures, chargebacks, and disputes related to payments processed offline." — [Adyen offline payments](https://docs.adyen.com/point-of-sale/offline-payment); [Ventrata help centre](https://support.ventrata.com/en/articles/13384674-how-to-activate-offline-payments-for-adyen-terminals)
- JP Morgan's store-and-forward function filters on maximum transaction amount, maximum queue total amount and maximum queue count. — [JP Morgan Payments developer portal](https://developer.payments.jpmorgan.com/docs/commerce/in-store-payments/capabilities/payment-terminal-application/saf)
- Acquirers must monitor the share and daily count of below-floor-limit transactions per merchant, and same-BIN totals per outlet per day, against thresholds they set (Visa Rules, Risk chapter). — [Visa Core Rules, Apr 2026](https://usa.visa.com/content/dam/VCOM/download/about-visa/visa-rules-public.pdf)

**Regulatory note (EU)**
- The EBA answered a Panasonic Avionics question about on-board purchases over a closed aircraft network: SCA still applies, a closed wireless network without a POS terminal "may not allow SCA to be applied", but an offline POS terminal can verify a PIN offline. — [EBA Q&A 2019_4740](https://www.eba.europa.eu/single-rule-book-qa/qna/view/publicId/2019_4740)
- This is irrelevant to testnet TAZ but shows that regulators treat "offline" as a terminal-plus-secure-chip problem, not a network problem.

---

## 2. Why the card model works, and which assumptions Zcash lacks

| Card-network assumption | Holds for Ironwood between basecamps? | Consequence |
|---|---|---|
| The payer is identified, and a failed charge becomes a debt the issuer can collect | No. ZEC is a bearer asset; a double-spender keeps the coins. | Recourse must come from an off-chain agreement between basecamps whose keys were exchanged in person, backed by the payer's intent signature as evidence |
| The card's secure element enforces offline counters the payer cannot reset | No. No spend-once hardware exists for Zcash, and the report rejected that route | Counters must be enforced by each payee locally (per payer key) or by a FROST co-signer, which is the closest analogue to "go online" |
| Offline approval proves only that the card is genuine | Zcash is stronger here. The payee verifies the Halo 2 proof, so the spent notes existed at the anchor | The offline check is better than a card's; only the "still unspent" question remains open |
| The merchant chooses to absorb losses below a limit | Applies directly | The payee's acceptance policy must name who absorbs a failed pending payment |
| Sync deadline (24 h, 4 days for transit) | Maps to the expiry height and gateway cadence | Already in the report as the fleet expiry delta |

My inference: the controls that limit card losses offline are mostly *policy at the acceptor*, not cryptography. Those transfer. The two controls that rely on the card being a trusted device (tamper-resistant counters, issuer credit) do not; FROST co-signing is the replacement for the first, and a bilateral agreement or deposit is the replacement for the second.

---

## 3. Reusable controls for the "pending" tier (design inference)

1. **Floor limit per payment.** A payee accepts a "pending" Ironwood payment only below a fleet-wide amount. Above it the payment needs a co-signer attestation, the equivalent of going online.
2. **Offline counters per payer key.** Each payee tracks, per payer basecamp key, the count and total value of pending payments it has accepted since that payer's last confirmed on-chain settlement. A lower limit asks for a co-signer when one is reachable; an upper limit refuses. The counter resets when the gateway chain bundle shows the payer's earlier payments mined. The card enforces its counter itself; here each payee enforces only its own view, so a payer can still spread pending payments across several payees. Fleet-wide exposure per payer is therefore up to (number of payees) × (upper limit).
3. **Random escalation.** A small random fraction of below-limit payments is escalated to a co-signer anyway, as Visa requires "random Online processing". This makes a payer's limit-gaming detectable.
4. **Hot list in the chain bundle.** Gateways publish, signed, the basecamp keys whose intent signatures appear on a double-spend (a nullifier mined in a different txid), plus nullifiers already spent on chain. Payees load it at each sync and refuse pending payments from listed keys. Unlike a card hot list, the spent-nullifier list is authoritative as of the bundle height.
5. **Explicit loss owner.** The fleet agreement states that a payee who accepts at "pending" absorbs a failed payment, and that the payer whose intent signature is on a double-spend must make the payee whole (or forfeits a deposit held by another camp). This replaces issuer debt collection.
6. **Settlement deadline.** Already present as the fleet expiry delta. Card rules show the deadline is set by the uplink cadence of the slowest acceptor, which matches the report's 3 × daily-uplink rule.
7. **Resubmission.** Airlines recover 15–40% of declined in-flight funds by resubmitting. The Zcash analogue is the report's rule to re-issue an expired payment from the same notes. A payment that failed because of a mined double-spend cannot be resubmitted; it can only be pursued under item 5.

---

## 4. Open questions

- What floor limit and counter values make sense? Testnet TAZ is worthless, so values can only be simulated with notional amounts.
- Can a payee learn a payer's earlier pending payments to other payees without a co-signer, for example from envelopes on the shared topic? The trial-decryption design hides them by intent, so probably not.
- Is the hot list itself a privacy leak? Listing a basecamp key reveals who double-spent; listing nullifiers reveals nothing new because they are already on chain.
- Does the fleet want deposits (collateral) held by a third camp, and in what form? A FROST escrow would work but re-imports the co-signer liveness problem.
