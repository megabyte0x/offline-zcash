//! `ozm-node`: one device in an offline Zcash mesh.
//!
//! `fleet-init` creates keys and invites; `join` installs an invite; `start`
//! runs the daemon with home-directory defaults and LAN discovery; `run` is the
//! fully explicit form. `pay`, `sync`, `fund`, `status`, `payments` and
//! `events` talk to a running daemon over its local HTTP API.


use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use ozm_node::{home_dir, join_invite, router, start, FleetInitOptions, RunOpts};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use zcash_offline_core::DeviceSecret;

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
        /// NAME, or NAME:gateway for the gateway; repeatable, at least one gateway. Every device pays, receives and relays.
        #[arg(long = "device", required = true)]
        devices: Vec<String>,
        #[arg(long, default_value = "ozm-fleet")]
        name: String,
        /// testnet, or regtest for a local ths chain (developers).
        #[arg(long, default_value = "testnet")]
        network: zcash_offline_core::fleet::Network,
        #[arg(long, default_value_t = 0)]
        birthday: u32,
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
        /// Network for a new invite-free identity: testnet, or regtest for a local ths chain.
        #[arg(long, env = "OZM_NETWORK", default_value = "testnet")]
        network: zcash_offline_core::fleet::Network,
        #[arg(long, env = "OZM_BIRTHDAY", default_value_t = 0)]
        birthday: u32,
        /// Gateway on Regtest: mine a block after each submission and serve test funds.
        #[arg(long, env = "OZM_MINE")]
        mine: bool,
        /// Enable Bluetooth LE scanning and advertising (Linux / BlueZ only).
        #[arg(long, env = "OZM_BLE")]
        ble: bool,
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
        /// Recipient's Regtest unified Zcash address.
        #[arg(long)]
        to: String,
        /// Amount in ZEC, e.g. 0.1
        #[arg(long)]
        amount: String,
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Quote the largest payment and its exact ZIP 317 builder fee without spending.
    Max {
        #[arg(long, env = "OZM_API_URL", default_value = API_URL)]
        api: String,
    },
    /// Prepare for offline: split one mined note, then wait for the payment's mined state and sync.
    Prepare {
        #[arg(long, default_value_t = 10)]
        notes: u32,
        #[arg(long, default_value = "0.0101")]
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,ozm_mesh=info".into()))
        .init();
    match Cli::parse().cmd {
        Cmd::FleetInit { out, devices, name, network, birthday } => {
            let result = ozm_node::fleet_init(&FleetInitOptions { out, devices, name, network, birthday })?;
            for d in &result.devices {
                println!("{:<8} {:<8} peer {}  {}", d.name, d.role, d.peer_id, d.address);
            }
            println!("\nGive each person their <name>.invite.json (it contains their private keys).");
            Ok(())
        }
        Cmd::Join { invite, home } => join(&invite, home),
        Cmd::Start { home, dial, rpc, network, birthday, mine, ble, api, listen, no_open } => {
            let home = home_dir(home)?;
            run(RunOpts { listen, dial, rpc, network, birthday, mine, ble, api, open: !no_open, ..RunOpts::for_home(&home) })
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
        Cmd::Max { api } => client(&api, "GET", "/api/max", None),
        Cmd::Prepare { notes, amount, api } => {
            let payment = client_value(&api, "POST", "/api/prepare", Some(json!({"notes": notes, "amount": amount})))?;
            let txid = payment["txid"].as_str().ok_or_else(|| anyhow!("split returned no txid"))?;
            println!("Split {txid}; waiting for gateway-reported mined state…");
            loop {
                let list = client_value(&api, "GET", "/api/payments", None)?;
                let p = list["payments"].as_array().and_then(|ps| ps.iter().find(|p| p["txid"] == txid))
                    .ok_or_else(|| anyhow!("split is missing from payment history"))?;
                let state = p["state"].as_str().unwrap_or_default();
                if state == "mined" { break; }
                anyhow::ensure!(!matches!(state, "expired" | "conflict" | "gateway-rejected"), "split {state}; inspect payment history before retrying");
                std::thread::sleep(Duration::from_secs(5));
            }
            client(&api, "POST", "/api/sync", None)
        },
        Cmd::Sync { api } => client(&api, "POST", "/api/sync", None),
        Cmd::Fund { notes, amount, api } => client(&api, "POST", "/api/fund", Some(json!({"notes": notes, "amount": amount}))),
        Cmd::Status { api } => client(&api, "GET", "/api/status", None),
        Cmd::Payments { api } => client(&api, "GET", "/api/payments", None),
        Cmd::Events { api } => client(&api, "GET", "/api/events", None),
        Cmd::Tampered { to, amount, tamper, api } => client(&api, "POST", "/api/debug/tampered", Some(json!({"to": to, "amount": amount, "tamper": tamper}))),
        Cmd::Resend { txid, api } => client(&api, "POST", "/api/debug/resend", Some(json!({"txid": txid}))),
    }
}

fn join(invite: &PathBuf, home: Option<PathBuf>) -> Result<()> {
    let text = std::fs::read_to_string(invite).with_context(|| format!("read {}", invite.display()))?;
    let home = home_dir(home)?;
    println!("{}", join_invite(&text, &home)?);
    println!("Next: ozm-node start");
    Ok(())
}

async fn run(o: RunOpts) -> Result<()> {
    // The API token is served to any client that sends a loopback Host header.
    anyhow::ensure!(o.api.ip().is_loopback() || std::env::var("OZM_API_REMOTE").as_deref() == Ok("1"),
        "--api {} is not a loopback address; anyone who can reach it can spend this wallet (set OZM_API_REMOTE=1 only inside an isolated container)", o.api);
    let app = start(&o).await?;
    let router = router(app.clone(), o.debug_api);
    let listener = tokio::net::TcpListener::bind(o.api).await.with_context(|| format!("bind API {}", o.api))?;
    let url = format!("http://{}", o.api).replace("0.0.0.0", "127.0.0.1");
    let gateway = if app.cfg.fleet.by_name(app.name()).is_ok_and(|d| d.is_gateway()) { " (gateway)" } else { "" };
    println!("ozm-node {} — {}{gateway} — open {url}", env!("CARGO_PKG_VERSION"), app.name());
    if o.open {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(&url).spawn();
    }
    axum::serve(listener, router).await?;
    Ok(())
}

fn client(api: &str, method: &str, path: &str, body: Option<serde_json::Value>) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&client_value(api, method, path, body)?)?);
    Ok(())
}

fn client_value(api: &str, method: &str, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value> {
    let url = format!("{}{}", api.trim_end_matches('/'), path);
    // The local UI bootstraps the per-run token; same-origin policy prevents remote sites reading it.
    let html = ureq::get(&format!("{}/", api.trim_end_matches('/'))).call()?.into_string()?;
    let token = html.split("name=\"ozm-api-token\" content=\"").nth(1)
        .and_then(|s| s.split('"').next()).ok_or_else(|| anyhow!("node did not provide an API token"))?;
    let req = ureq::request(method, &url).set("x-ozm-token", token).timeout(Duration::from_secs(300));
    let resp = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    let (ok, v): (bool, serde_json::Value) = match resp {
        Ok(r) => (true, r.into_json()?),
        Err(ureq::Error::Status(_, r)) => (false, r.into_json()?),
        Err(e) => return Err(anyhow!("{url}: {e} (is `ozm-node start` running?)")),
    };
    anyhow::ensure!(ok, "{}", v["error"].as_str().unwrap_or("request failed"));
    Ok(v)
}
