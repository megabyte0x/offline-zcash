//! `ozm-node` as a library: the device (mesh, wallet, payment roles) that both
//! the `ozm-node` binary and the Logos Basecamp module run.
#![allow(clippy::result_large_err)] // axum handlers return Response as the error type

pub mod app;
mod discovery;
pub mod lightwallet;

pub use app::App;

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::State,
    http::{StatusCode, Request},
    middleware::{self, Next},
    body::Body,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::Args;
use ozm_mesh::{Discovery, Mesh, MeshConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zcash_offline_core::{fleet::{generate_fleet_on, is_gateway_role, Network}, parse_zec, DeviceSecret, Fleet, Policy};

#[derive(Args, Clone)]
pub struct RunOpts {
    /// Directory for wallet, payments, mesh store and events.
    #[arg(long, env = "OZM_DATA")]
    pub data: PathBuf,
    #[arg(long, env = "OZM_FLEET")]
    pub fleet: PathBuf,
    #[arg(long, env = "OZM_SECRET")]
    pub secret: PathBuf,
    /// Network for a newly created invite-free identity; existing fleet files remain authoritative.
    #[arg(long, env = "OZM_NETWORK", default_value = "testnet")]
    pub network: Network,
    /// Bench-only first scan height override; 0 chooses automatically.
    #[arg(long, env = "OZM_BIRTHDAY", default_value_t = 0)]
    pub birthday: u32,
    /// Mesh TCP listen address.
    #[arg(long, env = "OZM_LISTEN", default_value = "0.0.0.0:7777")]
    pub listen: SocketAddr,
    /// Mesh neighbours to dial (host:port), repeatable or comma-separated.
    #[arg(long, env = "OZM_DIAL", value_delimiter = ',')]
    pub dial: Vec<String>,
    /// Find Offline Zcash devices on the local network (UDP beacons) and connect to them.
    #[arg(long, env = "OZM_DISCOVER")]
    pub discover: bool,
    /// Scan and advertise Bluetooth LE alongside LAN (Linux / BlueZ only).
    #[arg(long, env = "OZM_BLE")]
    pub ble: bool,
    #[arg(long, env = "OZM_DISCOVERY_PORT", default_value_t = 7778)]
    pub discovery_port: u16,
    /// Zakura JSON-RPC URL. Only online devices (gateway) set this.
    #[arg(long, env = "OZM_RPC")]
    pub rpc: Option<String>,
    /// Gateway: mine a block after each submission and serve test funds (Regtest only).
    #[arg(long, env = "OZM_MINE")]
    pub mine: bool,
    /// Gateway: most test funds served per request, in ZEC (0 disables).
    #[arg(long, env = "OZM_FAUCET_MAX", default_value = "1")]
    pub faucet_max: String,
    /// Local HTTP API and UI.
    #[arg(long, env = "OZM_API", default_value = "127.0.0.1:7780")]
    pub api: SocketAddr,
    /// Emulated BLE: sleep per frame, in milliseconds.
    #[arg(long, env = "OZM_FRAME_DELAY_MS", default_value_t = 0)]
    pub frame_delay_ms: u64,
    #[arg(long, env = "OZM_MTU", default_value_t = 512)]
    pub mtu: usize,
    /// Base app retry interval in seconds (doubles per attempt).
    #[arg(long, env = "OZM_RETRY_SECS", default_value_t = 30)]
    pub retry_secs: u64,
    /// Slow mesh-sync fallback in seconds (minimum 900, backs off to 3600; 0 disables).
    #[arg(long, env = "OZM_AUTO_SYNC_SECS", default_value_t = 900)]
    pub auto_sync_secs: u64,
    /// Enable test-only endpoints (tampered payments, resend).
    #[arg(long, env = "OZM_DEBUG_API")]
    pub debug_api: bool,
    /// Open the UI in the default browser once started.
    #[arg(long)]
    pub open: bool,
}

impl RunOpts {
    /// Defaults for a device installed with `join` in `home` (the `start` command and the Basecamp module).
    pub fn for_home(home: &Path) -> Self {
        RunOpts {
            data: home.join("data"),
            fleet: home.join("fleet.json"),
            secret: home.join("secret.json"),
            network: Network::NEW_SESSION,
            birthday: 0,
            listen: "0.0.0.0:7777".parse().unwrap(),
            dial: vec![],
            discover: true,
            ble: false,
            discovery_port: 7778,
            rpc: None,
            mine: false,
            faucet_max: "1".into(),
            api: "127.0.0.1:7780".parse().unwrap(),
            frame_delay_ms: 0,
            mtu: 512,
            retry_secs: 30,
            auto_sync_secs: 900,
            debug_api: false,
            open: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Invite {
    pub fleet: Fleet,
    pub secret: DeviceSecret,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetInitOptions {
    pub out: PathBuf,
    pub name: String,
    #[serde(default = "new_session_network")]
    pub network: Network,
    #[serde(default)]
    pub birthday: u32,
    /// NAME, or NAME:gateway for a gateway. Legacy payer/payee/relay roles are accepted and ignored.
    pub devices: Vec<String>,
}

fn new_session_network() -> Network {
    Network::NEW_SESSION
}

#[derive(Serialize)]
pub struct FleetInitResult {
    pub folder: PathBuf,
    pub fleet_path: PathBuf,
    pub devices: Vec<FleetInvite>,
}

#[derive(Serialize)]
pub struct FleetInvite {
    pub name: String,
    pub role: String,
    pub peer_id: String,
    pub address: String,
    pub invite_path: PathBuf,
    pub secret_path: PathBuf,
}

/// Creates the fleet and private per-device invites, shared by the CLI and Basecamp.
pub fn fleet_init(o: &FleetInitOptions) -> Result<FleetInitResult> {
    anyhow::ensure!(!o.name.trim().is_empty(), "session name is required");
    let devs = o.devices.iter().map(|d| {
        let (n, r) = d.split_once(':').unwrap_or((d, ""));
        anyhow::ensure!(!n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "device name must be 1–64 letters, digits, - or _: {n}");
        anyhow::ensure!(r.is_empty() || r.split('+').all(|r| matches!(r, "payer" | "payee" | "relay" | "gateway")), "expected NAME or NAME:gateway, got {d}");
        Ok((n.to_string(), if is_gateway_role(r) { "gateway" } else { "" }.to_string()))
    }).collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(devs.iter().any(|(_, r)| r == "gateway"), "at least one device must be the gateway");
    for (i, (n, _)) in devs.iter().enumerate() {
        anyhow::ensure!(!devs[..i].iter().any(|(other, _)| other.eq_ignore_ascii_case(n)), "duplicate device name: {n}");
    }
    anyhow::ensure!(!o.out.join("fleet.json").exists(), "folder already contains a fleet; choose a new folder");
    for (n, _) in &devs {
        for suffix in ["secret", "invite"] {
            anyhow::ensure!(!o.out.join(format!("{n}.{suffix}.json")).exists(), "folder already contains keys for {n}; choose a new folder");
        }
    }
    let (fleet, secrets) = generate_fleet_on(&o.name, &devs, Policy::default(), o.network, o.birthday)?;
    std::fs::create_dir_all(&o.out)?;
    let folder = o.out.canonicalize()?;
    let fleet_path = folder.join("fleet.json");
    std::fs::write(&fleet_path, serde_json::to_string_pretty(&fleet)?)?;
    let mut devices = Vec::new();
    for s in secrets {
        let secret_path = folder.join(format!("{}.secret.json", s.name));
        let invite_path = folder.join(format!("{}.invite.json", s.name));
        write_private(&secret_path, serde_json::to_string_pretty(&s)?.as_bytes())?;
        write_private(&invite_path, serde_json::to_string_pretty(&Invite { fleet: fleet.clone(), secret: s.clone() })?.as_bytes())?;
        let d = fleet.by_name(&s.name)?;
        devices.push(FleetInvite { name: d.name.clone(), role: d.role.clone(), peer_id: d.peer_id.clone(), address: d.address.clone(), invite_path, secret_path });
    }
    Ok(FleetInitResult { folder, fleet_path, devices })
}

/// Regtest gateways speak Zakura JSON-RPC over plain HTTP; TLS servers are testnet lightwalletd.
pub fn regtest_rpc_url(url: &str) -> Result<()> {
    let url = url.trim();
    anyhow::ensure!(!url.to_ascii_lowercase().starts_with("https://"),
        "this device is on a Regtest session, so the gateway needs the plain http:// Zakura JSON-RPC URL from `ths endpoints` (usually http://127.0.0.1:18232). \
         {url} is a TLS server such as testnet lightwalletd; to use testnet, leave this session and join a testnet session");
    anyhow::ensure!(url.starts_with("http://"), "the Regtest chain RPC URL must start with http://, for example http://127.0.0.1:18232 from `ths endpoints`");
    Ok(())
}

/// Checks a Regtest gateway URL against `getblockchaininfo`.
pub fn check_regtest_rpc(url: &str) -> Result<serde_json::Value> {
    regtest_rpc_url(url)?;
    let info = zcash_offline_core::rpc::Rpc::new(url.trim()).call("getblockchaininfo", json!([])).with_context(|| format!(
        "no Zakura JSON-RPC answered at {url}. Use the rpc URL from `ths endpoints` (usually http://127.0.0.1:18232), not lightwalletd (port 9067), and check that ths start is running"))?;
    anyhow::ensure!(info["blocks"].is_u64() && info["chain"].is_string(), "RPC returned no blockchain information");
    Ok(info)
}

pub fn home_dir(home: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(h) = home {
        return Ok(h);
    }
    let base = std::env::var_os("HOME").ok_or_else(|| anyhow!("set HOME or --home"))?;
    Ok(PathBuf::from(base).join(".ozm"))
}

pub fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    zcash_offline_core::wallet::write_atomic(path, bytes)
}

/// Installs an invite (fleet plus this device's keys) into `home`. Returns a one-line summary.
pub fn join_invite(invite_json: &str, home: &Path) -> Result<String> {
    let inv: Invite = serde_json::from_str(invite_json).context("not an ozm-node invite file")?;
    let me = inv.fleet.by_name(&inv.secret.name)?;
    anyhow::ensure!(*me == inv.secret.public_on(inv.fleet.network)?, "invite keys do not match its fleet");
    if home.join("secret.json").exists() {
        let old: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(home.join("secret.json"))?)?;
        anyhow::ensure!(old.ed25519_sk == inv.secret.ed25519_sk, "{} already holds device {}; use another home", home.display(), old.name);
    }
    std::fs::create_dir_all(home)?;
    std::fs::write(home.join("fleet.json"), serde_json::to_string_pretty(&inv.fleet)?)?;
    write_private(&home.join("secret.json"), serde_json::to_string_pretty(&inv.secret)?.as_bytes())?;
    let label = |d: &zcash_offline_core::DevicePublic| if d.is_gateway() { format!("{} (gateway)", d.name) } else { d.name.clone() };
    Ok(format!(
        "Joined fleet '{}' as {}. Devices: {}",
        inv.fleet.name,
        label(me),
        inv.fleet.devices.iter().map(label).collect::<Vec<_>>().join(", ")
    ))
}

/// Moves this device's files to `<home>-backup-<UTC time>` beside `home` and returns that path, so the
/// device can start without an invite or join another one. Nothing is deleted. Session exports under
/// `home/sessions` move to the sibling `sessions` folder first, so their invites stay reachable.
pub fn leave_home(home: &Path) -> Result<PathBuf> {
    anyhow::ensure!(std::fs::read_dir(home).is_ok_and(|mut d| d.any(|e| e.is_ok_and(|e| e.file_name() != "sessions"))),
        "this device has not joined a session");
    let old_sessions = home.join("sessions");
    if old_sessions.is_dir() {
        let sessions = home.with_file_name("sessions");
        std::fs::create_dir_all(&sessions)?;
        for e in std::fs::read_dir(&old_sessions)? {
            let e = e?;
            let to = sessions.join(e.file_name());
            if !to.exists() { std::fs::rename(e.path(), to)?; }
        }
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    // Civil date from days since 1970 (Howard Hinnant's algorithm), UTC.
    let z = (secs / 86_400) as i64 + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (day, month) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let year = yoe + z.div_euclid(146_097) * 400 + i64::from(month <= 2);
    let t = secs % 86_400;
    let mut name = home.file_name().ok_or_else(|| anyhow!("home has no folder name"))?.to_os_string();
    name.push(format!("-backup-{year:04}{month:02}{day:02}-{:02}{:02}{:02}", t / 3600, t % 3600 / 60, t % 60));
    let backup = home.with_file_name(name);
    anyhow::ensure!(!backup.exists(), "{} already exists; try again in a second", backup.display());
    std::fs::rename(home, &backup).with_context(|| format!("move {} to {}", home.display(), backup.display()))?;
    Ok(backup)
}

/// Creates keys on this device, once. No keys or wallet are replaced on restart.
pub fn join_open(home: &Path) -> Result<()> {
    join_open_on(home, Network::Regtest, 0)
}

pub fn join_open_on(home: &Path, network: Network, birthday: u32) -> Result<()> {
    if home.join("secret.json").exists() {
        return Ok(());
    }
    anyhow::ensure!(!home.join("fleet.json").exists() && !home.join("data/wallet.json").exists(),
        "device files exist but secret.json is missing; restore the keys or choose a new home");
    if let Some(kept) = kept_wallet(home, network) {
        // Same keys, address, notes and payment history; the session (gateway pin, mesh store,
        // events, start options) stays behind in the backup.
        let fleet = Fleet::load(&kept.join("fleet.json"))?;
        let secret: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(kept.join("secret.json"))?)?;
        join_invite(&serde_json::to_string(&Invite { fleet, secret })?, home)?;
        for file in ["wallet.json", "payments.json"] {
            if let Ok(bytes) = std::fs::read(kept.join("data").join(file)) {
                std::fs::create_dir_all(home.join("data"))?;
                write_private(&home.join("data").join(file), &bytes)?;
            }
        }
        return Ok(());
    }
    let mut me = DeviceSecret::generate("", "");
    me.name = me.public()?.peer_id;
    anyhow::ensure!(network != Network::Testnet || birthday == 0 || birthday >= 4_465_026, "testnet birthday must be at or after NU7");
    let fleet = Fleet { name: format!("Open {network} mesh"), network, birthday, servers: zcash_offline_core::fleet::default_servers(), policy: Policy::default(), devices: vec![me.public_on(network)?], open: true, address_proofs: Default::default() };
    join_invite(&serde_json::to_string(&Invite { fleet, secret: me })?, home)?;
    Ok(())
}

/// The newest `<home>-backup-*` folder that holds an invite-free wallet on `network`. Leave keeps
/// whole device folders there, so the device's own wallet outlives sessions and invited joins.
fn kept_wallet(home: &Path, network: Network) -> Option<PathBuf> {
    let prefix = format!("{}-backup-", home.file_name()?.to_str()?);
    let mut backups: Vec<PathBuf> = std::fs::read_dir(home.parent()?).ok()?.filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix)))
        .map(|e| e.path()).collect();
    backups.sort();
    backups.into_iter().rev().find(|b| b.join("secret.json").is_file()
        && Fleet::load(&b.join("fleet.json")).is_ok_and(|f| f.open && f.network == network))
}

/// Starts the mesh, the app and its background loops. Must run inside a tokio runtime.
pub async fn start(o: &RunOpts) -> Result<Arc<App>> {
    let mut creation_tip = None;
    if !o.secret.exists() && o.secret.file_name().is_some_and(|s| s == "secret.json") {
        // A bounded preflight lets online key creation start at the tip without making an offline
        // device wait for a server. The receive address needs no successful chain connection.
        if o.birthday == 0 && o.network == Network::Testnet {
            let servers = zcash_offline_core::fleet::default_servers();
            if let Ok(Ok(source)) = tokio::time::timeout(Duration::from_secs(2), lightwallet::Lightwallet::connect(&servers[..1])).await {
                creation_tip = Some(source.tip);
            }
        }
        join_open_on(o.secret.parent().ok_or_else(|| anyhow!("secret needs a home directory"))?, o.network, o.birthday)?;
    }
    let fleet = Fleet::load(&o.fleet)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&o.secret)?.permissions().mode() & 0o777 != 0o600 {
            std::fs::set_permissions(&o.secret, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    let me: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(&o.secret).context("read secret")?)?;
    let pubme = fleet.by_name(&me.name)?.clone();
    anyhow::ensure!(pubme == me.public_on(fleet.network)?, "secret for {} does not match the fleet file", me.name);
    anyhow::ensure!(fleet.network != Network::Testnet || !o.mine, "--mine is Regtest-only; public testnet cannot generate blocks");
    let rpc = o.rpc.clone().filter(|r| !r.is_empty()).or_else(|| {
        (fleet.network == Network::Testnet && pubme.is_gateway()).then(|| fleet.servers.join(","))
    });
    if o.rpc.as_ref().is_some_and(|r| !r.is_empty()) || o.mine {
        anyhow::ensure!(pubme.is_gateway(), "this invite does not grant the gateway role; ask the organiser for a gateway invite");
    }
    if let Some(url) = rpc.as_deref().filter(|_| fleet.network == Network::Regtest) {
        regtest_rpc_url(url)?;
    }
    std::fs::create_dir_all(&o.data)?;
    let discovery = o.discover.then(|| Discovery { port: o.discovery_port, ..Default::default() });
    let (mesh, mut events) = Mesh::start(MeshConfig {
        peer_id: pubme.peer_id.parse()?,
        ack_signer: Some(me.signing_key()?),
        listen: Some(o.listen),
        dial: o.dial.iter().filter(|d| !d.is_empty()).cloned().collect(),
        mtu: o.mtu,
        frame_delay: Duration::from_millis(o.frame_delay_ms),
        store_path: Some(o.data.join("mesh-store.json")),
        discovery,
        ..Default::default()
    })
    .await?;
    let app = App::new(
        app::Config {
            data: o.data.clone(),
            fleet,
            me,
            rpc,
            mine: o.mine,
            retry: Duration::from_secs(o.retry_secs.max(1)),
            faucet_max_zat: parse_zec(&o.faucet_max)?,
        },
        mesh,
    )?;
    {
        let mut st = app.state.lock().unwrap();
        if st.0.birthday == 0 {
            st.0.birthday = if o.birthday > 0 { o.birthday } else { creation_tip.map_or(0, |tip| tip.saturating_sub(100).max(4_465_026)) };
        }
        st.0.save(&o.data.join("wallet.json"))?;
    }
    if o.ble {
        match ozm_ble::Ble::start(app.mesh.clone(), [0; 4]).await {
            Ok(ble) => { let _ = app.ble.set(ble); }
            Err(e) => { app.mesh.shutdown(); return Err(e); }
        }
    }
    app.event("started", json!({"role": app.cfg.me.role, "peer_id": pubme.peer_id, "online": app.cfg.rpc.is_some(), "discover": o.discover, "version": env!("CARGO_PKG_VERSION")}));

    let a = app.clone();
    tokio::spawn(async move {
        let handlers = Arc::new(tokio::sync::Semaphore::new(16));
        while let Some(ev) = events.recv().await {
            let Ok(permit) = handlers.clone().acquire_owned().await else { break };
            let a = a.clone();
            tokio::spawn(async move { let _permit = permit; a.handle_mesh_event(ev).await });
        }
    });
    let a = app.clone();
    let auto_sync = o.auto_sync_secs;
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(1));
        let mut ticks = 0u64;
        loop {
            t.tick().await;
            if a.mesh.is_shutdown() { break; }
            ticks += 1;
            if ticks % 3600 == 1 {
                let _ = a.announce().await;
            }
            if ticks % 5 == 1 {
                if let Err(e) = a.gateway_tick().await { a.event("gateway-watch-error", json!({"error": format!("{e:#}")})); }
            }
            a.retry_tick().await;
            if auto_sync > 0 {
                if let Err(e) = a.auto_prepare_tick().await { a.event("auto-prepare-error", json!({"error": format!("{e:#}")})); }
            }
            if auto_sync > 0 && ticks % 5 == 3 && a.gateway_options().0.is_none() && !a.mesh.neighbors().is_empty() {
                a.fallback_sync(auto_sync).await;
            }
        }
    });

    app.start_sync();
    Ok(app)
}

/// The local HTTP API and web UI.
pub fn router(app: Arc<App>, debug_api: bool) -> Router {
    let token = DeviceSecret::generate("api", "").ed25519_sk;
    let html = include_str!("ui.html").replace("__OZM_API_TOKEN__", &token);
    let router = Router::new()
        .route("/", get(move || async { Html(html) }))
        .route("/api/status", get(|State(a): State<Arc<App>>| async move { Json(a.status()) }))
        .route("/api/fleet", get(|State(a): State<Arc<App>>| async move { Json(a.cfg.fleet.clone()) }))
        .route(
            "/api/payments",
            get(|State(a): State<Arc<App>>| async move {
                let st = a.state.lock().unwrap();
                Json(json!({"payments": st.1.payments.values().collect::<Vec<_>>(), "refusals": st.1.refusals, "gateway": st.1.gateway.values().collect::<Vec<_>>()}))
            }),
        )
        .route(
            "/api/events",
            get(|State(a): State<Arc<App>>| async move {
                let s = std::fs::read_to_string(a.cfg.data.join("events.jsonl")).unwrap_or_default();
                let v: Vec<serde_json::Value> = s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
                Json(v)
            }),
        )
        .route("/api/pay", post(pay))
        .route("/api/prepare", post(|State(a): State<Arc<App>>, Json(r): Json<FundReq>| async move {
            let amount = parse_zec(&r.amount).map_err(err)?;
            a.prepare_offline(r.notes, amount).await.map(Json).map_err(err)
        }))
        .route("/api/max", get(|State(a): State<Arc<App>>| async move { a.max_payment().map(Json).map_err(err) }))
        .route("/api/sync", post(|State(a): State<Arc<App>>| async move { a.sync().await.map(Json).map_err(err) }))
        .route(
            "/api/fund",
            post(|State(a): State<Arc<App>>, Json(r): Json<FundReq>| async move {
                let amount = parse_zec(&r.amount).map_err(err)?;
                a.request_funds(r.notes, amount).await.map(|gw| Json(json!({"requested_from": gw}))).map_err(err)
            }),
        );
    let router = if debug_api {
        router
            .route(
                "/api/debug/tampered",
                post(|State(a): State<Arc<App>>, Json(r): Json<TamperReq>| async move {
                    let amount = parse_zec(&r.amount).map_err(err)?;
                    a.pay_tampered(&r.to, amount, &r.tamper).await.map(|p| Json(serde_json::to_value(p).unwrap())).map_err(err)
                }),
            )
            .route(
                "/api/debug/resend",
                post(|State(a): State<Arc<App>>, Json(r): Json<serde_json::Value>| async move {
                    a.resend(r["txid"].as_str().unwrap_or_default()).await.map(|_| Json(json!({"ok": true}))).map_err(err)
                }),
            )
    } else {
        router
    };
    router.with_state(app).layer(middleware::from_fn(move |req: Request<Body>, next: Next| {
        let token = token.clone();
        async move {
            let host = req.headers().get("host").and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<axum::http::uri::Authority>().ok());
            if !host.is_some_and(|h| matches!(h.host(), "127.0.0.1" | "localhost" | "[::1]")) {
                return (StatusCode::FORBIDDEN, Json(json!({"error": "loopback Host required"}))).into_response();
            }
            if req.uri().path() != "/" && req.headers().get("x-ozm-token").and_then(|v| v.to_str().ok()) != Some(token.as_str()) {
                return (StatusCode::UNAUTHORIZED, Json(json!({"error": "API token required"}))).into_response();
            }
            let mut response = next.run(req).await;
            response.headers_mut().insert("cache-control", "no-store".parse().unwrap());
            response.headers_mut().insert("content-security-policy", "frame-ancestors 'none'; base-uri 'none'; object-src 'none'".parse().unwrap());
            response.headers_mut().insert("x-content-type-options", "nosniff".parse().unwrap());
            response
        }
    }))
}

#[derive(Deserialize)]
struct FundReq {
    #[serde(default = "five")]
    notes: u32,
    amount: String,
}
fn five() -> u32 {
    5
}

#[derive(Deserialize)]
struct TamperReq {
    to: String,
    amount: String,
    tamper: String,
}

#[derive(Deserialize)]
struct PayReq {
    to: String,
    amount: String,
}

async fn pay(State(a): State<Arc<App>>, Json(r): Json<PayReq>) -> Result<Json<serde_json::Value>, Response> {
    let amount = parse_zec(&r.amount).map_err(err)?;
    let p = a.pay(&r.to, amount).await.map_err(err)?;
    Ok(Json(serde_json::to_value(p).unwrap()))
}

pub fn err(e: anyhow::Error) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("{e:#}")}))).into_response()
}
