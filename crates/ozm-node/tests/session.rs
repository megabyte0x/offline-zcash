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
