//! `ozm-node`: one device in an offline Zcash mesh.
//!
//! `fleet-init` creates keys and invites; `join` installs an invite; `start`
//! runs the daemon with home-directory defaults and LAN discovery; `run` is the
//! fully explicit form. `pay`, `sync`, `fund`, `status`, `payments` and
//! `events` talk to a running daemon over its local HTTP API.
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
use clap::{Args, Parser, Subcommand};
use ozm_mesh::{Discovery, Mesh, MeshConfig};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use zcash_offline_core::{fleet::generate_fleet, parse_zec, DeviceSecret, Fleet, Policy};

#[derive(Parser)]
#[command(name = "ozm-node", version, about = "Offline Zcash payments over a mesh")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

const API_URL: &str = "http://127.0.0.1:7780";

#[derive(Subcommand)]
enum Cmd {
    /// Generate a fleet: fleet.json (public), and per device <name>.secret.json and <name>.invite.json.
    FleetInit {
        #[arg(long)]
        out: PathBuf,
        /// NAME:ROLE, repeatable (roles: payer, payee, relay, gateway; combine with +).
        #[arg(long = "device", required = true)]
        devices: Vec<String>,
        #[arg(long, default_value = "ozm-fleet")]
        name: String,
    },
    /// Install an invite file (fleet + this device's keys) into the home directory.
    Join {
        invite: PathBuf,
        #[arg(long, env = "OZM_HOME")]
        home: Option<PathBuf>,
    },
    /// Start this device from the home directory: LAN discovery on, UI on http://127.0.0.1:7780.
    Start {
        #[arg(long, env = "OZM_HOME")]
        home: Option<PathBuf>,
        /// Extra mesh neighbours to dial (host:port), e.g. a gateway reachable over the internet.
        #[arg(long, env = "OZM_DIAL", value_delimiter = ',')]
        dial: Vec<String>,
        /// Zakura JSON-RPC URL (gateway only).
        #[arg(long, env = "OZM_RPC")]
        rpc: Option<String>,
        /// Gateway on Regtest: mine a block after each submission and serve test funds.
        #[arg(long, env = "OZM_MINE")]
        mine: bool,
        #[arg(long, env = "OZM_API", default_value = "127.0.0.1:7780")]
        api: SocketAddr,
        #[arg(long, env = "OZM_LISTEN", default_value = "0.0.0.0:7777")]
        listen: SocketAddr,
        /// Do not open the UI in a browser.
        #[arg(long)]
        no_open: bool,
    },
    /// Run the node daemon with explicit paths.
    Run(RunOpts),
    /// Sync a wallet directly from Zakura RPC without the daemon (the online step before going offline).
    WalletSync {
        #[arg(long, env = "OZM_DATA")]
        data: PathBuf,
        #[arg(long, env = "OZM_SECRET")]
        secret: PathBuf,
        #[arg(long, env = "OZM_RPC")]
        rpc: String,
    },
    /// Ask a running node to build and send a payment.
    Pay {
        #[arg(long)]
        to: String,
        /// Amount in ZEC, e.g. 0.1
        #[arg(long)]
        amount: String,
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Ask a running node to sync (from RPC if online, else from a gateway over the mesh).
    Sync {
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Ask the gateway for test funds over the mesh (Regtest sessions).
    Fund {
        #[arg(long, default_value_t = 5)]
        notes: u32,
        /// ZEC per note
        #[arg(long, default_value = "0.1")]
        amount: String,
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    Status {
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    Payments {
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    Events {
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Debug: send a deliberately tampered payment (amount, payee, sig, branch, expired).
    Tampered {
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long)]
        tamper: String,
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Debug: re-send a payment as new envelopes.
    Resend {
        #[arg(long)]
        txid: String,
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
}

#[derive(Args, Clone)]
struct RunOpts {
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
    /// Find fleet devices on the local network (UDP beacons) and connect to them.
    #[arg(long, env = "OZM_DISCOVER")]
    discover: bool,
    #[arg(long, env = "OZM_DISCOVERY_PORT", default_value_t = 7778)]
    discovery_port: u16,
    /// Zakura JSON-RPC URL. Only online devices (gateway) set this.
    #[arg(long, env = "OZM_RPC")]
    rpc: Option<String>,
    /// Gateway: mine a block after each submission and serve test funds (Regtest only).
    #[arg(long, env = "OZM_MINE")]
    mine: bool,
    /// Gateway: most test funds served per request, in ZEC (0 disables).
    #[arg(long, env = "OZM_FAUCET_MAX", default_value = "1")]
    faucet_max: String,
    /// Local HTTP API and UI.
    #[arg(long, env = "OZM_API", default_value = "127.0.0.1:7780")]
    api: SocketAddr,
    /// Emulated BLE: sleep per frame, in milliseconds.
    #[arg(long, env = "OZM_FRAME_DELAY_MS", default_value_t = 0)]
    frame_delay_ms: u64,
    #[arg(long, env = "OZM_MTU", default_value_t = 512)]
    mtu: usize,
    /// Base app retry interval in seconds (doubles per attempt).
    #[arg(long, env = "OZM_RETRY_SECS", default_value_t = 30)]
    retry_secs: u64,
    /// Seconds between automatic syncs from the gateway over the mesh (0 disables).
    #[arg(long, env = "OZM_AUTO_SYNC_SECS", default_value_t = 30)]
    auto_sync_secs: u64,
    /// Enable test-only endpoints (tampered payments, resend).
    #[arg(long, env = "OZM_DEBUG_API")]
    debug_api: bool,
    /// Open the UI in the default browser once started.
    #[arg(long)]
    open: bool,
}

#[derive(Serialize, Deserialize)]
struct Invite {
    fleet: Fleet,
    secret: DeviceSecret,
}

fn home_dir(home: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(h) = home {
        return Ok(h);
    }
    let base = std::env::var_os("HOME").ok_or_else(|| anyhow!("set HOME or --home"))?;
    Ok(PathBuf::from(base).join(".ozm"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,ozm_mesh=info".into()))
        .init();
    match Cli::parse().cmd {
        Cmd::FleetInit { out, devices, name } => fleet_init(&out, &devices, &name),
        Cmd::Join { invite, home } => join(&invite, home),
        Cmd::Start { home, dial, rpc, mine, api, listen, no_open } => {
            let home = home_dir(home)?;
            anyhow::ensure!(home.join("secret.json").exists(), "no device in {}; run `ozm-node join <invite.json>` first", home.display());
            run(RunOpts {
                data: home.join("data"),
                fleet: home.join("fleet.json"),
                secret: home.join("secret.json"),
                listen,
                dial,
                discover: true,
                discovery_port: 7778,
                rpc,
                mine,
                faucet_max: "1".into(),
                api,
                frame_delay_ms: 0,
                mtu: 512,
                retry_secs: 30,
                auto_sync_secs: 30,
                debug_api: false,
                open: !no_open,
            })
            .await
        }
        Cmd::Run(o) => run(o).await,
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
        Cmd::Pay { to, amount, api } => client(&api, "POST", "/api/pay", Some(json!({"to": to, "amount": amount}))),
        Cmd::Sync { api } => client(&api, "POST", "/api/sync", None),
        Cmd::Fund { notes, amount, api } => client(&api, "POST", "/api/fund", Some(json!({"notes": notes, "amount": amount}))),
        Cmd::Status { api } => client(&api, "GET", "/api/status", None),
        Cmd::Payments { api } => client(&api, "GET", "/api/payments", None),
        Cmd::Events { api } => client(&api, "GET", "/api/events", None),
        Cmd::Tampered { to, amount, tamper, api } => client(&api, "POST", "/api/debug/tampered", Some(json!({"to": to, "amount": amount, "tamper": tamper}))),
        Cmd::Resend { txid, api } => client(&api, "POST", "/api/debug/resend", Some(json!({"txid": txid}))),
    }
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
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
        write_private(&out.join(format!("{}.secret.json", s.name)), serde_json::to_string_pretty(&s)?.as_bytes())?;
        let invite = Invite { fleet: fleet.clone(), secret: s.clone() };
        write_private(&out.join(format!("{}.invite.json", s.name)), serde_json::to_string_pretty(&invite)?.as_bytes())?;
    }
    for d in &fleet.devices {
        println!("{:<8} {:<14} peer {}  {}", d.name, d.role, d.peer_id, d.address);
    }
    println!("\nGive each person their <name>.invite.json (it contains their private keys).");
    Ok(())
}

fn join(invite: &PathBuf, home: Option<PathBuf>) -> Result<()> {
    let inv: Invite = serde_json::from_str(&std::fs::read_to_string(invite).with_context(|| format!("read {}", invite.display()))?)
        .context("not an ozm-node invite file")?;
    let me = inv.fleet.by_name(&inv.secret.name)?;
    anyhow::ensure!(*me == inv.secret.public()?, "invite keys do not match its fleet");
    let home = home_dir(home)?;
    if home.join("secret.json").exists() {
        let old: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(home.join("secret.json"))?)?;
        anyhow::ensure!(old.ed25519_sk == inv.secret.ed25519_sk, "{} already holds device {}; use another --home", home.display(), old.name);
    }
    std::fs::create_dir_all(&home)?;
    std::fs::write(home.join("fleet.json"), serde_json::to_string_pretty(&inv.fleet)?)?;
    write_private(&home.join("secret.json"), serde_json::to_string_pretty(&inv.secret)?.as_bytes())?;
    println!("Joined fleet '{}' as {} ({}) in {}", inv.fleet.name, inv.secret.name, inv.secret.role, home.display());
    println!("Devices: {}", inv.fleet.devices.iter().map(|d| format!("{} ({})", d.name, d.role)).collect::<Vec<_>>().join(", "));
    println!("Next: ozm-node start");
    Ok(())
}

async fn run(o: RunOpts) -> Result<()> {
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
    let app = app::App::new(
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
            ticks += 1;
            a.retry_tick().await;
            // Offline devices refresh their chain view from a gateway while one is reachable.
            if auto_sync > 0 && ticks % auto_sync == 3 && a.cfg.rpc.is_none() && !a.mesh.neighbors().is_empty() {
                let _ = a.request_mesh_sync(false).await;
            }
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
        .route("/api/sync", post(|State(a): State<Arc<app::App>>| async move { a.sync().await.map(Json).map_err(err) }))
        .route(
            "/api/fund",
            post(|State(a): State<Arc<app::App>>, Json(r): Json<FundReq>| async move {
                let amount = parse_zec(&r.amount).map_err(err)?;
                a.request_funds(r.notes, amount).await.map(|gw| Json(json!({"requested_from": gw}))).map_err(err)
            }),
        );
    let router = if o.debug_api {
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
    let listener = tokio::net::TcpListener::bind(o.api).await.with_context(|| format!("bind API {}", o.api))?;
    let url = format!("http://{}", o.api).replace("0.0.0.0", "127.0.0.1");
    println!("ozm-node {} — {} ({}) — open {url}", env!("CARGO_PKG_VERSION"), app.name(), app.cfg.me.role);
    if o.open {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&url).spawn();
    }
    axum::serve(listener, router).await?;
    Ok(())
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
        Err(e) => return Err(anyhow!("{url}: {e} (is `ozm-node start` running?)")),
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}
