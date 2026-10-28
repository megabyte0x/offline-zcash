#!/usr/bin/env bash
# v0.1.0 bench: four devices in four containers, linked as a line A – R – B – D.
#
#   A payer ──ozm-ar── R relay ──ozm-rb── B payee ──ozm-bd── D gateway ──ths-default── Zakura (ths)
#
# Each link is an internal Docker network (no internet), so a packet from A to B
# must hop through R, and only D can reach the chain. Frames are 512 bytes with a
# per-frame delay to emulate BLE. A and B sync their wallets once in a one-shot
# container on ths-default ("online"), then run with no route to the chain.
#
# Usage: bench/mesh-bench.sh [all|up|test|down]   (default: all = down, up, test)
# Needs: Docker, a running `ths` chain (amp orb services ensure), cargo, jq, python3.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/bench/.state"
IMG="ozm-node:bench"
RPC_HOST="http://127.0.0.1:18232"
FRAME_DELAY_MS="${FRAME_DELAY_MS:-20}"
AMOUNT="${AMOUNT:-0.05}"
declare -A PORT=([A]=7781 [R]=7782 [B]=7783 [D]=7784)
RESULTS="$STATE/results.jsonl"

log() { printf '\n\033[1m==> %s\033[0m (%(%T)T)\n' "$*" -1; }
ok() { printf '  \033[32mPASS\033[0m %s\n' "$*"; echo "{\"test\":$(jq -Rn --arg s "$1" '$s'),\"pass\":true}" >>"$RESULTS"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$*"; echo "{\"test\":$(jq -Rn --arg s "$1" '$s'),\"pass\":false}" >>"$RESULTS"; FAILED=1; }
rpc() { curl -s -X POST -H 'content-type: application/json' --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}" "$RPC_HOST" | jq -c .result; }
node() { local n=$1; shift; docker exec "ozm-$n" ozm-node "$@"; }
payments() { node "$1" payments; }
now_ms() { date +%s%3N; }

# wait_until TIMEOUT_S DESCRIPTION CMD...  (CMD must exit 0 when the condition holds)
wait_until() {
  local t=$1 d=$2; shift 2
  local end=$((SECONDS + t))
  while ((SECONDS < end)); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep 0.5
  done
  echo "  timeout after ${t}s waiting for: $d" >&2
  return 1
}
pay_state() { payments "$1" | jq -e --arg t "$2" --arg s "$3" '.payments[] | select(.txid==$t) | select(.state==$s)'; }

down() {
  log "Removing containers and networks"
  docker rm -f ozm-A ozm-R ozm-B ozm-D >/dev/null 2>&1 || true
  for n in ozm-ar ozm-rb ozm-bd; do docker network rm "$n" >/dev/null 2>&1 || true; done
  if [[ -d "$STATE" ]]; then sudo -n rm -rf "$STATE" 2>/dev/null || rm -rf "$STATE"; fi
}

start_node() { # NAME FIRST_NET "OTHER_NETS" DIAL EXTRA_ENV...
  local n=$1 first=$2 others=$3 dial=$4; shift 4
  docker run -d --name "ozm-$n" --hostname "$n" --network "$first" \
    --cap-add NET_ADMIN --cap-add NET_RAW \
    -p "127.0.0.1:${PORT[$n]}:7780" \
    -v "$STATE/$n:/data" -v "$STATE/fleet:/fleet:ro" \
    -e OZM_DATA=/data -e OZM_FLEET=/fleet/fleet.json -e "OZM_SECRET=/fleet/$n.secret.json" \
    -e OZM_LISTEN=0.0.0.0:7777 -e OZM_API=0.0.0.0:7780 -e "OZM_DIAL=$dial" \
    -e "OZM_FRAME_DELAY_MS=$FRAME_DELAY_MS" -e OZM_RETRY_SECS=5 -e OZM_DEBUG_API=true \
    -e RUST_LOG=info "$@" "$IMG" run >/dev/null
  for net in $others; do docker network connect "$net" "ozm-$n"; done
}

up() {
  log "Building ozm-node (release) and the runtime image"
  (cd "$ROOT" && cargo build --release -p ozm-node 2>&1 | tail -1)
  mkdir -p "$STATE/ctx"
  cp "$ROOT/target/release/ozm-node" "$STATE/ctx/"
  docker build -q -t "$IMG" -f "$ROOT/bench/Dockerfile" "$STATE/ctx" >/dev/null

  log "Checking the ths chain"
  local info; info=$(rpc getblockchaininfo '[]')
  echo "  tip $(jq .blocks <<<"$info"), consensus $(jq -r .consensus.chaintip <<<"$info")"
  [[ "$(jq -r .consensus.chaintip <<<"$info")" == "77190ad9" ]] || { echo "chain is not NU7"; exit 1; }

  log "Creating fleet keys (A payer, R relay, B payee, D gateway)"
  "$ROOT/target/release/ozm-node" fleet-init --out "$STATE/fleet" --name bench --network regtest \
    --device A --device R --device B --device D:gateway
  chmod 600 "$STATE"/fleet/*.json  # read-only mounts retain private modes; containers run as root

  log "Funding A with 10 notes of 0.1 ZEC from the ths faucet"
  local addr_a; addr_a=$(jq -r '.devices[] | select(.name=="A") | .address' "$STATE/fleet/fleet.json")
  for i in $(seq 10); do ths faucet "$addr_a" --amount 0.1 --json | jq -r '"  faucet \(.txid[0:16])… \(.status)"'; done

  log "Online step: A and B sync their wallets, then go offline"
  for n in A B; do
    mkdir -p "$STATE/$n"
    docker run --rm --network ths-default -v "$STATE/$n:/data" -v "$STATE/fleet:/fleet:ro" "$IMG" \
      wallet-sync --data /data --secret "/fleet/$n.secret.json" --rpc http://zakura:18232 | sed "s/^/  $n /"
  done
  mkdir -p "$STATE/R" "$STATE/D"

  log "Mesh links"
  for net in ozm-ar ozm-rb ozm-bd; do docker network create --internal "$net" >/dev/null; echo "  $net (internal)"; done
  start_node D ozm-bd "ths-default" "" -e OZM_RPC=http://zakura:18232 -e OZM_MINE=true
  start_node R ozm-ar "ozm-rb" ""
  start_node B ozm-rb "ozm-bd" "ozm-R:7777,ozm-D:7777"
  start_node A ozm-ar "" "ozm-R:7777"
  wait_until 30 "mesh links up" bash -c "docker exec ozm-B ozm-node status | jq -e '.neighbors | length == 2' && docker exec ozm-A ozm-node status | jq -e '.neighbors == [\"R\"]'"
  for n in A R B D; do echo "  $n neighbours: $(node "$n" status | jq -c .neighbors)"; done
}

test_all() {
  FAILED=0
  : >"$RESULTS"
  log "Topology checks"
  for n in A R B; do
    if docker exec "ozm-$n" curl -s -m 3 http://zakura:18232 >/dev/null 2>&1; then fail "$n cannot reach the chain"; else ok "$n cannot reach the chain (offline)"; fi
  done
  if docker exec ozm-A curl -s -m 3 http://ozm-B:7780 >/dev/null 2>&1; then fail "A has no direct link to B"; else ok "A has no direct link to B (must hop through R)"; fi
  docker exec ozm-D curl -s -m 3 -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"getblockcount","params":[]}' http://zakura:18232 >/dev/null && ok "D reaches the chain (gateway)" || fail "D reaches the chain"

  # ---------------- Test 1 + 6 ----------------
  log "Test 1: happy path A → R → B, receipt to A, B → D, mined, result to A and B (with packet capture on R)"
  docker exec -d ozm-R tcpdump -i any -U -w /data/cap.pcap tcp port 7777
  sleep 1
  local t0; t0=$(now_ms)
  local tx1; tx1=$(node A pay --to B --amount "$AMOUNT" | jq -r .txid)
  echo "  txid $tx1"
  if wait_until 120 "A and B see mined" bash -c "$(declare -f node payments pay_state); pay_state A $tx1 mined && pay_state B $tx1 mined"; then
    ok "1 happy path: payment mined, both A and B know"
  else
    fail "1 happy path"
  fi
  local t1; t1=$(now_ms)
  docker exec ozm-R pkill tcpdump || true
  local height; height=$(rpc getrawtransaction "[\"$tx1\",1]" | jq .height)
  echo "  on chain at height $height; end to end $((t1 - t0)) ms"
  [[ "$height" =~ ^[0-9]+$ ]] && ok "1 chain confirms $tx1 at height $height" || fail "1 chain lookup"
  local hops; hops=$(node B events | jq -r '[.[] | select(.event=="mesh-recv" and .detail.kind=="pay")][0].detail.hops')
  [[ "$hops" == "2" ]] && ok "1 pay crossed 2 hops (A→R→B)" || fail "1 pay hops = $hops"
  timeline "$tx1" "$t0"

  log "Test 6: payment bodies stay sealed; P12 nonce/status broadcasts expose txids"
  local txhex; txhex=$(payments A | jq -r --arg t "$tx1" '.payments[] | select(.txid==$t) | .tx_hex')
  local addr_b; addr_b=$(jq -r '.devices[] | select(.name=="B") | .address' "$STATE/fleet/fleet.json")
  if python3 - "$STATE/R/cap.pcap" "$txhex" "$tx1" "$addr_b" "$AMOUNT" <<'PY'
import sys
cap = open(sys.argv[1], 'rb').read()
tx = bytes.fromhex(sys.argv[2]); txid = sys.argv[3]; addr = sys.argv[4]; amount = sys.argv[5]
assert len(cap) > len(tx), f"capture too small ({len(cap)} bytes)"
chunks = [tx[i:i+24] for i in range(0, len(tx) - 24, 24)]
hits = sum(1 for c in chunks if c in cap)
needles = {"payee address": addr.encode(), "tx hex": sys.argv[2][:64].encode(), "amount text": amount.encode(),
           "amount zat LE": int(round(float(amount) * 1e8)).to_bytes(8, 'little')}
found = [k for k, v in needles.items() if v in cap]
assert b"ozm1-result:" in cap and txid.encode() in cap, "expected public P12 result not captured"
print(f"  capture {len(cap)} bytes; {hits}/{len(chunks)} 24-byte tx chunks found; plaintext needles found: {found}")
sys.exit(1 if hits or found else 0)
PY
  then ok "6 no raw transaction, address or amount; txid/status intentionally public (P12)"; else fail "6 payment-body leak or missing P12 result"; fi

  # ---------------- Test 2 ----------------
  log "Test 2: B is offline when A pays; B comes back later"
  docker stop ozm-B >/dev/null
  t0=$(now_ms)
  local tx2; tx2=$(node A pay --to B --amount "$AMOUNT" | jq -r .txid)
  sleep 4
  local stored; stored=$(node R status | jq .mesh.stored)
  echo "  R holds $stored packets for store-and-forward; A state: $(payments A | jq -r --arg t "$tx2" '.payments[] | select(.txid==$t) | .state')"
  docker start ozm-B >/dev/null
  if wait_until 120 "mined after B returns" bash -c "$(declare -f node payments pay_state); pay_state A $tx2 mined && pay_state B $tx2 mined"; then
    ok "2 B offline: delivered when B returned, mined, both know ($(( $(now_ms) - t0 )) ms)"
  else fail "2 B offline"; fi

  # ---------------- Test 3 ----------------
  log "Test 3: B stores and receipts; A leaves; B restarts; D appears later"
  # A mining result can precede the payee's wallet scan. Both need the anchor before D leaves.
  local tip; tip=$(rpc getblockcount '[]')
  for n in A B; do
    node "$n" sync >/dev/null
    wait_until 90 "$n scanned the anchor before disconnect" bash -c "docker exec ozm-$n ozm-node status | jq -e '.wallet.tip == $tip'"
  done
  docker stop ozm-D >/dev/null
  local tx3; tx3=$(node A pay --to B --amount "$AMOUNT" | jq -r .txid)
  if wait_until 60 "A has B's receipt" bash -c "$(declare -f node payments pay_state); pay_state A $tx3 accepted-pending"; then
    ok "3a A got B's signed receipt (pending) while no gateway exists"
  else fail "3a receipt"; fi
  docker stop ozm-A >/dev/null
  docker restart ozm-B >/dev/null
  wait_until 30 "B back" docker exec ozm-B ozm-node status
  payments B | jq -e --arg t "$tx3" '.payments[] | select(.txid==$t)' >/dev/null && ok "3b B kept the payment across restart" || fail "3b B lost the payment"
  docker start ozm-D >/dev/null
  if wait_until 120 "B sees mined" bash -c "$(declare -f node payments pay_state); pay_state B $tx3 mined"; then
    ok "3c D appeared later; B delivered; mined; result reached B"
  else fail "3c gateway later"; fi
  docker start ozm-A >/dev/null
  if wait_until 120 "A sees mined" bash -c "$(declare -f node payments pay_state); pay_state A $tx3 mined"; then
    ok "3d result reached A when A returned"
  else fail "3d A result"; fi

  # ---------------- Test 4 ----------------
  log "Test 4: tampered payments are refused with a reason and never reach D"
  local gw_before; gw_before=$(payments D | jq '.gateway | length')
  declare -A EXPECT=([amount]=11 [payee]=4 [sig]=5 [branch]="1 2" [expired]=7)
  for t in amount payee sig branch expired; do
    local txt; txt=$(node A tampered --to B --amount 0.01 --tamper "$t" | jq -r .txid)
    if wait_until 60 "B refuses $t" bash -c "$(declare -f node payments pay_state); pay_state A $txt 'refused, may still mine until expiry'"; then
      local c; c=$(payments B | jq -r --arg t "$txt" '[.refusals[] | select(.txid==$t)][0].reject | "\(.code) \(.reason)"')
      if [[ " ${EXPECT[$t]} " == *" ${c%% *} "* ]]; then ok "4 $t refused: code $c"; else fail "4 $t refused with unexpected code $c"; fi
    else fail "4 $t not refused"; fi
  done
  sleep 3
  local gw_after; gw_after=$(payments D | jq '.gateway | length')
  [[ "$gw_after" == "$gw_before" ]] && ok "4 nothing tampered reached the gateway ($gw_after submissions before and after)" || fail "4 gateway saw $((gw_after - gw_before)) tampered submissions"

  # ---------------- Test 5 + 7 ----------------
  log "Test 5 and 7: duplicates give one payment; resubmitting a mined tx gives the same result"
  local nb_before; nb_before=$(payments B | jq '.payments | length')
  local sub_before; sub_before=$(payments D | jq --arg t "$tx1" '.gateway[] | select(.txid==$t) | .submissions')
  node A resend --txid "$tx1" >/dev/null
  node A resend --txid "$tx1" >/dev/null
  if wait_until 60 "D saw resubmissions" bash -c "docker exec ozm-D ozm-node payments | jq -e --arg t $tx1 --argjson b $sub_before '.gateway[] | select(.txid==\$t) | select(.submissions >= \$b + 2)'"; then :; fi
  sleep 3
  local nb_after; nb_after=$(payments B | jq '.payments | length')
  local dups; dups=$(node B events | jq --arg t "$tx1" '[.[] | select(.event=="duplicate-pay" and .detail.txid==$t)] | length')
  [[ "$nb_after" == "$nb_before" && "$dups" -ge 2 ]] && ok "5 same txid in new envelopes: one payment ($nb_after), $dups duplicates answered" || fail "5 duplicates: payments $nb_before→$nb_after, dup events $dups"
  local g; g=$(payments D | jq -c --arg t "$tx1" '.gateway[] | select(.txid==$t) | {submissions, status: .result.body.status}')
  echo "  D: $g"
  if jq -e --argjson h "$height" '.submissions >= 3 and .status.Mined.height == $h' <<<"$g" >/dev/null; then
    ok "7 gateway idempotent: $(jq .submissions <<<"$g") submissions, all report mined at $height"
  else fail "7 gateway idempotence"; fi
  pay_state A "$tx1" mined >/dev/null && ok "7 A's payment stays mined (no error state)" || fail "7 A state changed"

  log "Summary"
  jq -s '{passed: map(select(.pass)) | length, failed: map(select(.pass|not)) | length}' "$RESULTS"
  for n in A R B D; do docker logs "ozm-$n" >"$STATE/$n.log" 2>&1; done
  return "$FAILED"
}

# Prints when each step of a payment happened, relative to the pay command.
timeline() {
  local tx=$1 t0=$2
  { for n in A R B D; do node "$n" events; done; } | jq -s --arg t "$tx" --argjson t0 "$t0" -r '
    add | map(select(.ms >= $t0)) | sort_by(.ms) |
    map(select((.detail.txid? == $t) or (.event == "mesh-recv") or (.event == "mesh-delivered"))) |
    .[] | "  +\(.ms - $t0 | tostring | (" " * (6 - length)) + .) ms  \(.node)  \(.event)  \(.detail | del(.txid, .envelope_id, .msg_id) | tostring)"' | sed -n '1,40p'
}

case "${1:-all}" in
  up) down; up ;;
  test) test_all ;;
  down) down ;;
  all) down; up; test_all ;;
  *) echo "usage: $0 [all|up|test|down]"; exit 2 ;;
esac
