#!/usr/bin/env bash
# v0.2.0 bench: the tester path on one shared LAN, with no manual wiring.
#
#   A payer ┐
#   C payee ├── ozm-lan (internal: no internet) ──┐
#   B payee ┘                                    D gateway ── ths-default ── Zakura (ths)
#
# Every device runs `ozm-node join <invite>` then `ozm-node start` (LAN discovery,
# no --dial). Only D can reach the chain. A gets test funds from D over the mesh,
# A and B sync their wallets from D over the mesh, A pays B, and B's balance
# rises once D reports the payment mined.
#
# Usage: bench/lan-bench.sh [all|up|test|down]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/bench/.state-lan"
IMG="ozm-node:bench"
RESULTS="$STATE/results.jsonl"
DEVICES=(A B C D)
declare -A PORT=([A]=7791 [B]=7792 [C]=7793 [D]=7794)

log() { printf '\n\033[1m==> %s\033[0m (%(%T)T)\n' "$*" -1; }
ok() { printf '  \033[32mPASS\033[0m %s\n' "$*"; echo "{\"test\":$(jq -Rn --arg s "$1" '$s'),\"pass\":true}" >>"$RESULTS"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$*"; echo "{\"test\":$(jq -Rn --arg s "$1" '$s'),\"pass\":false}" >>"$RESULTS"; FAILED=1; }
node() { local n=$1; shift; docker exec "lan-$n" ozm-node "$@"; }
wait_until() {
  local t=$1 d=$2; shift 2
  local end=$((SECONDS + t))
  while ((SECONDS < end)); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  echo "  timeout after ${t}s waiting for: $d" >&2; return 1
}
spendable_at_least() {
  local fees
  fees=$(node "$1" payments | jq '[.payments[] | select(.direction == "prepare" and .state == "mined") | .fee] | add // 0')
  node "$1" status | jq -e --argjson z "$2" --argjson fees "$fees" '.wallet.spendable + $fees >= $z'
}
pay_state() { node "$1" payments | jq -e --arg t "$2" --arg s "$3" '.payments[] | select(.txid==$t and .state==$s)'; }

down() {
  log "Removing LAN bench"
  for n in "${DEVICES[@]}"; do docker rm -f "lan-$n" >/dev/null 2>&1 || true; done
  docker network rm ozm-lan >/dev/null 2>&1 || true
  if [[ -d "$STATE" ]]; then sudo -n rm -rf "$STATE" 2>/dev/null || rm -rf "$STATE"; fi
}

start_device() { # NAME EXTRA_START_ARGS...
  local n=$1; shift
  docker run -d --name "lan-$n" --hostname "$n" --network ozm-lan \
    -p "127.0.0.1:${PORT[$n]}:7780" -v "$STATE/invites:/invites:ro" \
    --entrypoint sh "$IMG" -c \
    "ozm-node join /invites/$n.invite.json --home /home/ozm && exec ozm-node start --home /home/ozm --no-open --api 0.0.0.0:7780 $*" >/dev/null
}

up() {
  log "Building ozm-node and the runtime image"
  (cd "$ROOT" && cargo build --release -p ozm-node 2>&1 | tail -1)
  mkdir -p "$STATE/ctx"
  cp "$ROOT/target/release/ozm-node" "$STATE/ctx/"
  docker build -q -t "$IMG" -f "$ROOT/bench/Dockerfile" "$STATE/ctx" >/dev/null

  log "Fleet and invites (A payer, B payee, C payee, D gateway)"
  "$ROOT/target/release/ozm-node" fleet-init --out "$STATE/invites" --name lan-bench --network regtest \
    --device A --device B --device C --device D:gateway
  chmod 644 "$STATE"/invites/*.json  # disposable Regtest keys; containers read them as root

  log "Funding the gateway's faucet wallet from ths (2 × 1 ZEC)"
  local addr_d; addr_d=$(jq -r '.devices[] | select(.name=="D") | .address' "$STATE/invites/fleet.json")
  for _ in 1 2; do ths faucet "$addr_d" --amount 1 --json | jq -r '"  faucet \(.txid[0:16])… \(.status)"'; done

  log "One LAN, no --dial anywhere"
  docker network create --internal ozm-lan >/dev/null
  start_device D --rpc http://zakura:18232 --mine
  docker network connect ths-default lan-D
  for n in A B C; do start_device "$n"; done
  wait_until 30 "discovery links" bash -c "$(declare -f node); for n in A B C D; do node \$n status | jq -e '.neighbors | length == 3' || exit 1; done"
  for n in "${DEVICES[@]}"; do echo "  $n neighbours: $(node "$n" status | jq -c .neighbors)"; done
}

test_all() {
  FAILED=0; : >"$RESULTS"
  log "Topology"
  for n in A B C; do
    if docker exec "lan-$n" curl -s -m 3 http://zakura:18232 >/dev/null 2>&1; then fail "$n cannot reach the chain"; else ok "$n cannot reach the chain (offline)"; fi
  done
  local links=0 c
  for n in "${DEVICES[@]}"; do c=$(node "$n" status | jq '.neighbors | length'); links=$((links + c)); done
  [[ "$links" == 12 ]] && ok "discovery: every device found the other three with no --dial (one link per pair)" || fail "discovery links = $links/12"

  log "Gateway sync over the mesh"
  node D sync >/dev/null
  wait_until 60 "B synced from D" bash -c "$(declare -f node); node B status | jq -e '.wallet.tip > 0'" \
    && ok "B (offline) synced its chain view from D over the mesh: tip $(node B status | jq .wallet.tip)" || fail "B mesh sync"

  log "Test funds over the mesh"
  node A fund --notes 3 --amount 0.1 >/dev/null
  if wait_until 240 "A funded" bash -c "$(declare -f node spendable_at_least); spendable_at_least A 30000000"; then
    ok "A got 3 × 0.1 ZEC from D's faucet and saw it after a mesh sync"
  else fail "A funding"; fi

  log "Pay over the LAN mesh, mined, B's balance updates"
  local tx; tx=$(node A pay --to B --amount 0.05 | jq -r .txid)
  echo "  txid $tx"
  if wait_until 120 "mined at A and B" bash -c "$(declare -f node pay_state); pay_state A $tx mined && pay_state B $tx mined"; then
    ok "payment A→B accepted offline, mined by D, result at A and B"
  else fail "payment mined"; fi
  wait_until 90 "B balance" bash -c "$(declare -f node spendable_at_least); spendable_at_least B 5000000" \
    && ok "B's spendable balance shows 0.05 ZEC after syncing over the mesh" || fail "B balance after mesh sync"

  log "B automatically prepares its single received note"
  wait_until 90 "B split mined and scanned" bash -c "$(declare -f node); node B status | jq -e '.wallet.notes > 1 and .wallet.locked == 0'" \
    && ok "B received smaller spendable notes automatically, without spending unmined change" || fail "B automatic preparation"

  log "B spends what it received (mined, synced over the mesh)"
  local tx2; tx2=$(node B pay --to C --amount 0.01 | jq -r .txid)
  if wait_until 120 "mined at B and C" bash -c "$(declare -f node pay_state); pay_state B $tx2 mined && pay_state C $tx2 mined"; then
    ok "B paid C 0.01 ZEC from received funds; mined"
  else fail "B→C payment"; fi

  log "Restart a device: it rediscovers the LAN"
  docker restart lan-C >/dev/null
  wait_until 60 "C relinked" bash -c "$(declare -f node); node C status | jq -e '.neighbors | length == 3'" \
    && ok "C rejoined after restart via discovery" || fail "C rediscovery"

  log "Summary"
  jq -s '{passed: map(select(.pass)) | length, failed: map(select(.pass|not)) | length}' "$RESULTS"
  for n in "${DEVICES[@]}"; do docker logs "lan-$n" >"$STATE/$n.log" 2>&1; done
  return "$FAILED"
}

case "${1:-all}" in
  up) down; up ;;
  test) test_all ;;
  down) down ;;
  all) down; up; test_all ;;
  *) echo "usage: $0 [all|up|test|down]"; exit 2 ;;
esac
