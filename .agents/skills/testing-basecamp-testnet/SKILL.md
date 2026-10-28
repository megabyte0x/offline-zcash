---
name: testing-basecamp-testnet
description: Runs and records released Offline Zcash Basecamp packages on Linux with an isolated testnet gateway and mesh wallets. Use to reproduce multi-device sync, payment, refusal, restart and gateway behaviour with TAZ test coins.
---

# Testing Basecamp on public testnet

Run the real released Qt apps, keep each device's state separate, and correlate
recordings with wallet checkpoints and structured events. Do not substitute a
browser mock or Regtest for a requested public-testnet reproduction.

## Safety and scope

- Use **TAZ on public testnet only**, never mainnet ZEC. Ask the owner to fund a
  newly generated lab address; never request or import their seed/private keys.
- Use `bench/.state-testnet/` (gitignored) for packages, keys, invites and wallets.
  Keep it private with `umask 077`; never upload that directory. Invites contain
  spending keys. Preserve funded state across restarts; do not reset or delete it.
- Put reviewable recordings/screenshots and sanitized evidence in
  `.amp/in/artifacts/`. Scratch captures belong in `/tmp`.
- Use one `--user-dir` per device, each with the **same shared core module** and
  thin UI plugins. Never copy `module_data`, `secret.json`, invites or wallets
  from one device to another. Installing Gateway UI does not grant gateway role.
- These nodes share an orb's TCP network; this is not a physical-radio or
  multi-machine test. Wallets configured without RPC sync through the mesh, but
  this alone is not proof of an OS-level internet disconnection.
- Testnet `included` (1 confirmation) and `mined` (10) are gateway-reported,
  not independently proved or final. A payee refusal is **not a refund**: the
  original transaction can still mine. Inspect its txid before any replacement.

## Prepare released packages

The validated baseline is Offline Zcash **0.7.4**, Basecamp **0.3.2**, Linux x86_64.
Basecamp's tag is `0.3.2`, not `v0.3.2`. Its AppImage needs glibc 2.39;
the bundled Ubuntu 24.04 Dockerfile runs on a Debian 12 orb without changing
the host. Inspect available services and `docker info` first; use the repo's
Docker startup instructions if necessary. Do not start `ths` for this test.

From the repository root:

```bash
umask 077
mkdir -p bench/.state-testnet/packages .amp/in/artifacts
chmod 700 bench/.state-testnet
gh release download 0.3.2 --repo logos-co/logos-basecamp \
  --pattern 'LogosBasecamp-Desktop-v0.3.2-f2fae6-x86_64.AppImage' \
  --dir bench/.state-testnet/packages
gh release download v0.7.4 --repo megabyte0x/offline-zcash \
  --pattern '*-linux-amd64.lgx' --pattern '*-linux-amd64.lgx.sha256' \
  --dir bench/.state-testnet/packages
(cd bench/.state-testnet/packages && sha256sum -c ./*.lgx.sha256)
printf '%s  %s\n' \
  983de0c7e282155d93c7e338da78e5b1a7a32112390a89be48d44700c3ade012 \
  bench/.state-testnet/packages/LogosBasecamp-Desktop-v0.3.2-f2fae6-x86_64.AppImage \
  | sha256sum -c -
docker build -t ozm-basecamp-lab:0.3.2 \
  -f .agents/skills/testing-basecamp-testnet/scripts/Dockerfile \
  bench/.state-testnet/packages
```

If files already exist, verify and reuse them rather than overwriting them.
For another release, inspect its actual assets/digests and update the pinned
AppImage filename/digest and Dockerfile together. Never silently test a source
build instead of the requested shipped package.

## Start three native displays

Use supervised services, not detached shell processes. Check for an existing lab
before starting; resume it instead of creating conflicting names or displays.
The following are commands from the repository root; `$PWD` must be that root.

```bash
amp orb service start ozm-basecamp-lab --command \
  "docker run --rm --name ozm-basecamp-lab --network host --ipc=host \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v '$PWD/bench/.state-testnet:/lab' \
  ozm-basecamp-lab:0.3.2 \
  Xvfb :94 -screen 0 1600x1000x24 -ac -nolisten tcp"
docker exec -e DISPLAY=:94 ozm-basecamp-lab xdpyinfo >/dev/null
amp orb service start ozm-display-alice --command \
  'docker exec ozm-basecamp-lab Xvfb :95 -screen 0 1600x1000x24 -ac -nolisten tcp'
amp orb service start ozm-display-bob --command \
  'docker exec ozm-basecamp-lab Xvfb :96 -screen 0 1600x1000x24 -ac -nolisten tcp'
amp orb service start ozm-basecamp-gateway --command \
  'docker exec -e DISPLAY=:94 ozm-basecamp-lab /opt/squashfs-root/AppRun --user-dir /lab/gateway'
```

Shared X socket **and** `--ipc=host` are needed for host ffmpeg's MIT-SHM
capture. Xvfb permits local clients in this disposable orb; it exposes no X TCP
listener. Check `xdpyinfo` before launching an app on its display. If stopping
the lab, stop its app services, then Alice/Bob display services, then the
container service. Wallet data survives in the bind-mounted state directory.

Run the Docker Regtest benches separately from host-network Basecamp nodes.
Host UDP discovery can cross Docker bridges and add unexpected neighbours to
an otherwise isolated bench. Stop the native apps (not their funded state)
before those benches, then restart them for the public-testnet run.

**Individual restart pitfall:** stopping a service attached with `docker exec`
can stop just the Docker client, leaving Basecamp and its module running inside
the container. Before claiming a restart, inspect `docker top ozm-basecamp-lab
-eo pid,ppid,args`. After stopping that device's service, confirm its exact
`LogosBasecamp --user-dir /lab/DEVICE` PID with `ps -o pid,args -p PID`, then
terminate that PID (`sudo kill -TERM PID` for a root-owned container process).
Verify its module child has exited before starting the service again. Do not
use `pkill -f`, target another device, or confuse raising an existing window with
restarting its core. Stopping the whole lab container stops every device.

After restarting Basecamp, open an Offline Zcash app and verify its `logos_host`
child and mesh listener exist. The Basecamp welcome screen alone does not load
the engine; leaving the gateway there is still an outage, not a failed reconnect.

## Install and enroll through the UI

1. In Gateway's Basecamp **Manage Packages**, install the four downloaded LGXs:
   module, wallet UI, Organiser, Gateway. Inspect a screenshot after each dialog;
   file chooser coordinates change after the detail pane opens. In the validated
   build Ctrl+L did not navigate the native file chooser; double-click folders.
2. Open **Organiser**. Select testnet, make Operator the gateway, and choose a
   recent birthday **before funding**. Record the chosen height. A fresh wallet
   can use the current tip, but restoring an older wallet requires its original
   birthday. Create session/invites, then join this device as Operator. Never
   capture or publish the invitation JSON or keys.
3. Open **Gateway**, check the TLS chain service, then start it. Record network,
   package version, tip, source count/warning, and public funding address.
4. Install only the shared core module and wallet UI into Alice/Bob. For the
   same pinned version, the already-installed **code only** can be copied:

   ```bash
   docker exec ozm-basecamp-lab sh -c '
     for device in alice bob; do
       mkdir -p /lab/$device/modules /lab/$device/plugins
       cp -a /lab/gateway/modules/zcash_offline_module /lab/$device/modules/
       cp -a /lab/gateway/plugins/zcash_offline_ui /lab/$device/plugins/
     done'
   amp orb service start ozm-basecamp-alice --command \
     'docker exec -e DISPLAY=:95 ozm-basecamp-lab /opt/squashfs-root/AppRun --user-dir /lab/alice'
   amp orb service start ozm-basecamp-bob --command \
     'docker exec -e DISPLAY=:96 ozm-basecamp-lab /opt/squashfs-root/AppRun --user-dir /lab/bob'
   ```

5. Start each wallet without an invite on testnet. Under Connection options,
   dial `127.0.0.1:7777`, listen Alice `127.0.0.1:7797`, Bob
   `127.0.0.1:7798`. Give Alice a recent birthday; for the lagging-recipient
   case give Bob an older birthday (0 resolves to NU7 at 4,465,026). Observe
   actual checkpoints, not just the presence badge. If the first automatic
   request makes no progress, capture its events, then try **Sync now**.
6. Verify each visible header says **v0.7.4/testnet** and the installed module
   metadata agrees. Old Basecamp service processes can retain old code: stop
   the device's app before replacing packages, then restart and recheck.

For a 0.7.5 upgrade, preserve the baseline wallets, payments and mesh stores.
New wallets choose their scan start automatically and have no birthday field;
keep the baseline's lagging recipient for the catch-up comparison. Simple mode
shows funds, Send, Receive/Copy and history; Debug exposes technical controls.
Record both the LGX digest and the installed engine digest for source candidates.

For the enrolled-fleet control, use a **new** separate user directory and its
own invite, never overwrite an existing wallet. Open-mode peers must not issue
pending acceptance; invited peers have the documented pending limits.

## Capture interactions and state

Use `xdotool` inside the container with the correct `DISPLAY`. Capture, inspect,
then act; do not blindly replay coordinates from another screen or version.
Move the pointer through relevant controls so the recording shows the action.
Run `xdotool type ... TEXT` separately from later mouse/key commands: `type`
consumes trailing arguments as text. Header status changes can move Debug;
locate its current accessibility bounds just before clicking and assert that
the label changes to Hide debug (and back). Each display has its own
accessibility bus; set `DISPLAY` on Python/pyatspi calls too.
Qt's accessibility tree exposes text but may not implement EditableText. Click
the named field, use Ctrl+A and type with xdotool, then read the text back before
Send. Abort on a failed input command; do not turn setup footage into send proof.

First start the timed recorder with `shell_command` and `timeout_ms: 1000`:

```bash
ffmpeg -loglevel error -y -f x11grab -framerate 12 -video_size 1600x900 \
  -draw_mouse 1 -i :96 -t 45 -c:v libx264 -preset veryfast -crf 24 \
  -pix_fmt yuv420p .amp/in/artifacts/bob-interaction.mp4
```

In a separate tool call, perform the action and collect state:

```bash
docker exec -e DISPLAY=:96 ozm-basecamp-lab xdotool mousemove 310 435 click 1
ffmpeg -loglevel error -y -f x11grab -video_size 1600x900 \
  -i :96 -frames:v 1 .amp/in/artifacts/bob.png
docker exec -i ozm-basecamp-lab python3 - /lab \
  < .agents/skills/testing-basecamp-testnet/scripts/snapshot.py \
  > .amp/in/artifacts/testnet-snapshot.json
```

Follow the recorder's returned PID to completion. Inspect the media with
`view_media`; publishing a path alone is not verification. Embed the inspected final footage.
Use short, labeled clips; do not present startup footage as a successful payment.

`snapshot.py` whitelists checkpoint, package and transaction/event metadata,
including installed engine and signed-transaction SHA256 digests. Compare the
engine digest with the tested LGX; two builds can both report version 0.7.5. It
does not export raw wallet/secret/invite files, encrypted transactions, notes,
viewing keys or mesh caches. Snapshots read live files independently, not as one
atomic database transaction; take before/after snapshots at named milestones.
It exports aggregate spendable/locked zatoshis, signed receipt/result bodies
without signatures, the latest 40 relevant events and 200 transaction events
per device. Busy catch-up traffic must not hide the earlier refusal. Inspect the original
private `events.jsonl` locally when an older event is needed, then export only
the required sanitized fields. Correlate the same txid and `ms` across devices.

If a wallet loses gateway presence after relaying a long catch-up, preserve
the cache and count packet headers locally: `mesh-store.json` stores hex packets
under `store.*.packet`; byte 1 is kind (1 message, 2 broadcast, 3 ack), bytes
20–27 sender and 28–35 recipient. A relay holding 512 non-ack packets from one
sender can reject that sender's fresh packets, including packets for itself,
in `ozm-mesh::Node::handle_packet` before application delivery. Record this
separately from a payee verification failure. Before any payments are in flight,
with all affected apps actually stopped, a disposable lab can preserve each
`mesh-store.json` under a timestamped backup name and restart with fresh
transport caches. Never move keys, wallet, payments or trust pins; do not hide
this recovery step or describe it as a software fix.

## Funded scenario and controls

If the lab is unfunded, finish setup and ask the owner for about **0.05 TAZ** at
the newly generated gateway address. Read the address from the running UI and
cross-check the gateway's **public** fleet entry. Do not reuse an address written
in this skill. Never ask for a private key. Preserve the lab while waiting.

After funding, sync the gateway and confirm the note is spendable before sending:

- **Control:** gateway wallet → caught-up Alice, e.g. 0.001 TAZ. Record txid,
  exact amount/fee, receipt and gateway statuses, sender lock and recipient
  balance through 1 and 10 confirmations. Sync does not imply mining. A single
  funding note is locked in full until the payment mines and change is scanned;
  do not fabricate another input or bypass the lock to run the next case.
- **Reported case:** gateway wallet → Bob while Bob is several 200-block batches
  behind. Start capture before paying. Record both tips at send time and the
  ordering of `anchor-unknown-syncing`, `synced`, `refused` and gateway results.
  If Bob has caught up, create a fresh unfunded recipient with an older birthday;
  do not rewind or edit a funded wallet to manufacture lag.
- **Recovery:** let the same recipient catch up. Follow the **original txid** to
  inclusion/mined/expiry, and check balances even if its payee receipt says
  Refused. Verify its transaction digest stays identical across the upgrade;
  do not edit the payment book or send a replacement to test recovery.
- **Gateway self-submission:** stop the payee and send from the online gateway
  wallet. Confirm the gateway submits without another device forwarding it.
- **Coalesced-tip control:** send just after the payer scans a new block while
  the recipient's displayed tip still matches its last gateway advertisement.
  A refresh request is not a response; record the fresh response before refusal.
- **Offline and restart controls:** after splitting mined notes with Prepare for
  offline, wait for mined and sync before disconnecting. Exercise wallet → wallet
  → gateway, gateway outage/reconnect and app restart, preserving the same
  state directories. Never spend unmined change or silently regenerate wallets.

The funded 0.7.4 run on 28 October reproduced an automatic code-6 refusal after
one 200-block batch: receiver 4,539,425, gateway about 4,551,178. A separate
caught-up control mined. `App::on_pay` waited for **any** tip advance, not full
catch-up. The online gateway also omitted self-submission for ordinary sends;
a payee/forwarder could mask that defect. A stored anchor-refusal receipt
prevented subsequent payee-envelope retries. A real-proof/synthetic-chain
regression separately checks heights 1 → 201 → 401, restart, reversed txid/sequence
ordering and refusal of an unknown root after a fresh caught-up response.

Check balance and history separately. A transaction can be scanned without a
fresh mesh envelope: the original recovered 130,000-zat payment did this while
Bob was stopped. Basecamp's chain-scan history must show its amount without
inventing a sender, receipt, timestamp or ten-confirmation result. Verify no
duplicate appears when the mesh envelope arrives, and that self-change/split
outputs are not presented as external credits.

Do not weaken anchor/proof/signature checks to avoid the refusal. Keep released
before-runs separate from any patched build, record the actual build identity,
and rerun identical scenarios if implementing a fix. Report what ran, which
observations were controlled versus live, remaining funding/device gates, and
whether changes are local, committed, pushed or released. Do not claim a funded
payment, a fix, or independent chain verification from startup footage.
