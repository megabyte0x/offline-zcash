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
use zcash_offline_core::{fleet::{generate_fleet_on, Network}, parse_zec, DeviceSecret, Fleet, Policy};

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
    #[arg(long, env = "OZM_NETWORK", default_value = "regtest")]
    pub network: Network,
    /// First scan height; 0 selects 1 on Regtest and 4465026 on testnet.
    #[arg(long, env = "OZM_BIRTHDAY", default_value_t = 0)]
    pub birthday: u32,
    /// Mesh TCP listen address.
    #[arg(long, env = "OZM_LISTEN", default_value = "0.0.0.0:7777")]
    pub listen: SocketAddr,
    /// Mesh neighbours to dial (host:port), repeatable or comma-separated.
    #[arg(long, env = "OZM_DIAL", value_delimiter = ',')]
    pub dial: Vec<String>,
    /// Find fleet devices on the local network (UDP beacons) and connect to them.
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
            network: Network::Regtest,
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
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub birthday: u32,
    /// NAME:ROLE, with roles combined using +.
    pub devices: Vec<String>,
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
        let (n, r) = d.split_once(':').ok_or_else(|| anyhow!("expected NAME:ROLE, got {d}"))?;
        anyhow::ensure!(!n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "device name must be 1–64 letters, digits, - or _: {n}");
        anyhow::ensure!(r.split('+').all(|r| matches!(r, "payer" | "payee" | "relay" | "gateway")), "unknown role in {d}");
        Ok((n.to_string(), r.to_string()))
    }).collect::<Result<Vec<_>>>()?;
    for (i, (n, _)) in devs.iter().enumerate() {
        anyhow::ensure!(!devs[..i].iter().any(|(other, _)| other.eq_ignore_ascii_case(n)), "duplicate device name: {n}");
    }
    anyhow::ensure!(!o.out.join("fleet.json").exists(), "folder already contains a fleet; choose a new folder");
    for (n, _) in &devs {
        for suffix in ["secret", "invite"] {
            anyhow::ensure!(!o.out.join(format!("{n}.{suffix}.json")).exists(), "folder already contains keys for {n}; choose a new folder");
        }
    }
    let birthday = if o.birthday == 0 { match o.network { Network::Regtest => 1, Network::Testnet => 4_465_026 } } else { o.birthday };
    let (fleet, secrets) = generate_fleet_on(&o.name, &devs, Policy::default(), o.network, birthday)?;
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
    Ok(format!(
        "Joined fleet '{}' as {} ({}). Devices: {}",
        inv.fleet.name,
        inv.secret.name,
        inv.secret.role,
        inv.fleet.devices.iter().map(|d| format!("{} ({})", d.name, d.role)).collect::<Vec<_>>().join(", ")
    ))
}

/// Creates keys on this device, once. No keys or wallet are replaced on restart.
pub fn join_open(home: &Path) -> Result<()> {
    join_open_on(home, Network::Regtest, 1)
}

pub fn join_open_on(home: &Path, network: Network, birthday: u32) -> Result<()> {
    if home.join("secret.json").exists() {
        return Ok(());
    }
    anyhow::ensure!(!home.join("fleet.json").exists() && !home.join("data/wallet.json").exists(),
        "device files exist but secret.json is missing; restore the keys or choose a new home");
    let mut me = DeviceSecret::generate("", "payer+payee+relay");
    me.name = me.public()?.peer_id;
    let birthday = if birthday == 0 { match network { Network::Regtest => 1, Network::Testnet => 4_465_026 } } else { birthday };
    anyhow::ensure!(network != Network::Testnet || birthday >= 4_465_026, "testnet birthday must be at or after NU7");
    let fleet = Fleet { name: format!("Open {network} mesh"), network, birthday, servers: zcash_offline_core::fleet::default_servers(), policy: Policy::default(), devices: vec![me.public_on(network)?], open: true, address_proofs: Default::default() };
    join_invite(&serde_json::to_string(&Invite { fleet, secret: me })?, home)?;
    Ok(())
}

/// Starts the mesh, the app and its background loops. Must run inside a tokio runtime.
pub async fn start(o: &RunOpts) -> Result<Arc<App>> {
    if !o.secret.exists() && o.secret.file_name().is_some_and(|s| s == "secret.json") {
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
        (fleet.network == Network::Testnet && pubme.role.split('+').any(|r| r == "gateway")).then(|| fleet.servers.join(","))
    });
    if o.rpc.as_ref().is_some_and(|r| !r.is_empty()) || o.mine {
        anyhow::ensure!(pubme.role.split('+').any(|r| r == "gateway"), "this invite does not grant the gateway role; ask the organiser for a gateway invite");
    }
    std::fs::create_dir_all(&o.data)?;
    let tag = if pubme.role.split('+').any(|r| r == "gateway") { [0; 8] } else { fleet.discovery_tag() };
    let discovery = o.discover.then(|| Discovery { port: o.discovery_port, tag, ..Default::default() });
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
    if o.ble {
        let hint = tag[..4].try_into().expect("four bytes");
        match ozm_ble::Ble::start(app.mesh.clone(), hint).await {
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
            if auto_sync > 0 && ticks % 5 == 3 && a.gateway_options().0.is_none() && !a.mesh.neighbors().is_empty() {
                a.fallback_sync(auto_sync).await;
            }
        }
    });

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
