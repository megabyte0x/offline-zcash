use ozm_node::{fleet_init, join_invite, start, FleetInitOptions, RunOpts};

#[tokio::test]
async fn automatic_preparation_keeps_change_unspendable_and_does_not_loop_after_failure() {
    use std::time::Duration;
    use zcash_offline_core::{fleet::generate_fleet, wallet::{StoredNote, WalletState, BlockData, ChunkSource}, Policy};
    for (value, witness, expected) in [(749_999, true, 0), (750_000, false, 0), (750_000, true, 1), (8_000_000, true, 1)] {
        let dir = tempfile::tempdir().unwrap();
        let (fleet, secrets) = generate_fleet("auto", &[("G".into(), "gateway".into())], Policy::default()).unwrap();
        let me = secrets[0].clone();
        let mut note = StoredNote { position: 0, height: 1, txid: "external-funding".into(), value,
            recipient: hex::encode(me.address().unwrap().to_raw_address_bytes()), rho: hex::encode([0; 32]),
            rseed: hex::encode([7; 32]), nullifier: String::new(), spent_in: None, locked_by: None };
        let owned = note.note().unwrap();
        note.nullifier = hex::encode(owned.nullifier(&me.fvk().unwrap()).to_bytes());
        let cm = orchard::note::ExtractedNoteCommitment::from(owned.commitment());
        let mut wallet = WalletState { birthday: 1, tip_height: 1, tip_hash: "block1".into(),
            commitments: if witness { vec![hex::encode(cm.to_bytes())] } else { vec![] }, notes: vec![note], ..Default::default() };
        if witness { wallet.roots.insert(1, hex::encode(wallet.witness(0).unwrap().1)); }
        wallet.save(&dir.path().join("wallet.json")).unwrap();
        let (mesh, _events) = ozm_mesh::Mesh::start(ozm_mesh::MeshConfig {
            peer_id: me.public().unwrap().peer_id.parse().unwrap(), ack_signer: Some(me.signing_key().unwrap()), ..Default::default()
        }).await.unwrap();
        let cfg = || ozm_node::app::Config { data: dir.path().into(), fleet: fleet.clone(), me: me.clone(), rpc: Some("http://127.0.0.1:0".into()),
            mine: false, retry: Duration::from_secs(30), faucet_max_zat: 0 };
        let app = ozm_node::App::new(cfg(), mesh).unwrap();
        let result = app.auto_prepare_tick().await;
        if !witness { assert!(result.is_err()); }
        let path = dir.path().join("payments.json");
        let book: ozm_node::app::Book = if path.exists() { serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap() }
            else { assert_eq!(value, 749_999); Default::default() };
        let payments: Vec<_> = book.payments.into_values().collect();
        assert_eq!(payments.len(), expected);
        if let Some(payment) = payments.first() {
            assert_eq!(payment.direction, "prepare");
            assert!(payment.fee <= value / 50);
            assert_eq!(WalletState::load(&dir.path().join("wallet.json")).unwrap().balance(), (0, value));
            // The canonical scan, not a gateway notice, makes the split and its remainder spendable.
            let mut scanned = WalletState::load(&dir.path().join("wallet.json")).unwrap();
            scanned.sync_from(&ChunkSource { prev: (1, "block1".into()), blocks: vec![BlockData {
                height: 2, hash: "block2".into(), txs: vec![hex::decode(&payment.tx_hex).unwrap()],
            }] }, &me.fvk().unwrap()).unwrap();
            assert_eq!(scanned.balance(), (value - payment.fee, 0));
            assert_eq!(scanned.notes.iter().filter(|n| n.spendable()).count(), if value == 750_000 { 3 } else { 9 });
            assert!(scanned.notes.iter().any(|n| n.spendable() && n.value >= (value - payment.fee) / 2));
            app.state.lock().unwrap().0 = scanned;
        }
        app.auto_prepare_tick().await.unwrap();
        assert_eq!(app.status()["payments"], expected);
        // Group external outputs, retain spent credits, and exclude our change/splits and unmined notes.
        {
            let mut state = app.state.lock().unwrap();
            let note = state.0.notes[0].clone();
            state.0.notes.push(StoredNote { value: 3_007, ..note.clone() });
            state.0.pending_incoming.push(StoredNote { txid: "not-scanned".into(), value: 90_013, ..note });
        }
        let credits = app.status()["chain_received"].as_array().unwrap().clone();
        assert_eq!(credits.len(), 1);
        assert_eq!(credits[0]["txid"], "external-funding");
        assert_eq!(credits[0]["amount"], value + 3_007);
        assert_eq!(credits[0]["state"], "scanned");
        app.stop().await;
        // The failed witness does not become a retry loop after restarting the wallet either.
        let (mesh, _events) = ozm_mesh::Mesh::start(Default::default()).await.unwrap();
        let restarted = ozm_node::App::new(cfg(), mesh).unwrap();
        restarted.auto_prepare_tick().await.unwrap();
        assert_eq!(restarted.status()["payments"], expected);
        restarted.stop().await;
    }
}

#[tokio::test]
async fn payment_during_sync_download_keeps_its_reservation_after_install() {
    use std::{sync::Arc, time::Duration};
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use tokio::sync::{watch, Notify};
    use zcash_offline_core::{fleet::generate_fleet, wallet::{StoredNote, WalletState}, Policy};

    let dir = tempfile::tempdir().unwrap();
    let (fleet, secrets) = generate_fleet("concurrent", &[("A".into(), "gateway".into()), ("B".into(), "".into())], Policy::default()).unwrap();
    let me = secrets[0].clone();
    let mut note = StoredNote { position: 0, height: 1, txid: "funding".into(), value: 1_000_000,
        recipient: hex::encode(me.address().unwrap().to_raw_address_bytes()), rho: hex::encode([0; 32]),
        rseed: hex::encode([7; 32]), nullifier: String::new(), spent_in: None, locked_by: None };
    let owned = note.note().unwrap();
    note.nullifier = hex::encode(owned.nullifier(&me.fvk().unwrap()).to_bytes());
    let cm = orchard::note::ExtractedNoteCommitment::from(owned.commitment());
    let mut wallet = WalletState { birthday: 1, tip_height: 1, tip_hash: "block1".into(),
        commitments: vec![hex::encode(cm.to_bytes())], notes: vec![note], ..Default::default() };
    let (_, root) = wallet.witness(0).unwrap();
    wallet.roots.insert(1, hex::encode(root));
    wallet.hashes.insert(1, "block1".into());
    wallet.save(&dir.path().join("wallet.json")).unwrap();

    let downloading = Arc::new(Notify::new());
    let (release, released) = watch::channel(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let entered = downloading.clone();
    let rpc = Router::new().route("/", post(move |Json(request): Json<Value>| {
        let entered = entered.clone();
        let mut released = released.clone();
        async move {
            let result = match request["method"].as_str().unwrap() {
                "getblockcount" => json!(2),
                "getblockhash" => json!(format!("block{}", request["params"][0])),
                "sendrawtransaction" => json!("accepted"),
                "getrawtransaction" => json!({}),
                "getblock" => {
                    entered.notify_one();
                    while !*released.borrow_and_update() { released.changed().await.unwrap(); }
                    json!({"hash": "block2", "tx": []})
                }
                method => panic!("unexpected RPC {method}"),
            };
            Json(json!({"result": result, "error": null, "id": 1}))
        }
    }));
    let server = tokio::spawn(async move { axum::serve(listener, rpc).await.unwrap() });
    let (mesh, _events) = ozm_mesh::Mesh::start(ozm_mesh::MeshConfig {
        peer_id: me.public().unwrap().peer_id.parse().unwrap(), ack_signer: Some(me.signing_key().unwrap()),
        ..Default::default()
    }).await.unwrap();
    let app = ozm_node::App::new(ozm_node::app::Config { data: dir.path().into(), fleet, me, rpc: Some(url.clone()),
        mine: false, retry: Duration::from_secs(30), faucet_max_zat: 0 }, mesh).unwrap();
    app.start_sync();
    tokio::time::timeout(Duration::from_secs(5), downloading.notified()).await.unwrap();
    assert_eq!(app.status()["syncing"], true);
    // A fallback request must not overlap the same recipient's unfinished response.
    let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
    let sender = &secrets[1];
    let inner = zcash_offline_core::envelope::authenticate(zcash_offline_core::envelope::Inner {
        from: sender.name.clone(), from_key: sender.public().unwrap().ed25519,
        body: zcash_offline_core::messages::Body::CompactReq {
            have_height: 1, have_hash: "block1".into(), checkpoints: vec![], birthday: 1,
        },
    }, sender, &recipient).unwrap();
    let request = ozm_mesh::MeshEvent::Message { from: sender.public().unwrap().peer_id.parse().unwrap(),
        msg_id: ozm_mesh::MsgId::random(), payload: zcash_offline_core::envelope::seal(&inner, &recipient).unwrap(),
        hops: 1, broadcast: false };
    let serving = tokio::spawn({ let app = app.clone(); let request = request.clone(); async move { app.handle_mesh_event(request).await } });
    tokio::time::timeout(Duration::from_secs(5), downloading.notified()).await.unwrap();
    let mut duplicate = tokio::spawn({ let app = app.clone(); async move { app.handle_mesh_event(request).await } });
    let coalesced = tokio::time::timeout(Duration::from_secs(5), &mut duplicate).await.is_ok();
    app.configure_gateway("http://127.0.0.1:0".into(), false).unwrap();
    let absent = zcash_offline_core::DeviceSecret::generate("Absent", "").public().unwrap().address;
    let paid = tokio::time::timeout(Duration::from_secs(10), app.pay(&absent, 20_003)).await;
    // Let the download finish even when the pre-fix lock causes the payment to time out.
    release.send(true).unwrap();
    serving.await.unwrap();
    if !coalesced { duplicate.await.unwrap(); }
    assert!(coalesced, "duplicate sync request started another unfinished response");
    let served = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap().lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()).filter(|e| e["event"] == "sync-served").count();
    assert_eq!(served, 1);
    let payment = paid.expect("payment waited for the sync download").unwrap();
    // A transport failure after signing returns the saved payment, not a false "not sent" error.
    assert_eq!(payment.state, "waiting-gateway");
    assert!(payment.result.is_none());
    tokio::time::timeout(Duration::from_secs(5), async {
        while app.status()["wallet"]["tip"] != 2 { tokio::time::sleep(Duration::from_millis(10)).await; }
    }).await.unwrap();
    let durable = WalletState::load(&dir.path().join("wallet.json")).unwrap();
    assert_eq!(durable.balance(), (0, 1_000_000));
    assert_eq!(durable.notes[0].locked_by.as_deref(), Some(payment.txid.as_str()));
    assert_eq!(durable.reservations[&durable.notes[0].nullifier].0, payment.txid);
    // Upgrade recovery: retry the held original bytes even after an old payee refusal.
    {
        let mut st = app.state.lock().unwrap();
        st.1.gateway.clear();
        let p = st.1.payments.get_mut(&payment.txid).unwrap();
        p.state = "refused, may still mine until expiry".into();
        p.result = None;
        p.last_send_ms = 0;
    }
    app.configure_gateway(url, false).unwrap();
    app.retry_tick().await;
    let recovered = app.payment(&payment.txid).unwrap();
    assert_eq!(recovered.tx_hex, payment.tx_hex);
    // The gateway wallet submits directly, without any payee online to forward it.
    assert_eq!(recovered.state, "in-mempool");
    assert!(matches!(recovered.result.as_ref().unwrap().body.status, zcash_offline_core::messages::TxStatus::Mempool));
    app.stop().await;
    server.abort();
}

#[tokio::test]
async fn incoming_payment_waits_for_catchup_and_survives_restart_without_sender() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{builder::build_payment, envelope::{self, Inner}, messages::{now_ms, Body, Presence, Signed, Statement},
        sync::{Batch, CompactBlock}, wallet::{StoredNote, WalletState}, DeviceSecret, Policy};
    async fn deliver(app: &std::sync::Arc<ozm_node::App>, sender: &DeviceSecret, body: Body) {
        let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
        let inner = envelope::authenticate(Inner { from: sender.name.clone(),
            from_key: sender.public().unwrap().ed25519, body }, sender, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: sender.public().unwrap().peer_id.parse().unwrap(),
            msg_id: MsgId::random(), payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    }
    let dir = tempfile::tempdir().unwrap();
    let g = DeviceSecret::generate("G", "gateway");
    let home = dir.path().join("B");
    ozm_node::join_open_on(&home, zcash_offline_core::fleet::Network::Regtest, 1).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    let app = start(&opts).await.unwrap();
    let invite = serde_json::to_string(&ozm_node::Invite { fleet: app.cfg.fleet.clone(), secret: app.cfg.me.clone() }).unwrap();
    // An open wallet learns the sender from presence, not a permanent fleet entry.
    deliver(&app, &g, Body::Presence(Signed::sign(Presence {
        device: g.public().unwrap(), gateway: true, tip: 1, created_ms: now_ms(),
        address_proof: zcash_offline_core::binding::prove(&g).unwrap(),
    }, &g).unwrap())).await;
    let note = StoredNote { position: 0, height: 1, txid: "controlled funding".into(), value: 1_000_000,
        recipient: hex::encode(g.address().unwrap().to_raw_address_bytes()), rho: hex::encode([0;32]),
        rseed: hex::encode([7;32]), nullifier: String::new(), spent_in: None, locked_by: None };
    let cm = orchard::note::ExtractedNoteCommitment::from(note.note().unwrap().commitment());
    let mut early = WalletState { birthday: 1, tip_height: 1, tip_hash: "block1".into(),
        commitments: vec![hex::encode(cm.to_bytes())], ..Default::default() };
    early.roots.insert(1, hex::encode(early.witness(0).unwrap().1));
    app.state.lock().unwrap().0 = early.clone();
    let mut payer = early.clone();
    payer.tip_height = 401; payer.tip_hash = "block401".into();
    let other_note = StoredNote { position: 1, rseed: hex::encode([8;32]), ..note.clone() };
    payer.commitments.push(hex::encode(other_note.commitment().unwrap()));
    payer.notes.extend([note, other_note]);
    payer.roots.insert(401, hex::encode(payer.witness(0).unwrap().1));
    let mut caught_up = payer.clone(); caught_up.notes.clear();
    let mut payments = [20_003, 20_007].map(|amount|
        build_payment(&mut payer, &g, app.cfg.me.address().unwrap(), amount, &Policy::default()).unwrap());
    // Deliberately oppose txid order: retrying the later sequence first would refuse a valid payment.
    payments.sort_by(|a, b| b.txid.cmp(&a.txid));
    let [p, second] = payments;
    let statement = Signed::sign(Statement { txid: p.txid.clone(), auth_digest: p.auth_digest,
        amount: p.amount, fee: p.fee, expiry_height: p.expiry_height, payer: "G".into(), payee: app.name().into(),
        seq: 1, created_ms: 1 }, &g).unwrap();
    let second_statement = Signed::sign(Statement { txid: second.txid.clone(), auth_digest: second.auth_digest,
        amount: second.amount, fee: second.fee, expiry_height: second.expiry_height, payer: "G".into(), payee: app.name().into(),
        seq: 2, created_ms: 2 }, &g).unwrap();
    // Coalesced announcements can still say "caught up" when the payer used a newer root.
    deliver(&app, &g, Body::Tip { height: 1, hash: "block1".into() }).await;
    assert_eq!(app.status()["chain_tip"], 1);
    deliver(&app, &g, Body::Pay { tx: p.raw.clone(), statement: statement.clone() }).await;
    deliver(&app, &g, Body::Pay { tx: second.raw, statement: second_statement }).await;
    assert_eq!(app.status()["pending_checks"], 2);
    let first = Batch { prev_height: 1, prev_hash: "block1".into(), seed: None,
        blocks: vec![CompactBlock { height: 201, hash: "block201".into(), txs: vec![] }],
        chain_tip: 401, sources: vec!["controlled source".into()] };
    for part in first.chunks().unwrap() { deliver(&app, &g, Body::CompactPart(part)).await; }
    app.retry_tick().await;
    assert_eq!(app.status()["pending_checks"], 2);
    assert_eq!(app.status()["wallet"]["tip"], 201);
    assert!(app.state.lock().unwrap().1.refusals.is_empty());
    assert!(app.payment(&p.txid).is_none());
    app.stop().await;
    let app = start(&opts).await.unwrap();
    app.retry_tick().await;
    assert_eq!(app.status()["pending_checks"], 2);
    app.state.lock().unwrap().0 = caught_up;
    // Restored statements still require a valid signature bound to the original mesh sender.
    {
        let mut st = app.state.lock().unwrap();
        st.1.deferred_payments.get_mut(&p.txid).unwrap().statement.sig = hex::encode([0; 64]);
        st.1.deferred_payments.get_mut(&second.txid).unwrap().from = DeviceSecret::generate("Impostor", "").public().unwrap().peer_id;
    }
    app.retry_tick().await;
    assert!(app.payment(&p.txid).is_none());
    assert!(app.payment(&second.txid).is_none());
    {
        let mut st = app.state.lock().unwrap();
        st.1.deferred_payments.get_mut(&p.txid).unwrap().statement = statement.clone();
        st.1.deferred_payments.get_mut(&second.txid).unwrap().from = g.public().unwrap().peer_id;
    }
    // A failed retry waits for the next chain response instead of repeating every tick.
    app.retry_tick().await;
    assert!(app.payment(&p.txid).is_none());
    for d in app.state.lock().unwrap().1.deferred_payments.values_mut() { d.retry_generation = 0; }
    app.retry_tick().await; // No second Pay message and no sender needed.
    assert_eq!(app.status()["pending_checks"], 0);
    assert!(app.state.lock().unwrap().1.refusals.is_empty());
    assert_eq!(app.payment(&p.txid).unwrap().amount, p.amount);
    assert_eq!(app.payment(&p.txid).unwrap().tx_hex, hex::encode(&p.raw));
    assert_eq!(app.payment(&p.txid).unwrap().counterparty, g.public().unwrap().peer_id);
    assert_eq!(app.payment(&p.txid).unwrap().state, "awaiting-inclusion");
    assert_eq!(app.payment(&second.txid).unwrap().amount, second.amount);
    assert_eq!(app.status()["wallet"]["spendable"], 0);
    assert_eq!(app.status()["wallet"]["pending_incoming"], 40_010);
    app.stop().await;

    // A caught-up wallet still refuses an unknown/fabricated root instead of deferring forever.
    let other = dir.path().join("caught-up");
    join_invite(&invite, &other).unwrap(); opts.data = other.join("data"); opts.fleet = other.join("fleet.json"); opts.secret = other.join("secret.json");
    let app = start(&opts).await.unwrap();
    deliver(&app, &g, Body::Presence(Signed::sign(Presence {
        device: g.public().unwrap(), gateway: true, tip: 401, created_ms: now_ms(),
        address_proof: zcash_offline_core::binding::prove(&g).unwrap(),
    }, &g).unwrap())).await;
    early.tip_height = 401; early.tip_hash = "block401".into();
    app.state.lock().unwrap().0 = early;
    deliver(&app, &g, Body::Pay { tx: p.raw, statement }).await;
    assert_eq!(app.status()["pending_checks"], 1);
    app.retry_tick().await; // A timer is not a fresh chain response.
    assert!(app.state.lock().unwrap().1.refusals.is_empty());
    deliver(&app, &g, Body::Tip { height: 401, hash: "block401".into() }).await;
    app.retry_tick().await;
    assert_eq!(app.status()["pending_checks"], 0);
    assert_eq!(app.state.lock().unwrap().1.refusals.last().unwrap().reject.code, 6);
    assert!(app.payment(&p.txid).is_none());
    app.stop().await;
}

#[test]
fn generated_invites_join_and_never_overwrite_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut o = FleetInitOptions {
        out: dir.path().join("session"),
        name: "Three laptops".into(),
        network: Default::default(), birthday: 0,
        // Legacy roles stay accepted; only gateway is kept.
        devices: vec!["Operator:gateway+relay".into(), "Alice".into(), "Bob:payer+payee".into()],
    };
    let result = fleet_init(&o).unwrap();
    assert_eq!(result.devices.iter().map(|d| d.role.as_str()).collect::<Vec<_>>(), ["gateway", "", ""]);
    assert!(result.fleet_path.is_file());
    for device in &result.devices {
        let invite = std::fs::read_to_string(&device.invite_path).unwrap();
        let home = dir.path().join(&device.name);
        assert!(join_invite(&invite, &home).unwrap().contains(&device.name));
        let fleet = zcash_offline_core::Fleet::load(&home.join("fleet.json")).unwrap();
        assert_eq!(fleet.by_name(&device.name).unwrap().role, device.role);
        // Fleets and invites from before the network field stay Regtest sessions.
        let mut old: serde_json::Value = serde_json::from_str(&invite).unwrap();
        old["fleet"].as_object_mut().unwrap().remove("network");
        let old: ozm_node::Invite = serde_json::from_value(old).unwrap();
        assert_eq!(old.fleet.network, zcash_offline_core::fleet::Network::Regtest);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&device.invite_path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
    // The same device identity with another spending key must not replace the wallet's keys.
    let alice = result.devices.iter().find(|d| d.name == "Alice").unwrap();
    let home = dir.path().join("Alice");
    let keys = std::fs::read(home.join("secret.json")).unwrap();
    let mut swapped: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&alice.invite_path).unwrap()).unwrap();
    swapped["secret"]["ironwood_sk"] = zcash_offline_core::DeviceSecret::generate("Alice", "").ironwood_sk.into();
    assert!(join_invite(&swapped.to_string(), &home).is_err());
    assert_eq!(std::fs::read(home.join("secret.json")).unwrap(), keys);
    // A new session without a network choice (the Organiser and CLI default) is testnet.
    let new: FleetInitOptions = serde_json::from_value(serde_json::json!({"out": "x", "name": "n", "devices": []})).unwrap();
    assert_eq!(new.network, zcash_offline_core::fleet::Network::Testnet);
    let before = std::fs::read(&result.devices[0].secret_path).unwrap();
    assert!(fleet_init(&o).is_err());
    assert_eq!(std::fs::read(&result.devices[0].secret_path).unwrap(), before);
    o.out = dir.path().join("invalid");
    for devices in [vec!["G:gateway", "../escape"], vec!["Alice:gateway", "alice"], vec!["G:gateway", "D:notgateway"], vec!["D:gateway+"], vec!["A", "B:payer+relay"], vec![]] {
        o.devices = devices.into_iter().map(String::from).collect();
        assert!(fleet_init(&o).is_err());
        assert!(!o.out.exists());
    }
}

#[tokio::test]
async fn gateway_configuration_reuses_mesh_and_requires_invite_role() {
    let dir = tempfile::tempdir().unwrap();
    let result = fleet_init(&FleetInitOptions {
        out: dir.path().join("fleet"), name: "Roles".into(),
        network: Default::default(), birthday: 0,
        devices: vec!["D:relay+gateway".into(), "A:payer+payee".into()],
    }).unwrap();
    for device in &result.devices {
        let home = dir.path().join(&device.name);
        join_invite(&std::fs::read_to_string(&device.invite_path).unwrap(), &home).unwrap();
        let mut opts = RunOpts::for_home(&home);
        opts.listen = "127.0.0.1:0".parse().unwrap();
        opts.discover = false;
        opts.auto_sync_secs = 0;
        let app = start(&opts).await.unwrap();
        let before = app.status();
        let updated = app.configure_gateway("http://127.0.0.1:18232".into(), true);
        if device.name == "D" {
            updated.unwrap();
            let after = app.status();
            assert_eq!(after["peer_id"], before["peer_id"]);
            assert_eq!(after["address"], before["address"]);
            assert_eq!(after["online"], true);
            assert_eq!(after["mine"], true);
            app.configure_gateway("http://127.0.0.1:18233".into(), false).unwrap();
            let tls = app.configure_gateway("https://testnet.zec.rocks:443".into(), false).unwrap_err().to_string();
            assert!(tls.contains("Regtest session") && tls.contains("leave this session"), "{tls}");
            assert_eq!(app.status()["rpc"], "http://127.0.0.1:18233");
            assert_eq!(app.status()["mine"], false);
        } else {
            assert!(updated.unwrap_err().to_string().contains("does not grant the gateway role"));
            assert_eq!(app.status()["online"], false);
            opts.rpc = Some("http://127.0.0.1:18232".into());
            assert!(start(&opts).await.err().unwrap().to_string().contains("does not grant the gateway role"));
        }
        app.mesh.shutdown();
    }
}

#[tokio::test]
async fn open_identity_persists_and_first_used_gateway_stays_pinned() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{envelope::{self, Inner}, messages::{now_ms, Body, Presence, Signed}, DeviceSecret};
    let dir = tempfile::tempdir().unwrap();
    for (network, birthday, prefix) in [
        (zcash_offline_core::fleet::Network::Regtest, 1, "uregtest1"),
        (zcash_offline_core::fleet::Network::Testnet, 4_465_026, "utest1"),
    ] {
        let home = dir.path().join(network.to_string());
        let mut opts = RunOpts::for_home(&home);
        opts.network = network;
        opts.birthday = birthday;
        opts.listen = "127.0.0.1:0".parse().unwrap();
        opts.discover = false;
        opts.auto_sync_secs = 0;
        let app = start(&opts).await.unwrap();
        assert_eq!(app.status()["birthday"], birthday);
        assert!(app.status()["address"].as_str().unwrap().starts_with(prefix));
        assert!(app.request_funds(2, 10_000).await.is_err());
        app.mesh.shutdown();
    }
    let incomplete = dir.path().join("missing-keys");
    std::fs::create_dir(&incomplete).unwrap();
    std::fs::write(incomplete.join("fleet.json"), b"preserve existing device").unwrap();
    assert!(ozm_node::join_open(&incomplete).is_err());
    assert_eq!(std::fs::read(incomplete.join("fleet.json")).unwrap(), b"preserve existing device");
    let home = dir.path().join("open");
    ozm_node::join_open(&home).unwrap();
    let secret = std::fs::read(home.join("secret.json")).unwrap();
    ozm_node::join_open(&home).unwrap();
    assert_eq!(std::fs::read(home.join("secret.json")).unwrap(), secret);
    let mut o = RunOpts::for_home(&home);
    o.listen = "127.0.0.1:0".parse().unwrap();
    o.discover = false;
    o.auto_sync_secs = 0;
    let app = start(&o).await.unwrap();
    assert_eq!(app.status()["open_network"], true);
    let a = DeviceSecret::generate("Near", "gateway");
    let b = DeviceSecret::generate("Far", "gateway");
    let c = DeviceSecret::generate("Near", "gateway"); // Different operators can use the same display name.
    for (key, hops, tip, stale, corrupt) in [
        (&a, 1, 20, false, false), (&b, 2, 900, false, false),
        (&c, 1, 21, true, false), (&c, 1, 21, false, true),
    ] {
        let mut p = Signed::sign(Presence { device: key.public().unwrap(), gateway: true, tip,
            created_ms: if stale { now_ms() - 700_000 } else { now_ms() },
            address_proof: zcash_offline_core::binding::prove(key).unwrap() }, key).unwrap();
        if corrupt { p.body.tip += 1; }
        let inner = Inner { from: key.name.clone(), from_key: key.public().unwrap().ed25519, body: Body::Presence(p) };
        let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
        let inner = envelope::authenticate(inner, key, &recipient).unwrap();
        let payload = envelope::seal(&inner, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: key.public().unwrap().peer_id.parse().unwrap(),
            msg_id: MsgId::random(), payload, hops, broadcast: false }).await;
    }
    assert_eq!(app.status()["selected_gateway"]["name"], "Near");
    let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
    let inner = Inner { from: c.name.clone(), from_key: c.public().unwrap().ed25519,
        body: Body::Presence(Signed::sign(Presence { device: c.public().unwrap(), gateway: true, tip: 21, created_ms: now_ms(),
            address_proof: zcash_offline_core::binding::prove(&c).unwrap() }, &c).unwrap()) };
    let inner = envelope::authenticate(inner, &c, &recipient).unwrap();
    app.handle_mesh_event(MeshEvent::Message { from: c.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
        payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    // First valid gateway presence starts sync and pins a; a later higher tip cannot replace it.
    assert_eq!(app.status()["selected_gateway"]["peer_id"], a.public().unwrap().peer_id);
    let c = a;
    assert_eq!(app.status()["selected_gateway"]["peer_id"], c.public().unwrap().peer_id);
    assert_eq!(app.status()["selected_gateway"]["trust"], "gateway-reported");
    app.request_mesh_sync(true).await.unwrap();
    let batch = zcash_offline_core::sync::Batch { prev_height: 0, prev_hash: String::new(), seed: None,
        blocks: vec![zcash_offline_core::sync::CompactBlock { height: 1, hash: "block1".into(), txs: vec![] }],
        chain_tip: 1, sources: vec!["controlled source".into()] };
    let forged: Vec<Inner> = batch.chunks().unwrap().into_iter()
        .map(|part| Inner { from: c.name.clone(), from_key: c.public().unwrap().ed25519, body: Body::CompactPart(part) }).collect();
    // Public sealing keys are sufficient to encrypt, never to impersonate the gateway.
    for inner in forged.iter().cloned().chain(forged.iter().map(|f| envelope::authenticate(f.clone(), &b, &recipient).unwrap())) {
        app.handle_mesh_event(MeshEvent::Message { from: c.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
        assert_eq!(app.status()["wallet"]["tip"], 0);
    }
    for inner in forged {
        let signed = envelope::authenticate(inner, &c, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: c.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&signed, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    }
    assert_eq!(app.status()["wallet"]["tip"], 1);
    let pins: serde_json::Value = serde_json::from_slice(&std::fs::read(o.data.join("trust.json")).unwrap()).unwrap();
    assert_eq!(pins["gateway"]["peer_id"], c.public().unwrap().peer_id);
    let better = Inner { from: b.name.clone(), from_key: b.public().unwrap().ed25519,
        body: Body::Presence(Signed::sign(Presence { device: b.public().unwrap(), gateway: true, tip: 10_000,
            created_ms: now_ms() + 1, address_proof: zcash_offline_core::binding::prove(&b).unwrap() }, &b).unwrap()) };
    let better = envelope::authenticate(better, &b, &recipient).unwrap();
    app.handle_mesh_event(MeshEvent::Message { from: b.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
        payload: envelope::seal(&better, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    assert_eq!(app.status()["selected_gateway"]["peer_id"], c.public().unwrap().peer_id);
    app.mesh.shutdown();
    let restarted = start(&o).await.unwrap();
    assert!(restarted.status()["gateway_warning"].as_str().unwrap().contains(&c.public().unwrap().peer_id));
    restarted.mesh.shutdown();
}

#[tokio::test]
async fn compact_requests_send_history_only_after_a_matching_gateway_challenge() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{envelope::{self, Inner}, messages::Body, DeviceSecret};
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "sync".into(),
        network: Default::default(), birthday: 0, devices: vec!["A:payer".into(), "G:gateway".into()] }).unwrap();
    let home = dir.path().join("A");
    join_invite(&std::fs::read_to_string(&fleet.devices[0].invite_path).unwrap(), &home).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    let app = start(&opts).await.unwrap();
    let hash = "ab".repeat(32);
    {
        let mut st = app.state.lock().unwrap();
        st.0.birthday = 1;
        st.0.tip_height = 200;
        st.0.tip_hash = hash.clone();
        st.0.hashes = (1..=200).map(|h| (h, hash.clone())).collect();
    }
    app.request_mesh_sync(true).await.unwrap();
    let g: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[1].secret_path).unwrap()).unwrap();
    let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
    for body in [
        Body::Tip { height: 200, hash: hash.clone() },
        Body::CheckpointReq { have_height: 199, have_hash: hash.clone() },
        Body::CheckpointReq { have_height: 200, have_hash: "wrong".into() },
        Body::CheckpointReq { have_height: 200, have_hash: hash },
    ] {
        let inner = envelope::authenticate(Inner { from: "G".into(), from_key: g.public().unwrap().ed25519, body }, &g, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: g.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    }
    let events = std::fs::read_to_string(opts.data.join("events.jsonl")).unwrap();
    let sizes: Vec<_> = events.lines().map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .filter(|v| v["event"] == "mesh-send" && v["detail"]["kind"] == "syncq")
        .map(|v| v["detail"]["bytes"].as_u64().unwrap()).collect();
    assert_eq!(sizes, vec![512 + 65, 16384 + 65]);
    app.mesh.shutdown();
}

#[tokio::test]
async fn local_api_requires_token_and_loopback_host() {
    let dir = tempfile::tempdir().unwrap();
    ozm_node::join_open(dir.path()).unwrap();
    let mut opts = RunOpts::for_home(dir.path());
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    let app = start(&opts).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = ozm_node::router(app.clone(), false);
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::task::spawn_blocking(move || {
        let status = |r: Result<ureq::Response, ureq::Error>| match r {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r.status(), Err(e) => panic!("{e}"),
        };
        let html = ureq::get(&url).call().unwrap().into_string().unwrap();
        let token = html.split("name=\"ozm-api-token\" content=\"").nth(1).unwrap().split('"').next().unwrap();
        assert_eq!(status(ureq::get(&format!("{url}/api/status")).call()), 401);
        assert_eq!(status(ureq::post(&format!("{url}/api/pay")).send_json(serde_json::json!({"to": "stolen", "amount": "1"}))), 401);
        assert_eq!(status(ureq::get(&format!("{url}/api/status")).set("x-ozm-token", "wrong").call()), 401);
        assert_eq!(status(ureq::get(&format!("{url}/api/status")).set("x-ozm-token", token).call()), 200);
        for host in ["attacker.example:7780", "127.0.0.1.attacker.example", "localhost.attacker.example"] {
            assert_eq!(status(ureq::get(&url).set("host", host).call()), 403);
            assert_eq!(status(ureq::get(&format!("{url}/api/status")).set("host", host).set("x-ozm-token", token).call()), 403);
        }
    }).await.unwrap();
    server.abort(); app.mesh.shutdown();
}

#[cfg(unix)]
#[tokio::test]
async fn private_files_stay_private_on_write_and_upgrade() {
    use std::os::unix::fs::PermissionsExt;
    use zcash_offline_core::wallet::{write_atomic, WalletState};
    let dir = tempfile::tempdir().unwrap();
    ozm_node::join_open(dir.path()).unwrap();
    let mut opts = RunOpts::for_home(dir.path());
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    std::fs::create_dir_all(&opts.data).unwrap();
    let wallet = opts.data.join("wallet.json");
    let book = opts.data.join("payments.json");
    WalletState::default().save(&wallet).unwrap();
    write_atomic(&book, &serde_json::to_vec(&ozm_node::app::Book::default()).unwrap()).unwrap();
    // A crash can leave an older, world-readable temporary file.
    std::fs::write(wallet.with_extension("tmp"), b"old").unwrap();
    std::fs::set_permissions(wallet.with_extension("tmp"), std::fs::Permissions::from_mode(0o644)).unwrap();
    WalletState::default().save(&wallet).unwrap();
    assert_eq!(std::fs::metadata(&wallet).unwrap().permissions().mode() & 0o777, 0o600);
    let app = start(&opts).await.unwrap(); app.mesh.shutdown();
    let files = [wallet, book, opts.data.join("mesh-store.json"), dir.path().join("secret.json"), opts.data.join("events.jsonl")];
    for path in &files {
        assert_eq!(std::fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600, "{}", path.display());
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let app = start(&opts).await.unwrap();
    for path in &files {
        assert_eq!(std::fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600, "upgrade {}", path.display());
    }
    app.mesh.shutdown();
}

#[tokio::test]
async fn gateway_limits_unverified_submission_work_before_proof_checks() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{envelope::{self, Inner}, messages::Body, DeviceSecret};
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "quota".into(),
        network: Default::default(), birthday: 0, devices: vec!["A:payer".into(), "G:gateway".into()] }).unwrap();
    let home = dir.path().join("G");
    join_invite(&std::fs::read_to_string(&fleet.devices[1].invite_path).unwrap(), &home).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    opts.rpc = Some("http://127.0.0.1:0".into());
    let app = start(&opts).await.unwrap();
    let payer: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[0].secret_path).unwrap()).unwrap();
    let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
    for i in 0..5 {
        let inner = envelope::authenticate(Inner { from: "A".into(), from_key: payer.public().unwrap().ed25519,
            body: Body::Sub { tx: vec![0; 20], txid: format!("{i:064x}"), notify: vec![] } }, &payer, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: payer.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
    }
    let events = std::fs::read_to_string(opts.data.join("events.jsonl")).unwrap();
    let errors: Vec<_> = events.lines().map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .filter(|v| v["event"] == "error").collect();
    assert_eq!(errors.len(), 5);
    assert!(!errors[3]["detail"]["error"].as_str().unwrap().contains("rate limit"));
    assert!(errors[4]["detail"]["error"].as_str().unwrap().contains("rate limit"));
    assert_eq!(app.status()["gateway_submissions"], 0);
    app.mesh.shutdown();
}

fn recorded_payment(payer: &zcash_offline_core::DeviceSecret, txid: &str, amount: u64, state: &str) -> ozm_node::app::Payment {
    use zcash_offline_core::messages::{Receipt, ReceiptState, Signed, Statement};
    let statement = Signed::sign(Statement { txid: txid.into(), auth_digest: "auth".into(), amount, fee: 10_000,
        expiry_height: 100, payer: payer.name.clone(), payee: "B".into(), seq: 1, created_ms: 1 }, payer).unwrap();
    let receipt = Signed::sign(Receipt { txid: txid.into(), auth_digest: "auth".into(), amount,
        payee: "B".into(), state: ReceiptState::Pending, view_height: 1, created_ms: 1 }, payer).unwrap();
    serde_json::from_value(serde_json::json!({"txid": txid, "auth_digest": "auth", "direction": "received",
        "counterparty": payer.public().unwrap().address, "amount": amount, "fee": 10000, "expiry_height": 100,
        "state": state, "tx_hex": "00", "nullifiers": [], "statement": statement, "receipt": receipt,
        "result": null, "history": [], "created_ms": 1})).unwrap()
}

#[tokio::test]
async fn pending_caps_count_expired_debt_but_release_mined_payments() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{envelope::{self, Inner}, messages::{Body, Signed, Statement}, DeviceSecret};
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "exposure".into(),
        network: Default::default(), birthday: 0, devices: vec!["A".into(), "B".into(), "C".into(), "G:gateway".into()] }).unwrap();
    let a: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[0].secret_path).unwrap()).unwrap();
    let c: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[2].secret_path).unwrap()).unwrap();
    for (i, (payer, count, value, state, amount, capped)) in [
        (&a, 3, 10_000_000, "submitted", 1, true),
        (&a, 2, 40_000_000, "submitted", 20_000_000, false),
        (&a, 2, 40_000_000, "submitted", 20_000_001, true),
        (&c, 6, 50_000_000, "submitted", 1, true),
        (&c, 6, 49_999_999, "submitted", 6, false),
        (&a, 3, 10_000_000, "expired", 1, true),
        (&a, 3, 10_000_000, "mined", 1, false),
    ].into_iter().enumerate() {
        let home = dir.path().join(i.to_string());
        join_invite(&std::fs::read_to_string(&fleet.devices[1].invite_path).unwrap(), &home).unwrap();
        let mut opts = RunOpts::for_home(&home);
        opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
        std::fs::create_dir_all(&opts.data).unwrap();
        let mut book = ozm_node::app::Book::default();
        for n in 0..count {
            let txid = format!("{n:064x}");
            book.payments.insert(txid.clone(), recorded_payment(payer, &txid, value, state));
        }
        std::fs::write(opts.data.join("payments.json"), serde_json::to_vec(&book).unwrap()).unwrap();
        let app = start(&opts).await.unwrap();
        let txid = "new-payment";
        let mut statement = recorded_payment(&a, txid, amount, "sent").statement.unwrap();
        statement.body.seq = 10;
        let statement: Signed<Statement> = Signed::sign(statement.body, &a).unwrap();
        let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
        let inner = envelope::authenticate(Inner { from: "A".into(), from_key: a.public().unwrap().ed25519,
            body: Body::Pay { tx: vec![0; 20], statement } }, &a, &recipient).unwrap();
        app.handle_mesh_event(MeshEvent::Message { from: a.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
        let book: ozm_node::app::Book = serde_json::from_slice(&std::fs::read(opts.data.join("payments.json")).unwrap()).unwrap();
        assert_eq!(book.refusals.last().unwrap().reject.reason.contains("exposure cap"), capped, "case {i}");
        app.mesh.shutdown();
    }
}

#[tokio::test]
async fn submission_hides_counterparty_and_nonce_results_survive_restart_and_reorg() {
    use ozm_mesh::{Mesh, MeshConfig, MeshEvent, MsgId};
    use serde::{Deserialize, Serialize};
    use zcash_offline_core::{envelope, messages::{Body, Domain, GatewayResult, Signed, TxStatus}, DeviceSecret};
    #[derive(Serialize, Deserialize)]
    struct Notice { request_nonce: String, result: Signed<GatewayResult> }
    impl Domain for Notice { const DOMAIN: &'static [u8] = b"ozm1-gateway-notice-v1"; }
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "nonce".into(),
        network: Default::default(), birthday: 0, devices: vec!["A:payer".into(), "B:payee".into(), "G:gateway".into()] }).unwrap();
    let a: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[0].secret_path).unwrap()).unwrap();
    let g: DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[2].secret_path).unwrap()).unwrap();
    let home = dir.path().join("A");
    join_invite(&std::fs::read_to_string(&fleet.devices[0].invite_path).unwrap(), &home).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    std::fs::create_dir_all(&opts.data).unwrap();
    let txid = "payment";
    let mut book = ozm_node::app::Book::default();
    book.payments.insert(txid.into(), recorded_payment(&a, txid, 7, "submitted"));
    std::fs::write(opts.data.join("payments.json"), serde_json::to_vec(&book).unwrap()).unwrap();
    let app = start(&opts).await.unwrap();
    let incoming = zcash_offline_core::wallet::StoredNote {
        position: 0, height: 2, txid: txid.into(), value: 7,
        recipient: hex::encode(app.cfg.me.address().unwrap().to_raw_address_bytes()),
        rho: hex::encode([0; 32]), rseed: hex::encode([7; 32]), nullifier: String::new(),
        spent_in: None, locked_by: None,
    };
    app.state.lock().unwrap().0.record_incoming(vec![incoming.clone()]).unwrap();
    let (gateway, mut events) = Mesh::start(MeshConfig { peer_id: g.public().unwrap().peer_id.parse().unwrap(),
        ack_signer: Some(g.signing_key().unwrap()), ..Default::default() }).await.unwrap();
    let (left, right) = tokio::io::duplex(65536);
    let mesh = app.mesh.clone(); tokio::spawn(async move { mesh.attach(left, true).await });
    let mesh = gateway.clone(); tokio::spawn(async move { mesh.attach(right, false).await });
    app.resend(txid).await.unwrap();
    let nonce = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(MeshEvent::Message { payload, broadcast: false, .. }) = events.recv().await {
                let inner = envelope::open(&payload, &g.x25519_secret().unwrap()).unwrap();
                if let Body::Sub { notify, .. } = envelope::authenticated_body(&inner, &a.public().unwrap(), &g.public().unwrap().x25519().unwrap()).unwrap() {
                    assert_eq!(notify.len(), 1);
                    let nonce = notify[0].strip_prefix("nonce:").unwrap().to_string();
                    assert_eq!(nonce.len(), 32);
                    break nonce;
                }
            }
        }
    }).await.unwrap();
    app.mesh.shutdown(); gateway.shutdown();
    let app = start(&opts).await.unwrap();
    for (seq, request_nonce, signer, status, expected, pending) in [
        (1, "wrong".to_string(), &g, TxStatus::Mined { height: 2, block_hash: "block".into() }, "submitted", 7),
        (2, nonce.clone(), &a, TxStatus::Mined { height: 2, block_hash: "block".into() }, "submitted", 7),
        (3, nonce.clone(), &g, TxStatus::Mined { height: 2, block_hash: "block".into() }, "mined", 7),
        (4, nonce.clone(), &g, TxStatus::Submitted, "submitted", 7),
        (5, nonce.clone(), &g, TxStatus::Expired, "expired", 0),
        (6, nonce, &g, TxStatus::Conflict { reason: "competing spend".into() }, "conflict", 0),
    ] {
        app.state.lock().unwrap().0.record_incoming(vec![incoming.clone()]).unwrap();
        let result = Signed::sign(GatewayResult { txid: txid.into(), status, tip_height: 2,
            gateway: "G".into(), seq, created_ms: 1 }, &g).unwrap();
        let notice = Signed::sign(Notice { request_nonce, result }, signer).unwrap();
        let mut payload = b"ozm1-result:".to_vec(); payload.extend(serde_json::to_vec(&notice).unwrap());
        app.handle_mesh_event(MeshEvent::Message { from: g.public().unwrap().peer_id.parse().unwrap(),
            msg_id: MsgId::random(), payload, hops: 1, broadcast: true }).await;
        assert_eq!(app.payment(txid).unwrap().state, expected);
        assert_eq!(app.status()["wallet"]["pending_incoming"], pending);
        assert_eq!(app.status()["wallet"]["spendable"], 0);
    }
    app.mesh.shutdown();
}

#[test]
fn leave_keeps_a_backup_and_session_exports() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("ozm");
    let exported = home.join("sessions/1");
    let result = fleet_init(&FleetInitOptions { out: exported.clone(), name: "leave".into(),
        network: Default::default(), birthday: 0, devices: vec!["G:gateway".into()] }).unwrap();
    let secret = std::fs::read(&result.devices[0].secret_path).unwrap();
    join_invite(&std::fs::read_to_string(&result.devices[0].invite_path).unwrap(), &home).unwrap();
    let backup = ozm_node::leave_home(&home).unwrap();
    assert!(!home.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777, 0o700, "key folders are owner-only");
    }
    assert!(backup.file_name().unwrap().to_str().unwrap().starts_with("ozm-backup-20"));
    assert_eq!(std::fs::read(backup.join("secret.json")).unwrap(), secret);
    assert!(dir.path().join("sessions/1/G.invite.json").is_file());
    assert!(ozm_node::leave_home(&home).is_err());
    ozm_node::join_open(&home).unwrap();
    assert!(home.join("secret.json").is_file());
    // The invite-free wallet survives Leave and a later invited session; the session does not.
    let own = std::fs::read(home.join("secret.json")).unwrap();
    std::fs::create_dir_all(home.join("data")).unwrap();
    for (file, body) in [("wallet.json", "notes"), ("payments.json", "history"), ("trust.json", "gateway pin")] {
        std::fs::write(home.join("data").join(file), body).unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(1100)); // Backup names have 1 s resolution.
    ozm_node::leave_home(&home).unwrap();
    join_invite(&std::fs::read_to_string(dir.path().join("sessions/1/G.invite.json")).unwrap(), &home).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ozm_node::leave_home(&home).unwrap();
    ozm_node::join_open(&home).unwrap();
    assert_eq!(std::fs::read(home.join("secret.json")).unwrap(), own);
    assert_eq!(std::fs::read_to_string(home.join("data/wallet.json")).unwrap(), "notes");
    assert_eq!(std::fs::read_to_string(home.join("data/payments.json")).unwrap(), "history");
    assert!(!home.join("data/trust.json").exists());
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ozm_node::leave_home(&home).unwrap();
    ozm_node::join_open_on(&home, zcash_offline_core::fleet::Network::Testnet, 0).unwrap();
    let testnet = std::fs::read(home.join("secret.json")).unwrap();
    assert_ne!(testnet, own, "wallets are kept per network");
    // An empty wallet is not restored, so a new start scans from near the tip, not its old start.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ozm_node::leave_home(&home).unwrap();
    ozm_node::join_open_on(&home, zcash_offline_core::fleet::Network::Testnet, 0).unwrap();
    let fresh = std::fs::read(home.join("secret.json")).unwrap();
    assert_ne!(fresh, testnet, "empty wallets are not restored");
    // A payment still waiting for its chain view keeps the wallet worth restoring.
    std::fs::create_dir_all(home.join("data")).unwrap();
    std::fs::write(home.join("data/payments.json"), r#"{"payments":{},"refusals":[],"gateway":{},"deferred_payments":{"tx":{}}}"#).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ozm_node::leave_home(&home).unwrap();
    ozm_node::join_open_on(&home, zcash_offline_core::fleet::Network::Testnet, 0).unwrap();
    assert_eq!(std::fs::read(home.join("secret.json")).unwrap(), fresh, "a deferred incoming payment is kept");
    // New wallet on request, even when the backup holds funds.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ozm_node::leave_home(&home).unwrap();
    ozm_node::join_open_with(&home, zcash_offline_core::fleet::Network::Regtest, 0, false).unwrap();
    assert_ne!(std::fs::read(home.join("secret.json")).unwrap(), own, "a new wallet was requested");
}

/// An invite-free testnet wallet whose chain server accepts connections and never answers, so
/// online mode stays active (no fallback) for the 15 s connection bound.
async fn silent_server_wallet(dir: &std::path::Path) -> (RunOpts, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("https://{}", listener.local_addr().unwrap());
    let silent = tokio::spawn(async move {
        let mut held = vec![];
        while let Ok((socket, _)) = listener.accept().await { held.push(socket); }
    });
    let home = dir.join("wallet");
    ozm_node::join_open_on(&home, zcash_offline_core::fleet::Network::Testnet, 4_465_026).unwrap();
    let mut fleet: serde_json::Value = serde_json::from_slice(&std::fs::read(home.join("fleet.json")).unwrap()).unwrap();
    fleet["servers"] = serde_json::json!([server]);
    std::fs::write(home.join("fleet.json"), serde_json::to_vec(&fleet).unwrap()).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap();
    opts.discover = false;
    opts.auto_sync_secs = 0;
    opts.birthday = 4_465_026;
    (opts, silent)
}

/// An invite-free identity, named by its mesh ID as `join_open` names them.
fn open_key(role: &str) -> zcash_offline_core::DeviceSecret {
    let mut key = zcash_offline_core::DeviceSecret::generate("", role);
    key.name = key.public().unwrap().peer_id;
    key
}

async fn deliver(app: &std::sync::Arc<ozm_node::App>, sender: &zcash_offline_core::DeviceSecret, body: zcash_offline_core::messages::Body) {
    use zcash_offline_core::envelope::{self, Inner};
    let recipient = app.cfg.me.public().unwrap().x25519().unwrap();
    let inner = envelope::authenticate(Inner { from: sender.name.clone(), from_key: sender.public().unwrap().ed25519, body }, sender, &recipient).unwrap();
    app.handle_mesh_event(ozm_mesh::MeshEvent::Message { from: sender.public().unwrap().peer_id.parse().unwrap(),
        msg_id: ozm_mesh::MsgId::random(), payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
}

fn presence(key: &zcash_offline_core::DeviceSecret, gateway: bool) -> zcash_offline_core::messages::Body {
    use zcash_offline_core::{fleet::Network, messages::{now_ms, Body, Presence, Signed}};
    Body::Presence(Signed::sign(Presence { device: key.public_on(Network::Testnet).unwrap(), gateway, tip: 4_500_000, created_ms: now_ms(),
        address_proof: zcash_offline_core::binding::prove_on(key, Network::Testnet).unwrap() }, key).unwrap())
}

fn events(app: &ozm_node::App) -> Vec<serde_json::Value> {
    std::fs::read_to_string(app.cfg.data.join("events.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[tokio::test]
async fn online_mode_wallet_never_announces_or_serves_as_a_gateway() {
    use std::time::Duration;
    use zcash_offline_core::{envelope, messages::Body};
    let dir = tempfile::tempdir().unwrap();
    let (mut opts, silent) = silent_server_wallet(dir.path()).await;
    opts.direct = true;
    let app = start(&opts).await.unwrap();
    assert_eq!(app.status()["direct"], true);
    assert_eq!(app.status()["sync_via"], "internet");
    // A neighbour that announces its key receives the wallet's sealed presence: no gateway claim.
    let observer = open_key("");
    let (mesh, mut inbox) = ozm_mesh::Mesh::start(ozm_mesh::MeshConfig {
        peer_id: observer.public().unwrap().peer_id.parse().unwrap(), ack_signer: Some(observer.signing_key().unwrap()),
        listen: Some("127.0.0.1:0".parse().unwrap()), dial: vec![app.mesh.listen_addr().unwrap().to_string()], ..Default::default()
    }).await.unwrap();
    let mut key = b"ozmk".to_vec();
    key.extend_from_slice(observer.signing_key().unwrap().verifying_key().as_bytes());
    mesh.broadcast(key).await.unwrap();
    let me = app.cfg.me.public_on(app.cfg.fleet.network).unwrap();
    let observer_x = x25519_dalek::PublicKey::from(&observer.x25519_secret().unwrap()).to_bytes();
    let announced = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let Some(ozm_mesh::MeshEvent::Message { payload, broadcast: false, .. }) = inbox.recv().await else { continue };
            let inner = envelope::open(&payload, &observer.x25519_secret().unwrap()).unwrap();
            if let Body::Presence(p) = envelope::authenticated_body(&inner, &me, &observer_x).unwrap() { break p; }
        }
    }).await.expect("no presence from the online-mode wallet");
    assert_eq!(announced.body.device.peer_id, me.peer_id);
    assert!(!announced.body.gateway, "an online-mode wallet must never advertise itself as a gateway");
    mesh.shutdown();
    // Known to the wallet, a peer still gets no chain data, clock, faucet or submission service.
    deliver(&app, &observer, presence(&observer, false)).await;
    let tx = vec![0; 100];
    for body in [
        Body::CompactReq { have_height: 0, have_hash: String::new(), checkpoints: vec![], birthday: 4_465_026 },
        Body::SyncReq { have_height: 0, have_hash: String::new() },
        Body::ChainClockReq,
        Body::Sub { tx, txid: "ab".repeat(32), notify: vec![] },
    ] {
        deliver(&app, &observer, body).await;
    }
    let log = events(&app);
    let refused = log.iter().filter(|e| e["event"] == "error" && e["detail"]["from"] == observer.public().unwrap().peer_id).count();
    assert_eq!(refused, 4, "{log:#?}");
    assert!(!log.iter().any(|e| e["event"] == "sync-served" || e["event"] == "gateway-result"
        || (e["event"] == "mesh-send" && e["detail"]["kind"] != "presence")), "{log:#?}");
    assert_eq!(app.status()["gateway_submissions"], 0);
    assert_eq!(app.status()["direct"], true);
    app.stop().await;
    silent.abort();
}

#[tokio::test]
async fn online_mode_switch_persists_and_moves_sync_between_servers_and_mesh() {
    use std::time::Duration;
    use zcash_offline_core::messages::Body;
    let dir = tempfile::tempdir().unwrap();
    let (opts, silent) = silent_server_wallet(dir.path()).await;
    let syncq = |app: &ozm_node::App| events(app).iter().filter(|e| e["event"] == "mesh-send" && e["detail"]["kind"] == "syncq").count();
    let app = start(&opts).await.unwrap();
    assert_eq!(app.status()["sync_via"], "mesh");
    let gateway = open_key("gateway");
    deliver(&app, &gateway, presence(&gateway, true)).await;
    assert_eq!(syncq(&app), 1, "a mesh wallet asks the gateway it hears for compact blocks");
    app.set_direct(true).unwrap();
    assert_eq!(app.status()["direct"], true);
    assert_eq!(app.status()["sync_via"], "internet");
    // The gateway's next tip is ignored instead of starting a second, interleaved mesh sync.
    deliver(&app, &gateway, Body::Tip { height: 4_500_001, hash: "cd".repeat(32) }).await;
    assert!(app.request_mesh_sync(true).await.is_err());
    assert_eq!(syncq(&app), 1);
    app.stop().await;
    let restarted = start(&opts).await.unwrap();
    assert_eq!(restarted.status()["direct"], true, "online mode survives a restart");
    assert_eq!(restarted.status()["sync_via"], "internet");
    deliver(&restarted, &gateway, presence(&gateway, true)).await;
    let before = syncq(&restarted);
    // Switching off resumes mesh sync through the pinned gateway at once.
    restarted.set_direct(false).unwrap();
    assert_eq!(restarted.status()["sync_via"], "mesh");
    tokio::time::timeout(Duration::from_secs(5), async {
        while syncq(&restarted) == before { tokio::time::sleep(Duration::from_millis(20)).await; }
    }).await.expect("no mesh sync after switching online mode off");
    restarted.stop().await;
    let off = start(&opts).await.unwrap();
    assert_eq!(off.status()["direct"], false);
    assert!(off.set_direct(true).is_ok());
    off.stop().await;
    silent.abort();
}

#[tokio::test]
async fn included_payments_keep_resending_at_the_longest_backoff() {
    // A payer told "included" only by the payee's receipt has no gateway nonce yet; resending is how it
    // learns mined, and a prepare stuck at included would block automatic preparation for good.
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "included".into(),
        network: Default::default(), birthday: 0, devices: vec!["A".into(), "B".into(), "G:gateway".into()] }).unwrap();
    let a: zcash_offline_core::DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[0].secret_path).unwrap()).unwrap();
    let home = dir.path().join("a");
    join_invite(&std::fs::read_to_string(&fleet.devices[0].invite_path).unwrap(), &home).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    let app = start(&opts).await.unwrap();
    let now = zcash_offline_core::messages::now_ms();
    let mut p = recorded_payment(&a, &"ab".repeat(32), 1_000, "included");
    p.direction = "sent".into();
    p.attempts = 1;
    // Ten minutes is past the first-attempt backoff (30 s) but inside the longest (32 × 30 s).
    p.last_send_ms = now - 10 * 60_000;
    app.state.lock().unwrap().1.payments.insert(p.txid.clone(), p.clone());
    app.retry_tick().await;
    assert_eq!(app.payment(&p.txid).unwrap().attempts, 1, "included waits for the longest backoff");
    app.state.lock().unwrap().1.payments.get_mut(&p.txid).unwrap().last_send_ms = now - 17 * 60_000;
    app.retry_tick().await;
    assert_eq!(app.payment(&p.txid).unwrap().attempts, 2, "included is still resent");
    app.mesh.shutdown();
}

#[tokio::test]
async fn only_the_ack_of_the_pay_message_marks_delivered_even_after_restart() {
    use ozm_mesh::{MeshEvent, MsgId};
    let dir = tempfile::tempdir().unwrap();
    let fleet = fleet_init(&FleetInitOptions { out: dir.path().join("fleet"), name: "delivered".into(),
        network: Default::default(), birthday: 0, devices: vec!["A".into(), "B".into(), "G:gateway".into()] }).unwrap();
    let a: zcash_offline_core::DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[0].secret_path).unwrap()).unwrap();
    let b: zcash_offline_core::DeviceSecret = serde_json::from_slice(&std::fs::read(&fleet.devices[1].secret_path).unwrap()).unwrap();
    let home = dir.path().join("a");
    join_invite(&std::fs::read_to_string(&fleet.devices[0].invite_path).unwrap(), &home).unwrap();
    let mut opts = RunOpts::for_home(&home);
    opts.listen = "127.0.0.1:0".parse().unwrap(); opts.discover = false; opts.auto_sync_secs = 0;
    std::fs::create_dir_all(&opts.data).unwrap();
    // The Pay was sent before a restart; only its persisted msg_id links the later ack to it.
    let pay = MsgId::random();
    let mut p = recorded_payment(&a, &"cd".repeat(32), 1_000, "sent");
    p.direction = "sent".into();
    p.receipt_peer = Some("B".into());
    p.pay_msgs = vec![pay.to_string()];
    let mut book = ozm_node::app::Book::default();
    book.payments.insert(p.txid.clone(), p.clone());
    std::fs::write(opts.data.join("payments.json"), serde_json::to_vec(&book).unwrap()).unwrap();
    let app = start(&opts).await.unwrap();
    let by = b.public().unwrap().peer_id.parse().unwrap();
    app.handle_mesh_event(MeshEvent::Delivered { msg_id: MsgId::random(), by }).await;
    assert_eq!(app.payment(&p.txid).unwrap().state, "sent", "an ack of another message from the payee");
    app.handle_mesh_event(MeshEvent::Delivered { msg_id: pay, by }).await;
    assert_eq!(app.payment(&p.txid).unwrap().state, "delivered");
    app.mesh.shutdown();
}
