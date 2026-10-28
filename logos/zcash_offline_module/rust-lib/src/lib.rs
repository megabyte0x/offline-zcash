//! `zcash_offline_module`: the ozm-node device (mesh, wallet, payments and
//! the optional gateway) as a Logos core module.
//!
//! Rust-first: the trait below is the module's contract. Every method returns
//! a JSON string; failures are `{"error": "..."}`. The device lives in
//! `<instance_persistence_path>/ozm`, like `~/.ozm` for the `ozm-node` binary.

/// The module API. Method names are what `logosctl call zcash_offline_module <method>` takes.
pub trait ZcashOfflineModule: Send + 'static {
    /// Installs an invite file's JSON (fleet plus this device's keys). Returns `{"ok":true,"message":...}`.
    fn join(&mut self, invite_json: String) -> String;

    /// Starts the device. Options JSON: listen, dial, rpc, mine, discover, ble (Linux only), discovery_port, api. Existing mesh is reused; explicit rpc/mine update the gateway.
    fn start(&mut self, options_json: String) -> String;

    /// Device status (GET /api/status) plus joined, running and home. Works before join and start.
    fn status(&mut self) -> String;

    /// The fleet file (GET /api/fleet).
    fn fleet(&mut self) -> String;

    /// Builds and sends an offline payment to a Regtest unified address. Returns the payment (POST /api/pay).
    fn pay(&mut self, to: String, amount_zec: String) -> String;

    /// Starts a wallet sync in the background and returns at once; `status` reports `syncing`/`sync_error`
    /// and `node_event` sends `synced` or `sync-error`. Online: from the chain; offline: via a gateway.
    fn sync_now(&mut self) -> String;

    /// Asks a Regtest gateway for `notes` test notes of `amount_zec` each (POST /api/fund).
    fn request_funds(&mut self, notes: i64, amount_zec: String) -> String;

    /// Payments, refusals and gateway submissions (GET /api/payments).
    fn list_payments(&mut self) -> String;

    /// Creates a session folder and per-device invites. Options: out, name, devices (NAME or NAME:gateway), optional join_device.
    fn fleet_init(&mut self, options_json: String) -> String;

    /// Checks that a chain RPC answers getblockchaininfo. Does not start or grant a role.
    fn check_rpc(&mut self, rpc_url: String) -> String;

    /// Asks the desktop to open a folder containing a fleet. UI apps cannot open local file URLs.
    fn open_fleet_folder(&mut self, folder: String) -> String;

    /// Current public-testnet tip from the default TLS lightwalletd servers, for choosing a session birthday.
    fn testnet_tip(&mut self) -> String;

    /// Largest spendable amount and the exact builder fee. Does not lock notes or make a payment.
    fn max_payment(&mut self) -> String;

    /// Splits one mined note into equal self outputs. Wait for mined and sync before going offline.
    fn prepare_offline(&mut self, notes: i64, amount_zec: String) -> String;

    /// Stops the node and moves the device files to a timestamped backup, so the device can join again.
    fn leave(&mut self) -> String;

    /// Auto-starts a device that was already joined, so restarts resume.
    fn on_context_ready(&mut self, _ctx: &RustModuleContext) {}
}

/// Events. `node_event` carries every ozm-node app event (the lines of `events.jsonl`).
pub trait ZcashOfflineModuleEvents {
    fn node_event(&self, event: String, detail_json: String);
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/generated/provider_gen.rs"));

use anyhow::{anyhow, Context as _, Result};
use ozm_node::App;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use zcash_offline_core::{parse_zec, DeviceSecret, Fleet};

/// The module's one tokio runtime; methods block on it from the dispatch thread.
fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("ozm").build().expect("tokio runtime"))
}

/// `start` options. Saved in the home so a restart resumes with the same ones.
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct StartOptions {
    network: Option<zcash_offline_core::fleet::Network>,
    birthday: Option<u32>,
    listen: Option<String>,
    dial: Option<Vec<String>>,
    rpc: Option<String>,
    mine: Option<bool>,
    discover: Option<bool>,
    ble: Option<bool>,
    discovery_port: Option<u16>,
    api: Option<String>,
}

const OPTIONS_FILE: &str = "module-start.json";

#[derive(Default)]
struct Node {
    app: Option<Arc<App>>,
    api: Option<tokio::task::JoinHandle<()>>,
    /// A first testnet scan can take minutes; it must never hold the module's call thread.
    sync: Arc<SyncState>,
}

#[derive(Default)]
struct SyncState {
    running: std::sync::atomic::AtomicBool,
    error: std::sync::Mutex<Option<String>>,
}

fn home() -> Result<PathBuf> {
    let ctx = context().ok_or_else(|| anyhow!("module context not ready yet"))?;
    anyhow::ensure!(!ctx.instance_persistence_path.is_empty(), "host gave no instance persistence path");
    Ok(PathBuf::from(ctx.instance_persistence_path).join("ozm"))
}

fn joined(home: &std::path::Path) -> bool {
    home.join("secret.json").exists() && home.join("fleet.json").exists()
}

fn saved_options(home: &std::path::Path) -> StartOptions {
    std::fs::read(home.join(OPTIONS_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn out(r: Result<Value>) -> String {
    match r {
        Ok(v) => v.to_string(),
        Err(e) => json!({"error": format!("{e:#}")}).to_string(),
    }
}

impl Node {
    fn app(&self) -> Result<&Arc<App>> {
        self.app.as_ref().ok_or_else(|| anyhow!("not started: call start first"))
    }

    fn status_value(&self) -> Result<Value> {
        let home = home()?;
        let mut v = match &self.app {
            Some(a) => a.status(),
            None if joined(&home) => {
                // Not running: name, gateway role and address from the installed files.
                let me: DeviceSecret = serde_json::from_str(&std::fs::read_to_string(home.join("secret.json"))?)?;
                let fleet = Fleet::load(&home.join("fleet.json"))?;
                json!({"name": me.name, "role": if zcash_offline_core::fleet::is_gateway_role(&me.role) { "gateway" } else { "" }, "network": fleet.network, "address": fleet.by_name(&me.name).ok().map(|d| d.address.clone())})
            }
            None => json!({}),
        };
        v["syncing"] = json!(self.sync.running.load(std::sync::atomic::Ordering::SeqCst));
        v["sync_error"] = json!(self.sync.error.lock().unwrap().clone());
        v["joined"] = json!(joined(&home));
        v["running"] = json!(self.app.is_some());
        v["home"] = json!(home.display().to_string());
        v["sessions"] = json!(home.with_file_name("sessions").display().to_string());
        if self.app.is_none() {
            let o = saved_options(&home);
            v["rpc"] = json!(o.rpc);
            v["mine"] = json!(o.mine.unwrap_or(false));
        }
        Ok(v)
    }

    fn start_with(&mut self, o: StartOptions) -> Result<Value> {
        if let Some(app) = &self.app {
            if o.rpc.is_some() || o.mine.is_some() {
                let (rpc, mine) = app.gateway_options();
                let rpc = o.rpc.clone().or(rpc).ok_or_else(|| anyhow!("chain RPC URL is required"))?;
                let mine = o.mine.unwrap_or(mine);
                app.configure_gateway(rpc.clone(), mine)?;
                let h = home()?;
                let mut saved = saved_options(&h);
                saved.rpc = Some(rpc);
                saved.mine = Some(mine);
                std::fs::write(h.join(OPTIONS_FILE), serde_json::to_vec_pretty(&saved)?)?;
            }
            return self.status_value();
        }
        let home = home()?;
        ozm_node::join_open_on(&home, o.network.unwrap_or_default(), o.birthday.unwrap_or(0))?;
        let mut opts = ozm_node::RunOpts::for_home(&home);
        if let Some(l) = &o.listen {
            opts.listen = l.parse().with_context(|| format!("listen {l}"))?;
        }
        if let Some(d) = &o.dial {
            opts.dial = d.clone();
        }
        opts.rpc = o.rpc.clone().filter(|r| !r.is_empty());
        opts.mine = o.mine.unwrap_or(false);
        opts.discover = o.discover.unwrap_or(true);
        opts.ble = o.ble.unwrap_or(false);
        if let Some(p) = o.discovery_port {
            opts.discovery_port = p;
        }
        let api: Option<std::net::SocketAddr> = match o.api.as_deref().filter(|a| !a.is_empty()) {
            Some(a) => Some(a.parse().with_context(|| format!("api {a}"))?),
            None => None,
        };
        // Build the proving key off the call path; the first pay would otherwise wait for it.
        std::thread::spawn(|| {
            zcash_offline_core::builder::proving_key();
        });
        let app = rt().block_on(async {
            // Bind the API first: a failure here must not leave a started mesh behind.
            let listener = match api {
                Some(addr) => Some(tokio::net::TcpListener::bind(addr).await.with_context(|| format!("bind api {addr}"))?),
                None => None,
            };
            let app = ozm_node::start(&opts).await?;
            let api = listener.map(|listener| {
                let router = ozm_node::router(app.clone(), false);
                tokio::spawn(async move {
                    let _ = axum::serve(listener, router).await;
                })
            });
            anyhow::Ok((app, api))
        })?;
        let (app, api) = app;
        self.api = api;
        let _ = app.on_event.set(Box::new(|event: &str, detail: &Value| emit_node_event(event, &detail.to_string())));
        self.app = Some(app);
        let _ = std::fs::write(home.join(OPTIONS_FILE), serde_json::to_vec_pretty(&o)?);
        self.status_value()
    }
}

impl Node {
    fn start_sync(&self) -> Result<Value> {
        use std::sync::atomic::Ordering::SeqCst;
        let app = self.app()?.clone();
        if !self.sync.running.swap(true, SeqCst) {
            *self.sync.error.lock().unwrap() = None;
            let sync = self.sync.clone();
            rt().spawn(async move {
                if let Err(e) = app.sync().await {
                    let error = format!("{e:#}");
                    app.event("sync-error", json!({"error": error}));
                    *sync.error.lock().unwrap() = Some(error);
                }
                sync.running.store(false, SeqCst);
            });
        }
        Ok(json!({"syncing": true}))
    }

    /// Payments wait for the wallet lock a sync holds; refuse instead of blocking the call thread.
    fn not_syncing(&self) -> Result<()> {
        anyhow::ensure!(!self.sync.running.load(std::sync::atomic::Ordering::SeqCst), "wallet sync in progress; try again when it finishes");
        Ok(())
    }
}

impl ZcashOfflineModule for Node {
    fn fleet_init(&mut self, options_json: String) -> String {
        out((|| {
            let mut v: Value = serde_json::from_str(&options_json).context("fleet options")?;
            let join = v.as_object_mut().ok_or_else(|| anyhow!("fleet options must be an object"))?.remove("join_device");
            let o: ozm_node::FleetInitOptions = serde_json::from_value(v)?;
            let join = join.map(serde_json::from_value::<String>).transpose()?.filter(|s| !s.is_empty());
            if let Some(name) = &join {
                anyhow::ensure!(!joined(&home()?), "this device already joined a fleet; create invites without joining");
                anyhow::ensure!(o.devices.iter().any(|d| d.split_once(':').is_some_and(|(n, _)| n == name)), "no generated device named {name}");
            }
            let result = ozm_node::fleet_init(&o)?;
            let mut v = serde_json::to_value(&result)?;
            if let Some(name) = join {
                let d = result.devices.iter().find(|d| d.name == name).expect("validated name");
                let message = ozm_node::join_invite(&std::fs::read_to_string(&d.invite_path)?, &home()?)?;
                v["joined"] = json!(message);
            }
            Ok(v)
        })())
    }

    fn check_rpc(&mut self, rpc_url: String) -> String {
        out((|| {
            anyhow::ensure!(!rpc_url.trim().is_empty(), "chain RPC URL is required");
            let network = Fleet::load(&home()?.join("fleet.json")).map(|f| f.network).unwrap_or_default();
            if network == zcash_offline_core::fleet::Network::Testnet {
                let servers = rpc_url.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>();
                let source = rt().block_on(ozm_node::lightwallet::Lightwallet::connect(&servers))?;
                return Ok(json!({"chain": "testnet", "blocks": source.tip, "sources": source.sources}));
            }
            ozm_node::check_regtest_rpc(&rpc_url)
        })())
    }

    fn open_fleet_folder(&mut self, folder: String) -> String {
        out((|| {
            let folder = PathBuf::from(folder).canonicalize().context("session folder")?;
            Fleet::load(&folder.join("fleet.json")).context("folder must contain a fleet")?;
            let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
            let mut child = std::process::Command::new(opener).arg(&folder)
                .stdin(std::process::Stdio::null()).spawn().context("launch desktop folder opener")?;
            // A Linux file manager can stay open; do not block module calls while it runs.
            std::thread::spawn(move || { let _ = child.wait(); });
            Ok(json!({"ok": true, "folder": folder, "message": "Folder opener requested. If no window opens, copy the displayed path into your file manager."}))
        })())
    }

    fn join(&mut self, invite_json: String) -> String {
        out(home().and_then(|h| ozm_node::join_invite(&invite_json, &h)).map(|m| json!({"ok": true, "message": m})))
    }

    fn start(&mut self, options_json: String) -> String {
        out((|| {
            let o: StartOptions = if options_json.trim().is_empty() { StartOptions::default() } else { serde_json::from_str(&options_json).context("start options")? };
            self.start_with(o)
        })())
    }

    fn status(&mut self) -> String {
        out(self.status_value())
    }

    fn fleet(&mut self) -> String {
        out((|| match &self.app {
            Some(a) => Ok(serde_json::to_value(&a.cfg.fleet)?),
            None => {
                let h = home()?;
                anyhow::ensure!(joined(&h), "no device in {}: call join with an invite first", h.display());
                Ok(serde_json::to_value(Fleet::load(&h.join("fleet.json"))?)?)
            }
        })())
    }

    fn pay(&mut self, to: String, amount_zec: String) -> String {
        out((|| {
            let a = self.app()?.clone();
            self.not_syncing()?;
            let zat = parse_zec(&amount_zec)?;
            let p = rt().block_on(a.pay(&to, zat))?;
            Ok(serde_json::to_value(p)?)
        })())
    }

    fn sync_now(&mut self) -> String {
        out(self.start_sync())
    }

    fn testnet_tip(&mut self) -> String {
        out(rt().block_on(ozm_node::lightwallet::Lightwallet::connect(&zcash_offline_core::fleet::default_servers()))
            .map(|s| json!({"blocks": s.tip, "sources": s.sources})))
    }

    fn max_payment(&mut self) -> String {
        out(self.app().and_then(|a| a.max_payment()))
    }

    fn prepare_offline(&mut self, notes: i64, amount_zec: String) -> String {
        out((|| {
            let notes = u32::try_from(notes).context("positive note count")?;
            self.not_syncing()?;
            let payment = rt().block_on(self.app()?.prepare_offline(notes, parse_zec(&amount_zec)?))?;
            Ok(serde_json::to_value(payment)?)
        })())
    }

    fn request_funds(&mut self, notes: i64, amount_zec: String) -> String {
        out((|| {
            let a = self.app()?.clone();
            let notes = u32::try_from(notes).ok().filter(|n| *n > 0).ok_or_else(|| anyhow!("notes must be a positive number"))?;
            let zat = parse_zec(&amount_zec)?;
            let gw = rt().block_on(a.request_funds(notes, zat))?;
            Ok(json!({"requested_from": gw}))
        })())
    }

    fn list_payments(&mut self) -> String {
        out(self.app().map(|a| {
            let st = a.state.lock().unwrap();
            json!({"payments": st.1.payments.values().collect::<Vec<_>>(), "refusals": st.1.refusals, "gateway": st.1.gateway.values().collect::<Vec<_>>()})
        }))
    }

    fn leave(&mut self) -> String {
        out((|| {
            let home = home()?;
            self.not_syncing()?;
            if let Some(app) = self.app.take() {
                app.mesh.shutdown();
            }
            if let Some(api) = self.api.take() {
                api.abort();
            }
            let backup = ozm_node::leave_home(&home)?;
            Ok(json!({"ok": true, "backup": backup, "sessions": home.with_file_name("sessions"),
                "message": format!("Left the session. Keys, wallet and history are kept in {}; nothing was deleted.", backup.display())}))
        })())
    }

    fn on_context_ready(&mut self, _ctx: &RustModuleContext) {
        let Ok(h) = home() else { return };
        if !joined(&h) {
            return;
        }
        let o = saved_options(&h);
        if let Err(e) = self.start_with(o) {
            eprintln!("zcash_offline_module: auto-start failed: {e:#}");
        } else if self.app.as_ref().is_some_and(|a| a.gateway_options().0.is_some()) {
            // Resume a sync a restart interrupted; never on the call thread.
            let _ = self.start_sync();
        }
    }
}

#[no_mangle]
pub extern "Rust" fn logos_module_install() {
    install::<Node>();
}
