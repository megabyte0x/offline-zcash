# How it works

Alice pays Bob with no internet. Her device builds and proves a Zcash transaction offline, the mesh carries it hop by hop, Bob checks it and replies, and a gateway with internet gets it mined. The result travels back the same way.

## 1. The payment journey

```diagram
Alice ──hop──▶ relay ──hop──▶ Bob ──hop──▶ relay ──hop──▶ Gateway ──▶ Zcash chain
  ▲                            │                            │            │
  └──── receipt ◀──────────────┘                            │       mined at H
  └──── mined at H ◀─── result travels back over the mesh ◀─┘◀───────────┘
```

1. **Alice builds and proves** a v6 Ironwood transaction on her device, offline, from notes she synced earlier.
2. **The mesh carries it.** Each device that hears the packet stores it and passes it on (up to 7 hops) until Bob acknowledges it. If Bob is in range directly, no relay is needed.
3. **Bob checks it offline**: the proofs, the signatures and that the output pays him. He saves it, then sends Alice a signed receipt: `accepted-pending`.
4. **Alice and Bob both forward the bytes** towards any gateway, so the payment still gets out if one of them goes away.
5. **The gateway submits** the transaction to the Zcash node (submitting twice is harmless) and it is mined.
6. **The result comes back**: the gateway signs "mined at height H" and floods it to the mesh. Alice and Bob both mark the payment `mined`.

## 2. Inside the mesh

Every message is sealed in an `ozm1` envelope for its recipient and split into 512-byte frames. Relays see only the frames, not the amount or the parties.

| Step | Alice (payer) | Relay | Bob (payee) | Gateway |
|---|---|---|---|---|
| Build | `built` | | | |
| Send `pay` | `sent` | stores and forwards each frame, TTL − 1 | | |
| Delivered | `delivered` (mesh ack) | forwards the ack | reassembles, checks, saves | |
| Receipt `rcpt` | `accepted-pending` (or `refused` with a reason code) | forwards | `accepted-pending`, then forwards `sub` | |
| Submit `sub` | | forwards | `submitted` | `sendrawtransaction` |
| Result `res` | `mined` | forwards | `mined` | signs "mined at H" |

A device that is offline when a packet passes gets it later: neighbours keep packets and re-send them when a link comes up, and the app retries by txid until it sees an ack.

## 3. The gateway bridge

The gateway is a normal mesh device that also has internet.

1. It receives `sub` messages from the mesh (from Alice, from Bob, or both).
2. It calls `sendrawtransaction` on the Zcash node. "Already in mempool" and "already mined" count as success.
3. On the local Regtest chain it also mines a block, because the chain only mines on demand.
4. It watches for the transaction in a block and floods a signed `res` back to the mesh.
5. It also answers `syncq` (chain state for wallets that cannot reach the chain) and, on test chains, `fundq` (test funds).

The result is "gateway-reported": the payee trusts the gateway's signature. Proving inclusion with headers and a Merkle branch is designed ([gateway](components/gateway.md), [payee-verifier](components/payee-verifier.md)) but not built yet.

More detail: [mvp-v0.1.0.md](mvp-v0.1.0.md) (gating tests and timeline), [components/mesh.md](components/mesh.md) (mesh as built), [design.md](design.md) (the mainnet design).
