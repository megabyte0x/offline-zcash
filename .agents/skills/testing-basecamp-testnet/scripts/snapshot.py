"""Export selected lab diagnostics, never raw wallet, key, invite or transaction data."""

import collections
import datetime
import hashlib
import json
from pathlib import Path
import sys


def read_json(path):
    return json.loads(path.read_text()) if path.exists() else {}


def select(row, fields):
    return {key: row[key] for key in fields if key in row}


def snapshot(root):
    devices = {}
    important = {
        "started", "neighbor-up", "neighbor-down", "synced", "sync-error",
        "sync-served", "anchor-unknown-syncing", "refused", "accepted",
        "awaiting-inclusion", "receipt", "duplicate-pay", "error", "built",
        "payee-unreachable", "gateway-result", "result", "result-undelivered",
    }
    detail_fields = (
        "txid", "view_height", "tip", "chain_tip", "from_height", "code",
        "reason", "error", "state", "amount", "verify_ms", "new_notes",
        "advanced", "rescanned", "payload_bytes", "chunks", "spendable", "locked",
        "status", "gateway_reported", "prove_ms", "bytes",
    )
    for device in sorted(root.iterdir()):
        if not (device / "module_data/zcash_offline_module").is_dir():
            continue
        name = device.name
        versions = {}
        for directory in ("modules", "plugins"):
            for path in sorted((device / directory).glob("zcash_offline_*/manifest.json")):
                versions[path.parent.name] = read_json(path).get("version")
        homes = []
        for home in sorted((device / "module_data/zcash_offline_module").glob("*/ozm")):
            wallet = read_json(home / "data/wallet.json")
            book = read_json(home / "data/payments.json")
            events = collections.deque(maxlen=40)
            transaction_events = collections.deque(maxlen=200)
            counts = collections.Counter()
            incomplete_lines = 0
            log = home / "data/events.jsonl"
            if log.exists():
                with log.open() as lines:
                    for line in lines:
                        try:
                            event = json.loads(line)
                        except json.JSONDecodeError:
                            incomplete_lines += 1
                            continue
                        kind = event.get("event")
                        counts[kind] += 1
                        if kind in important:
                            safe = {
                                "ms": event.get("ms"),
                                "event": kind,
                                "detail": select(event.get("detail", {}), detail_fields),
                            }
                            events.append(safe)
                            if safe["detail"].get("txid"):
                                transaction_events.append(safe)
            unspent = [note for note in wallet.get("notes", []) if note.get("spent_in") is None]
            homes.append({
                "instance": home.parent.name,
                "wallet": {
                    **select(wallet, ("tip_height", "tip_hash", "birthday")),
                    "spendable_zat": sum(note["value"] for note in unspent if note.get("locked_by") is None),
                    "locked_zat": sum(note["value"] for note in unspent if note.get("locked_by") is not None),
                },
                "payments": [{
                    **select(payment, (
                        "txid", "state", "direction", "amount", "fee", "expiry_height", "created_ms",
                    )),
                    "transaction_sha256": hashlib.sha256(bytes.fromhex(payment["tx_hex"])).hexdigest(),
                    "receipt": select((payment.get("receipt") or {}).get("body", {}), (
                        "state", "view_height", "created_ms",
                    )),
                    "result": select((payment.get("result") or {}).get("body", {}), (
                        "status", "tip_height", "created_ms",
                    )),
                } for payment in book.get("payments", {}).values()],
                "refusals": [{
                    **select(refusal, ("ms", "txid")),
                    "reject": select(refusal.get("reject", {}), ("code", "reason")),
                } for refusal in book.get("refusals", [])],
                "gateway_submission_count": len(book.get("gateway", {})),
                "pending_checks": len(book.get("deferred_payments", {})),
                "auto_preparation": select(book, ("auto_prepare_status", "auto_prepare_txid")),
                "event_counts": dict(counts),
                "incomplete_event_lines": incomplete_lines,
                "recent_events": list(events),
                "transaction_events": list(transaction_events),
            })
        engine = device / "modules/zcash_offline_module/zcash_offline_module_plugin.so"
        devices[name] = {
            "versions": versions,
            "engine_sha256": hashlib.sha256(engine.read_bytes()).hexdigest() if engine.is_file() else None,
            "instances": homes,
        }
    return {
        "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "devices": devices,
    }


if __name__ == "__main__":
    print(json.dumps(snapshot(Path(sys.argv[1])), indent=2))
