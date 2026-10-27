//! `ozm-node`: one device in an offline Zcash mesh.
//!
//! `fleet-init` creates keys and invites; `join` installs an invite; `start`
//! runs the daemon with home-directory defaults and LAN discovery; `run` is the
//! fully explicit form. `pay`, `sync`, `fund`, `status`, `payments` and
//! `events` talk to a running daemon over its local HTTP API.


use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use ozm_node::{home_dir, join_invite, router, start, write_private, Invite, RunOpts};
use serde_json::json;
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use zcash_offline_core::{fleet::generate_fleet, DeviceSecret, Policy};

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
            run(RunOpts { listen, dial, rpc, mine, api, open: !no_open, ..RunOpts::for_home(&home) })
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
    let text = std::fs::read_to_string(invite).with_context(|| format!("read {}", invite.display()))?;
    let home = home_dir(home)?;
    println!("{}", join_invite(&text, &home)?);
    println!("Next: ozm-node start");
    Ok(())
}

async fn run(o: RunOpts) -> Result<()> {
    let app = start(&o).await?;
    let router = router(app.clone(), o.debug_api);
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
