#!/usr/bin/env node
// Run against a local ozm-node: node bench/ui-security-check.mjs http://127.0.0.1:7780
import { execFileSync } from 'node:child_process';

const session = `ozm-xss-${process.pid}`;
const browser = (...args) => execFileSync('agent-browser', ['--session', session, ...args], { encoding: 'utf8' });
try {
  browser('open', process.argv[2] || 'http://127.0.0.1:7780');
  const result = browser('eval', `(async () => {
    const status = await api('/api/status');
    const attack = '<img src=x onerror="globalThis.ozmXss=true">';
    status.name = status.role = status.version = status.address = attack;
    status.neighbor_links = [{ name: attack, kind: 'tcp' }];
    status.selected_gateway = { name: attack, peer_id: attack, hops: 1, tip: 42 };
    status.gateway_warning = attack;
    const payments = { payments: [{ created_ms: 1, direction: attack, counterparty: attack,
      amount: 123, state: attack, txid: attack, history: [{ ms: 1, event: attack }] }],
      refusals: [{ from: attack, reject: { code: 5, reason: attack } }] };
    globalThis.fetch = async path => new Response(JSON.stringify(path === '/api/status' ? status : payments),
      { status: 200, headers: { 'content-type': 'application/json' } });
    await refresh();
    await new Promise(resolve => setTimeout(resolve, 100));
    for (const id of ['who', 'ver', 'addr', 'nbrs', 'gateway', 'pays', 'refs']) {
      if (!document.getElementById(id).textContent.includes(attack)) throw new Error(id + ' lost remote text');
    }
    if (globalThis.ozmXss || document.querySelector('main img, main script, header img')) throw new Error('remote HTML executed');
    return 'PASS remote names, addresses, states, txids, history and refusal text render literally; no HTML executes';
  })()`);
  process.stdout.write(result);
} finally {
  browser('close');
}
