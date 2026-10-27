//! `ozm-node` as a library: the device (mesh, wallet, payment roles) that both
//! the `ozm-node` binary and the Logos Basecamp module run.
#![allow(clippy::result_large_err)] // axum handlers return Response as the error type

pub mod app;

pub use app::App;

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
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
use zcash_offline_core::{parse_zec, DeviceSecret, Fleet};

#[derive(Args, Clone)]
pub struct RunOpts {
    /// Directory for wallet, payments, mesh store and events.
    #[arg(long, env = "OZM_DATA")]
    pub data: PathBuf,
    #[arg(long, env = "OZM_FLEET")]
    pub fleet: PathBuf,
    #[arg(long, env = "OZM_SECRET")]
    pub secret: PathBuf,
    /// Mesh TCP listen address.
    #[arg(long, env = "OZM_LISTEN", default_value = "0.0.0.0:7777")]
    pub listen: SocketAddr,
    /// Mesh neighbours to dial (host:port), repeatable or comma-separated.
    #[arg(long, env = "OZM_DIAL", value_delimiter = ',')]
    pub dial: Vec<String>,
    /// Find fleet devices on the local network (UDP beacons) and connect to them.
    #[arg(long, env = "OZM_DISCOVER")]
    pub discover: bool,
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
    /// Seconds between automatic syncs from the gateway over the mesh (0 disables).
    #[arg(long, env = "OZM_AUTO_SYNC_SECS", default_value_t = 30)]
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
            listen: "0.0.0.0:7777".parse().unwrap(),
            dial: vec![],
            discover: true,
            discovery_port: 7778,
            rpc: None,
            mine: false,
            faucet_max: "1".into(),
            api: "127.0.0.1:7780".parse().unwrap(),
            frame_delay_ms: 0,
            mtu: 512,
            retry_secs: 30,
            auto_sync_secs: 30,
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

pub fn home_dir(home: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(h) = home {
        return Ok(h);
    }
    let base = std::env::var_os("HOME").ok_or_else(|| anyhow!("set HOME or --home"))?;
    Ok(PathBuf::from(base).join(".ozm"))
}

pub fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Installs an invite (fleet plus this device's keys) into `home`. Returns a one-line summary.
pub fn join_invite(invite_json: &str, home: &Path) -> Result<String> {
    let inv: Invite = serde_json::from_str(invite_json).context("not an ozm-node invite file")?;
    let me = inv.fleet.by_name(&inv.secret.name)?;
    anyhow::ensure!(*me == inv.secret.public()?, "invite keys do not match its fleet");
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

/// Starts the mesh, the app and its background loops. Must run inside a tokio runtime.
pub async fn start(o: &RunOpts) -> Result<Arc<App>> {
    let fleet = Fleet::load(&o.fleet)?;
    let me: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(&o.secret).context("read secret")?)?;
    let pubme = fleet.by_name(&me.name)?.clone();
    anyhow::ensure!(pubme == me.public()?, "secret for {} does not match the fleet file", me.name);
    std::fs::create_dir_all(&o.data)?;
    let discovery = o.discover.then(|| Discovery { port: o.discovery_port, tag: fleet.discovery_tag(), ..Default::default() });
    let (mesh, mut events) = Mesh::start(MeshConfig {
        peer_id: pubme.peer_id.parse()?,
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
            rpc: o.rpc.clone().filter(|r| !r.is_empty()),
            mine: o.mine,
            retry: Duration::from_secs(o.retry_secs.max(1)),
            faucet_max_zat: parse_zec(&o.faucet_max)?,
        },
        mesh,
    )?;
    app.event("started", json!({"role": app.cfg.me.role, "peer_id": pubme.peer_id, "online": app.cfg.rpc.is_some(), "discover": o.discover, "version": env!("CARGO_PKG_VERSION")}));

    let a = app.clone();
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            let a = a.clone();
            // Handle each message in its own task so a slow gateway submit never blocks relaying.
            tokio::spawn(async move { a.handle_mesh_event(ev).await });
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
            a.retry_tick().await;
            // Offline devices refresh their chain view from a gateway while one is reachable.
            if auto_sync > 0 && ticks % auto_sync == 3 && a.cfg.rpc.is_none() && !a.mesh.neighbors().is_empty() {
                let _ = a.request_mesh_sync(false).await;
            }
        }
    });

    Ok(app)
}

/// The local HTTP API and web UI.
pub fn router(app: Arc<App>, debug_api: bool) -> Router {
    let router = Router::new()
        .route("/", get(|| async { Html(include_str!("ui.html")) }))
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
    router.with_state(app)
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
