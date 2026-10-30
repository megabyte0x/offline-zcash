//! Synchronous device calls run on a Kotlin worker, never the UI or BLE callback thread.
//! LinkSink.write blocks until the platform has flow-controlled all bytes (false closes the link).
use anyhow::{anyhow, ensure, Result};
use ozm_node::{App, RunOpts};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::Runtime,
    sync::{mpsc, watch},
};
use zcash_offline_core::{parse_zec, DeviceSecret, Fleet};

uniffi::setup_scaffolding!();

#[uniffi::export(callback_interface)]
pub trait LinkSink: Send + Sync {
    /// Must not call back into node operations. May call link_received on the other node.
    fn write(&self, handle: u64, bytes: Vec<u8>) -> bool;
    /// Also called on duplicate rejection, bad framing, timeout and mesh shutdown.
    fn link_closed(&self, handle: u64);
}

struct Link {
    received: mpsc::Sender<Vec<u8>>,
    closed: watch::Sender<bool>,
}

#[derive(uniffi::Object)]
pub struct OfflineNode {
    home: PathBuf,
    app: Mutex<Option<Arc<App>>>,
    links: Arc<Mutex<HashMap<u64, Link>>>,
    rt: Runtime,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Options {
    network: Option<zcash_offline_core::fleet::Network>,
    birthday: Option<u32>,
    listen: Option<String>,
    dial: Option<Vec<String>>,
    rpc: Option<String>,
    mine: Option<bool>,
    discover: Option<bool>,
    discovery_port: Option<u16>,
}

fn out(r: Result<Value>) -> String {
    r.unwrap_or_else(|e| json!({"error": format!("{e:#}")}))
        .to_string()
}

impl OfflineNode {
    fn app(&self) -> Result<Arc<App>> {
        self.app
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow!("not started"))
    }

    fn status_value(&self) -> Result<Value> {
        let app = self.app.lock().unwrap();
        let mut v = match app.as_ref() {
            Some(a) => a.status(),
            None if self.home.join("secret.json").exists() => {
                let me: DeviceSecret =
                    serde_json::from_slice(&std::fs::read(self.home.join("secret.json"))?)?;
                json!({"name": me.name, "role": if zcash_offline_core::fleet::is_gateway_role(&me.role) { "gateway" } else { "" }})
            }
            None => json!({}),
        };
        v["joined"] = json!(self.home.join("secret.json").exists());
        v["running"] = json!(app.is_some());
        Ok(v)
    }
}

#[uniffi::export]
impl OfflineNode {
    #[uniffi::constructor]
    pub fn new(home: String) -> Arc<Self> {
        Arc::new(Self {
            home: home.into(),
            app: Mutex::new(None),
            links: Arc::new(Mutex::new(HashMap::new())),
            rt: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("ozm")
                .build()
                .expect("runtime"),
        })
    }

    pub fn join(&self, invite_json: String) -> String {
        out((|| {
            let app = self.app.lock().unwrap();
            ensure!(app.is_none(), "stop the node before joining");
            Ok(json!({"ok": true, "message": ozm_node::join_invite(&invite_json, &self.home)?}))
        })())
    }

    pub fn start(&self, options_json: String) -> String {
        out((|| {
            let mut app = self.app.lock().unwrap();
            if app.is_none() {
                let o: Options = if options_json.trim().is_empty() {
                    Options::default()
                } else {
                    serde_json::from_str(&options_json)?
                };
                let mut opts = RunOpts::for_home(&self.home);
                opts.network = o.network.unwrap_or_default();
                opts.birthday = o.birthday.unwrap_or(0);
                if let Some(l) = o.listen {
                    opts.listen = l.parse()?;
                }
                if let Some(d) = o.dial {
                    opts.dial = d;
                }
                opts.rpc = o.rpc;
                opts.mine = o.mine.unwrap_or(false);
                opts.discover = o.discover.unwrap_or(true);
                if let Some(p) = o.discovery_port {
                    opts.discovery_port = p;
                }
                *app = Some(self.rt.block_on(ozm_node::start(&opts))?);
            }
            drop(app);
            self.status_value()
        })())
    }

    pub fn status(&self) -> String {
        out(self.status_value())
    }

    pub fn fleet(&self) -> String {
        out((|| {
            Ok(serde_json::to_value(Fleet::load(
                &self.home.join("fleet.json"),
            )?)?)
        })())
    }

    pub fn pay(&self, to: String, amount_zec: String) -> String {
        out((|| {
            Ok(serde_json::to_value(self.rt.block_on(
                self.app()?.pay(&to, parse_zec(&amount_zec)?),
            )?)?)
        })())
    }

    pub fn sync_now(&self) -> String {
        out(self.app().and_then(|a| self.rt.block_on(a.sync())))
    }

    pub fn max_payment(&self) -> String {
        out(self.app().and_then(|a| a.max_payment()))
    }

    pub fn prepare_offline(&self, notes: u32, amount_zec: String) -> String {
        out((|| Ok(serde_json::to_value(self.rt.block_on(
            self.app()?.prepare_offline(notes, parse_zec(&amount_zec)?),
        )?)?))())
    }

    pub fn request_funds(&self, notes: u32, amount_zec: String) -> String {
        out((|| {
            ensure!(notes > 0, "notes must be positive");
            Ok(
                json!({"requested_from": self.rt.block_on(self.app()?.request_funds(notes, parse_zec(&amount_zec)?))?}),
            )
        })())
    }

    pub fn list_payments(&self) -> String {
        out(self.app().map(|a| a.payments_json()))
    }

    /// Handles must be unique for the lifetime of this object, including reconnects.
    pub fn link_opened(&self, handle: u64, outbound: bool, sink: Box<dyn LinkSink>) -> String {
        out((|| {
            let mesh = self.app()?.mesh.clone();
            let mut links = self.links.lock().unwrap();
            ensure!(!links.contains_key(&handle), "handle already open");
            ensure!(links.len() < 6, "six BLE links already open");
            let (tx, mut rx) = mpsc::channel::<Vec<u8>>(128);
            let (close, mut closed) = watch::channel(false);
            links.insert(
                handle,
                Link {
                    received: tx,
                    closed: close,
                },
            );
            let links = self.links.clone();
            let (stream, bridge) = tokio::io::duplex(64 * 1024);
            let (mut rd, mut wr) = tokio::io::split(bridge);
            let sink: Arc<dyn LinkSink> = sink.into();
            self.rt.spawn(async move {
                let attached = tokio::spawn(async move {
                    mesh.attach(stream, outbound).await;
                });
                let incoming = async move {
                    while let Some(bytes) = rx.recv().await {
                        if wr.write_all(&bytes).await.is_err() {
                            break;
                        }
                    }
                };
                let s = sink.clone();
                let outgoing = async move {
                    let mut buf = vec![0; 4096];
                    while let Ok(n) = rd.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let s = s.clone();
                        let bytes = buf[..n].to_vec();
                        if !tokio::task::spawn_blocking(move || s.write(handle, bytes))
                            .await
                            .unwrap_or(false)
                        {
                            break;
                        }
                    }
                };
                tokio::select! {
                    _ = incoming => {},
                    _ = outgoing => {},
                    _ = closed.changed() => {},
                }
                // Both bridge halves have dropped; let run_link unregister the neighbour on EOF.
                let _ = attached.await;
                links.lock().unwrap().remove(&handle);
                // Wake any platform write waiting for a callback, including after Rust EOF.
                tokio::task::spawn_blocking(move || sink.link_closed(handle));
            });
            Ok(json!({"ok": true}))
        })())
    }

    /// Called serially per link. Overflow/oversize closes the stream, never drops a chunk silently.
    pub fn link_received(&self, handle: u64, bytes: Vec<u8>) -> bool {
        let mut links = self.links.lock().unwrap();
        let ok = links.get(&handle).is_some_and(|l| {
            !bytes.is_empty() && bytes.len() <= 514 && l.received.try_send(bytes).is_ok()
        });
        if !ok {
            if let Some(link) = links.remove(&handle) {
                let _ = link.closed.send(true);
            }
        }
        ok
    }

    pub fn link_closed(&self, handle: u64) {
        if let Some(link) = self.links.lock().unwrap().remove(&handle) {
            let _ = link.closed.send(true);
        }
    }

    pub fn stop(&self) {
        if let Some(app) = self.app.lock().unwrap().take() {
            app.mesh.shutdown();
        }
        for (_, link) in self.links.lock().unwrap().drain() {
            let _ = link.closed.send(true);
        }
    }
}

impl Drop for OfflineNode {
    fn drop(&mut self) {
        self.stop();
    }
}
