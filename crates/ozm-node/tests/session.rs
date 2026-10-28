use ozm_node::{fleet_init, join_invite, start, FleetInitOptions, RunOpts};

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
        opts.listen = "127.0.0.1:0".parse().unwrap();
        opts.discover = false;
        opts.auto_sync_secs = 0;
        let app = start(&opts).await.unwrap();
        assert_eq!(app.cfg.fleet.birthday, birthday);
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
    let forged = Inner { from: c.name.clone(), from_key: c.public().unwrap().ed25519,
        body: Body::SyncRes { reset: false, prev_hash: String::new(), chain_tip: 1,
            blocks: vec![zcash_offline_core::wallet::BlockData { height: 1, hash: "block1".into(), txs: vec![] }] } };
    // Public sealing keys are sufficient to encrypt, never to impersonate the gateway.
    for inner in [forged.clone(), envelope::authenticate(forged.clone(), &b, &recipient).unwrap()] {
        app.handle_mesh_event(MeshEvent::Message { from: c.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
            payload: envelope::seal(&inner, &recipient).unwrap(), hops: 1, broadcast: false }).await;
        assert_eq!(app.status()["wallet"]["tip"], 0);
    }
    let signed = envelope::authenticate(forged, &c, &recipient).unwrap();
    app.handle_mesh_event(MeshEvent::Message { from: c.public().unwrap().peer_id.parse().unwrap(), msg_id: MsgId::random(),
        payload: envelope::seal(&signed, &recipient).unwrap(), hops: 1, broadcast: false }).await;
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
    for (seq, request_nonce, signer, status, expected) in [
        (1, "wrong".to_string(), &g, TxStatus::Mined { height: 2, block_hash: "block".into() }, "submitted"),
        (2, nonce.clone(), &a, TxStatus::Mined { height: 2, block_hash: "block".into() }, "submitted"),
        (3, nonce.clone(), &g, TxStatus::Mined { height: 2, block_hash: "block".into() }, "mined"),
        (4, nonce, &g, TxStatus::Submitted, "submitted"),
    ] {
        let result = Signed::sign(GatewayResult { txid: txid.into(), status, tip_height: 2,
            gateway: "G".into(), seq, created_ms: 1 }, &g).unwrap();
        let notice = Signed::sign(Notice { request_nonce, result }, signer).unwrap();
        let mut payload = b"ozm1-result:".to_vec(); payload.extend(serde_json::to_vec(&notice).unwrap());
        app.handle_mesh_event(MeshEvent::Message { from: g.public().unwrap().peer_id.parse().unwrap(),
            msg_id: MsgId::random(), payload, hops: 1, broadcast: true }).await;
        assert_eq!(app.payment(txid).unwrap().state, expected);
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
    assert_ne!(std::fs::read(home.join("secret.json")).unwrap(), own, "wallets are kept per network");
}
