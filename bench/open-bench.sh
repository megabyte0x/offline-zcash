#!/usr/bin/env bash
# Six locally generated identities, no invites or dials; only the gateway can reach ths.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/bench/.state-open"
IMG=ozm-node:bench
DEVICES=(G A B C D E F)
node() { local n=$1; shift; docker exec "open-$n" ozm-node "$@"; }
wait_for() {
  local end=$((SECONDS + $1)); shift
  until "$@" >/dev/null 2>&1; do
    ((SECONDS < end)) || { echo "FAIL timeout: $*"; return 1; }
    sleep 1
  done
}
synced() { node "$1" status | jq -e '.open_network and .wallet.tip > 0 and .selected_gateway.hops == 1'; }
funded() { node "$1" status | jq -e '.wallet.spendable >= 30000000'; }
mined() { node "$1" payments | jq -e --arg t "$2" '.payments[] | select(.txid == $t and .state == "mined")'; }
receipted() { node "$1" payments | jq -e --arg t "$2" '.payments[] | select(.txid == $t and .receipt.body.state == "Included")'; }
empty_wallet() { node "$1" status | jq -e '.wallet.spendable == 0 and .wallet.locked == 0'; }
limited() { node "$1" events | jq -e '.[] | select(.event == "funds" and .detail.ok == false and (.detail.detail | contains("rate limit")))'; }
address() { node "$1" status | jq -r .address; }
chain_mined() {
  local result
  result=$(curl -fsS -H 'content-type: application/json' --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getrawtransaction\",\"params\":[\"$1\",1]}" http://127.0.0.1:18232)
  jq -e '.result.height > 0' <<<"$result" >/dev/null
  echo "PASS chain mined $1 at $(jq .result.height <<<"$result")"
}
down() {
  for n in "${DEVICES[@]}"; do docker rm -f "open-$n" >/dev/null 2>&1 || true; done
  docker network rm ozm-open >/dev/null 2>&1 || true
  if [[ -d "$STATE" ]]; then sudo -n rm -rf "$STATE"; fi
}
up() {
  cargo build --manifest-path "$ROOT/Cargo.toml" --release -p ozm-node -j 2
  mkdir -p "$STATE/ctx"
  cp "$ROOT/target/release/ozm-node" "$STATE/ctx/"
  docker build -q -t "$IMG" -f "$ROOT/bench/Dockerfile" "$STATE/ctx" >/dev/null
  "$ROOT/target/release/ozm-node" fleet-init --out "$STATE/operator" --name open-bench --network regtest --device G:gateway >/dev/null
  local addr; addr=$(jq -r '.devices[0].address' "$STATE/operator/fleet.json")
  ths faucet "$addr" --amount 4 --json >/dev/null
  docker network create --internal ozm-open >/dev/null
  # Gateways also share ths-default: keep this discovery channel separate from other benches.
  docker run -d --name open-G --network ozm-open -v "$STATE/operator:/operator:ro" --entrypoint sh "$IMG" -c \
    'ozm-node join /operator/G.invite.json --home /home/ozm && exec ozm-node run --data /home/ozm/data --fleet /home/ozm/fleet.json --secret /home/ozm/secret.json --discover --discovery-port 7788 --rpc http://zakura:18232 --mine' >/dev/null
  docker network connect ths-default open-G
  for n in A B C D E F; do
    docker run -d --name "open-$n" --hostname "$n" --network ozm-open -p "127.0.0.1:$((7810 + $(printf '%d' "'$n") - 65)):7780" \
      "$IMG" run --network regtest --data /home/ozm/data --fleet /home/ozm/fleet.json --secret /home/ozm/secret.json --discover --discovery-port 7788 --api 0.0.0.0:7780 >/dev/null
  done
}
test_all() {
  for n in A B C D E F; do
    wait_for 120 synced "$n"
    if docker exec "open-$n" curl -s -m 2 http://zakura:18232 >/dev/null 2>&1; then echo "FAIL $n reached chain directly"; return 1; fi
    node "$n" status | jq '{name,peer_id,open_network,selected_gateway,wallet}' >"$STATE/$n-synced.json"
    echo "PASS $n found gateway and synced without invite or dial"
    node "$n" fund --notes 3 --amount 0.1 >/dev/null
    if [[ "$n" == A ]]; then
      node A fund --notes 3 --amount 0.1 >/dev/null
      wait_for 30 limited A
      echo 'PASS faucet rate limit rejects repeat device request'
    fi
    wait_for 240 funded "$n"
  done
  local from to tx
  for edge in A:B B:C C:D D:E E:F F:A; do
    from=${edge%:*}; to=${edge#*:}
    tx=$(node "$from" pay --to "$(address "$to")" --amount 0.0137 | jq -er .txid)
    wait_for 120 mined "$from" "$tx"
    wait_for 120 mined "$to" "$tx"
    chain_mined "$tx"
    wait_for 60 receipted "$from" "$tx"
    node "$to" events | jq -e 'all(.[]; .event != "accepted")' >/dev/null
    echo "PASS $from → $to by address, included receipt and mined on both; no pending acceptance"
  done
  # A locally generated destination never starts a mesh, so no binding can be discovered.
  "$ROOT/target/release/ozm-node" fleet-init --out "$STATE/absent" --name absent --network regtest --device Absent:gateway >/dev/null
  local absent; absent=$(jq -r '.devices[0].address' "$STATE/absent/fleet.json")
  node A sync >/dev/null; sleep 5
  tx=$(node A pay --to "$absent" --amount 0.0073 | jq -er .txid)
  wait_for 120 mined A "$tx"
  chain_mined "$tx"
  node A payments | jq -e --arg t "$tx" '.payments[] | select(.txid == $t and .receipt == null and .receipt_peer == null and any(.history[]; .event | contains("sent over mesh")))' >/dev/null
  echo 'PASS unknown address mined without payee receipt'
  # F has three or more spendable inputs: catches a guessed two-action fee.
  node F sync >/dev/null; sleep 8
  local quote amount fee before
  quote=$(node F max); amount=$(jq -r .amount <<<"$quote"); fee=$(jq -r .fee_zat <<<"$quote")
  before=$(node F status | jq .wallet.spendable)
  ((fee > 10000)) || { echo "FAIL Max did not exercise >2 actions: $quote"; return 1; }
  tx=$(node F pay --to "$(address B)" --amount "$amount" | jq -er .txid)
  wait_for 120 mined F "$tx"; wait_for 120 mined B "$tx"
  chain_mined "$tx"
  node F sync >/dev/null
  wait_for 120 empty_wallet F
  node F payments | jq -e --arg t "$tx" --argjson fee "$fee" --argjson before "$before" '.payments[] | select(.txid == $t and .fee == $fee and .amount + .fee == $before)' >/dev/null
  echo "PASS Max mined, exact builder fee $fee zat, spendable and locked both zero"
  for n in "${DEVICES[@]}"; do docker logs "open-$n" >"$STATE/$n.log" 2>&1; node "$n" payments >"$STATE/$n-payments.json"; done
  echo 'PASS open mesh: six identities, six ring payments, unknown destination and Max'
}
case "${1:-all}" in
  all) down; up; test_all ;;
  up) down; up ;;
  test) test_all ;;
  down) down ;;
  *) echo "usage: $0 [all|up|test|down]"; exit 2 ;;
esac
