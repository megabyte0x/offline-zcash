use ozm_node::{fleet_init, join_invite, start, FleetInitOptions, RunOpts};

#[test]
fn generated_invites_join_and_never_overwrite_keys() {
    let dir = tempfile::tempdir().unwrap();
    let mut o = FleetInitOptions {
        out: dir.path().join("session"),
        name: "Three laptops".into(),
        devices: vec!["Operator:gateway+relay".into(), "Alice:payer+payee".into(), "Bob:payee".into()],
    };
    let result = fleet_init(&o).unwrap();
    assert_eq!(result.devices.len(), 3);
    assert!(result.fleet_path.is_file());
    for device in &result.devices {
        let invite = std::fs::read_to_string(&device.invite_path).unwrap();
        let home = dir.path().join(&device.name);
        assert!(join_invite(&invite, &home).unwrap().contains(&device.name));
        let fleet = zcash_offline_core::Fleet::load(&home.join("fleet.json")).unwrap();
        assert_eq!(fleet.by_name(&device.name).unwrap().role, device.role);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&device.invite_path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
    let before = std::fs::read(&result.devices[0].secret_path).unwrap();
    assert!(fleet_init(&o).is_err());
    assert_eq!(std::fs::read(&result.devices[0].secret_path).unwrap(), before);
    o.out = dir.path().join("invalid");
    for devices in [vec!["../escape:payer"], vec!["Alice:payer", "alice:payee"], vec!["D:notgateway"], vec!["D:gateway+"], vec![]] {
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
async fn open_identity_persists_and_signed_gateways_rank_by_hops_then_tip() {
    use ozm_mesh::{MeshEvent, MsgId};
    use zcash_offline_core::{envelope::{self, Inner}, messages::{now_ms, Body, Presence, Signed}, DeviceSecret};
    let dir = tempfile::tempdir().unwrap();
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
            created_ms: if stale { now_ms() - 40_000 } else { now_ms() },
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
    assert_eq!(app.status()["selected_gateway"]["peer_id"], c.public().unwrap().peer_id);
    assert_eq!(app.status()["selected_gateway"]["trust"], "gateway-reported");
    app.request_mesh_sync(true).await.unwrap(); // Pins c at first use, not at first announcement.
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
