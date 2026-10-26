//! `ozm-node`: one device in an offline Zcash mesh.
//!
//! `fleet-init` creates keys; `run` starts the daemon (mesh + local HTTP API);
//! `pay`, `sync`, `status`, `payments` and `events` talk to a running daemon.

#![allow(clippy::result_large_err)] // axum handlers return Response as the error type

mod app;

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use ozm_mesh::{Mesh, MeshConfig};
use serde::Deserialize;
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use zcash_offline_core::{fleet::generate_fleet, parse_zec, DeviceSecret, Fleet, Policy};

#[derive(Parser)]
#[command(name = "ozm-node", version, about = "Offline Zcash payments over a mesh")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a fleet: fleet.json (public) and one <name>.secret.json per device.
    FleetInit {
        #[arg(long)]
        out: PathBuf,
        /// NAME:ROLE, repeatable (roles: payer, payee, relay, gateway; combine with +).
        #[arg(long = "device", required = true)]
        devices: Vec<String>,
        #[arg(long, default_value = "ozm-fleet")]
        name: String,
    },
    /// Run the node daemon.
    Run {
        /// Directory for wallet, payments, mesh store and events.
        #[arg(long, env = "OZM_DATA")]
        data: PathBuf,
        #[arg(long, env = "OZM_FLEET")]
        fleet: PathBuf,
        #[arg(long, env = "OZM_SECRET")]
        secret: PathBuf,
        /// Mesh TCP listen address.
        #[arg(long, env = "OZM_LISTEN", default_value = "0.0.0.0:7777")]
        listen: SocketAddr,
        /// Mesh neighbours to dial (host:port), repeatable or comma-separated.
        #[arg(long, env = "OZM_DIAL", value_delimiter = ',')]
        dial: Vec<String>,
        /// Zakura JSON-RPC URL. Only online devices (gateway, or a wallet while syncing) set this.
        #[arg(long, env = "OZM_RPC")]
        rpc: Option<String>,
        /// Gateway: mine a block after each submission (Regtest only).
        #[arg(long, env = "OZM_MINE")]
        mine: bool,
        /// Local HTTP API and UI.
        #[arg(long, env = "OZM_API", default_value = "127.0.0.1:7780")]
        api: SocketAddr,
        /// Emulated BLE: sleep per 512-byte frame, in milliseconds.
        #[arg(long, env = "OZM_FRAME_DELAY_MS", default_value_t = 0)]
        frame_delay_ms: u64,
        #[arg(long, env = "OZM_MTU", default_value_t = 512)]
        mtu: usize,
        /// Base app retry interval in seconds (doubles per attempt).
        #[arg(long, env = "OZM_RETRY_SECS", default_value_t = 30)]
        retry_secs: u64,
        /// Enable test-only endpoints (tampered payments, resend).
        #[arg(long, env = "OZM_DEBUG_API")]
        debug_api: bool,
    },
    /// Sync a wallet directly from Zakura RPC without the daemon (the online step before going offline).
    WalletSync {
        #[arg(long, env = "OZM_DATA")]
        data: PathBuf,
        #[arg(long, env = "OZM_SECRET")]
        secret: PathBuf,
        #[arg(long, env = "OZM_RPC")]
        rpc: String,
    },
    /// Debug: send a deliberately tampered payment (amount, payee, sig, branch, expired).
    Tampered {
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long)]
        tamper: String,
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    /// Debug: re-send a payment as new envelopes.
    Resend {
        #[arg(long)]
        txid: String,
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    /// Ask a running node to build and send a payment.
    Pay {
        #[arg(long)]
        to: String,
        /// Amount in ZEC, e.g. 0.1
        #[arg(long)]
        amount: String,
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    /// Ask a running node to sync its wallet (needs --rpc on that node).
    Sync {
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    Status {
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    Payments {
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
    Events {
        #[arg(long, env = "OZM_API_URL", default_value = "http://127.0.0.1:7780")]
        api: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,ozm_mesh=info".into()))
        .init();
    match Cli::parse().cmd {
        Cmd::FleetInit { out, devices, name } => fleet_init(&out, &devices, &name),
        Cmd::Run { data, fleet, secret, listen, dial, rpc, mine, api, frame_delay_ms, mtu, retry_secs, debug_api } => {
            run(data, fleet, secret, listen, dial, rpc, mine, api, frame_delay_ms, mtu, retry_secs, debug_api).await
        }
        Cmd::WalletSync { data, secret, rpc } => {
            let me: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(&secret)?)?;
            std::fs::create_dir_all(&data)?;
            let path = data.join("wallet.json");
            let mut w = zcash_offline_core::wallet::WalletState::load(&path)?;
            let rep = tokio::task::spawn_blocking(move || {
                let r = w.sync(&zcash_offline_core::rpc::Rpc::new(&rpc), &me.fvk()?);
                r.and_then(|rep| w.save(&path).map(|_| (rep, w.balance())))
            })
            .await??;
            println!("{}", json!({"tip": rep.0.tip, "new_notes": rep.0.new_notes, "spendable": rep.1 .0, "locked": rep.1 .1}));
            Ok(())
        }
        Cmd::Tampered { to, amount, tamper, api } => client(&api, "POST", "/api/debug/tampered", Some(json!({"to": to, "amount": amount, "tamper": tamper}))),
        Cmd::Resend { txid, api } => client(&api, "POST", "/api/debug/resend", Some(json!({"txid": txid}))),
        Cmd::Pay { to, amount, api } => client(&api, "POST", "/api/pay", Some(json!({"to": to, "amount": amount}))),
        Cmd::Sync { api } => client(&api, "POST", "/api/sync", None),
        Cmd::Status { api } => client(&api, "GET", "/api/status", None),
        Cmd::Payments { api } => client(&api, "GET", "/api/payments", None),
        Cmd::Events { api } => client(&api, "GET", "/api/events", None),
    }
}

fn fleet_init(out: &PathBuf, devices: &[String], name: &str) -> Result<()> {
    let devs = devices
        .iter()
        .map(|d| d.split_once(':').map(|(n, r)| (n.to_string(), r.to_string())).ok_or_else(|| anyhow!("expected NAME:ROLE, got {d}")))
        .collect::<Result<Vec<_>>>()?;
    let (fleet, secrets) = generate_fleet(name, &devs, Policy::default())?;
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("fleet.json"), serde_json::to_string_pretty(&fleet)?)?;
    for s in secrets {
        let p = out.join(format!("{}.secret.json", s.name));
        std::fs::write(&p, serde_json::to_string_pretty(&s)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    for d in &fleet.devices {
        println!("{:<6} {:<14} peer {}  {}", d.name, d.role, d.peer_id, d.address);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run(
    data: PathBuf,
    fleet: PathBuf,
    secret: PathBuf,
    listen: SocketAddr,
    dial: Vec<String>,
    rpc: Option<String>,
    mine: bool,
    api: SocketAddr,
    frame_delay_ms: u64,
    mtu: usize,
    retry_secs: u64,
    debug_api: bool,
) -> Result<()> {
    let fleet = Fleet::load(&fleet)?;
    let me: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(&secret).context("read secret")?)?;
    let pubme = fleet.by_name(&me.name)?.clone();
    anyhow::ensure!(pubme == me.public()?, "secret for {} does not match the fleet file", me.name);
    std::fs::create_dir_all(&data)?;
    let (mesh, mut events) = Mesh::start(MeshConfig {
        peer_id: pubme.peer_id.parse()?,
        listen: Some(listen),
        dial: dial.into_iter().filter(|d| !d.is_empty()).collect(),
        mtu,
        frame_delay: Duration::from_millis(frame_delay_ms),
        store_path: Some(data.join("mesh-store.json")),
        ..Default::default()
    })
    .await?;
    let app = app::App::new(
        app::Config { data, fleet, me, rpc: rpc.filter(|r| !r.is_empty()), mine, retry: Duration::from_secs(retry_secs.max(1)) },
        mesh,
    )?;
    app.event("started", json!({"role": app.cfg.me.role, "peer_id": pubme.peer_id, "online": app.cfg.rpc.is_some()}));

    let a = app.clone();
    tokio::spawn(async move {
        while let Some(ev) = events.recv().await {
            let a = a.clone();
            // Handle each message in its own task so a slow gateway submit never blocks relaying.
            tokio::spawn(async move { a.handle_mesh_event(ev).await });
        }
    });
    let a = app.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(Duration::from_secs(1));
        loop {
            t.tick().await;
            a.retry_tick().await;
        }
    });

    let router = Router::new()
        .route("/", get(|| async { Html(include_str!("ui.html")) }))
        .route("/api/status", get(|State(a): State<Arc<app::App>>| async move { Json(a.status()) }))
        .route("/api/fleet", get(|State(a): State<Arc<app::App>>| async move { Json(a.cfg.fleet.clone()) }))
        .route(
            "/api/payments",
            get(|State(a): State<Arc<app::App>>| async move {
                let st = a.state.lock().unwrap();
                Json(json!({"payments": st.1.payments.values().collect::<Vec<_>>(), "refusals": st.1.refusals, "gateway": st.1.gateway.values().collect::<Vec<_>>()}))
            }),
        )
        .route(
            "/api/events",
            get(|State(a): State<Arc<app::App>>| async move {
                let s = std::fs::read_to_string(a.cfg.data.join("events.jsonl")).unwrap_or_default();
                let v: Vec<serde_json::Value> = s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
                Json(v)
            }),
        )
        .route("/api/pay", post(pay))
        .route("/api/sync", post(|State(a): State<Arc<app::App>>| async move { a.sync().await.map(Json).map_err(err) }));
    let router = if debug_api {
        router
            .route(
                "/api/debug/tampered",
                post(|State(a): State<Arc<app::App>>, Json(r): Json<TamperReq>| async move {
                    let amount = parse_zec(&r.amount).map_err(err)?;
                    a.pay_tampered(&r.to, amount, &r.tamper).await.map(|p| Json(serde_json::to_value(p).unwrap())).map_err(err)
                }),
            )
            .route(
                "/api/debug/resend",
                post(|State(a): State<Arc<app::App>>, Json(r): Json<serde_json::Value>| async move {
                    a.resend(r["txid"].as_str().unwrap_or_default()).await.map(|_| Json(json!({"ok": true}))).map_err(err)
                }),
            )
    } else {
        router
    };
    let router = router.with_state(app.clone());
    let listener = tokio::net::TcpListener::bind(api).await.with_context(|| format!("bind API {api}"))?;
    tracing::info!("API and UI on http://{api}");
    axum::serve(listener, router).await?;
    Ok(())
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

async fn pay(State(a): State<Arc<app::App>>, Json(r): Json<PayReq>) -> Result<Json<serde_json::Value>, Response> {
    let amount = parse_zec(&r.amount).map_err(err)?;
    let p = a.pay(&r.to, amount).await.map_err(err)?;
    Ok(Json(serde_json::to_value(p).unwrap()))
}

fn err(e: anyhow::Error) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"error": format!("{e:#}")}))).into_response()
}

fn client(api: &str, method: &str, path: &str, body: Option<serde_json::Value>) -> Result<()> {
    let url = format!("{}{}", api.trim_end_matches('/'), path);
    let req = ureq::request(method, &url).timeout(Duration::from_secs(300));
    let resp = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    let (ok, v): (bool, serde_json::Value) = match resp {
        Ok(r) => (true, r.into_json()?),
        Err(ureq::Error::Status(_, r)) => (false, r.into_json()?),
        Err(e) => return Err(anyhow!("{url}: {e}")),
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}
