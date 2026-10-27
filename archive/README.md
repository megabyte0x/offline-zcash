# Archive

Original research and reviews, kept as deep sources. Do not edit. Where anything here conflicts with [`docs/`](../docs/README.md), `docs/` wins.

| File | What it is | Superseded by |
|---|---|---|
| `reports/Offline Zcash over Logos mesh.md` | The original long research report (testnet-oriented) | `docs/` as a whole |
| `reports/MVP v1 design.md` | MVP design, already updated with the review fixes | `docs/design.md` and `docs/components/` |
| `reports/Design review reconciliation.md` | Comparison of the two reviews and the re-checks that settled contradictions | `docs/components/*` "Decided" sections |
| `reports/Double-spend design 2026-10-25.md` | Single 2-of-2 co-signer design, source-verified PCZT/FROST API facts, topology and prior-art tables | `docs/components/double-spend.md` (26 Oct) |
| `docs-2026-10-27/transport-bitchat.md` | Plan to fork BitChat on four Macs (`privateMessageV2`, durability rules, BLE rates, Offline Protocol evaluation) | `docs/components/mesh.md`: we built `ozm-mesh` with BitChat semantics instead |
| `docs-2026-10-27/later/logos*.md` | Logos pathway: Logos Delivery, `mesh_transport` contract, `mesh_offlineprotocol`, Basecamp plan and upstream issue drafts | `logos/` (built Basecamp core module and QML app) and `docs/basecamp-install.md` |
| `research_notes/Offline Zcash over Logos mesh/*.md` | Nine source notes (BitChat, Logos messaging and modules, Ironwood, physical links, threat model, double-spend, card networks, prior art) | Per-component docs, which cite them |

Known wrong or outdated statements in the long report and notes:

- "Proof the nullifier was never spent" from a chain bundle: impossible; no header commits to the nullifier set (ZIP 221).
- The signed intent sent to relays and gateways: replaced by tx bytes only; details go to the payee privately.
- `lock_time` as a bond refund path: ignored on fully shielded transactions. Transparent CLTV *is* enforced on v6 (measured 26 Oct), but bonds stay dropped: a self-sweep produces no second statement to slash.
- "Total offline liability ≤ k × floor limit": false across partitions.
- "Same nullifier, different txid" as proof of fraud: it is conflict evidence only.
- Three Logos Store nodes needed: two suffice.

The two reviews themselves live in Amp threads internal research notes and internal research notes.
