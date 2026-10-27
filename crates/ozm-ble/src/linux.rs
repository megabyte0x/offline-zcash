//! BlueZ IO mode gives each remote device its own SOCK_SEQPACKET fd. In particular,
//! notifications must not use the shared Value property (which broadcasts to all subscribers).
use std::{
    collections::BTreeMap,
    os::fd::FromRawFd,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context as TaskContext, Poll},
    time::Duration,
};

use anyhow::{bail, ensure, Context, Result};
use bluer::{
    adv::Advertisement,
    gatt::{
        local::{self, CharacteristicControlEvent as Event},
        CharacteristicReader, CharacteristicWriter,
    },
    Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport,
};
use futures::StreamExt;
use ozm_mesh::Mesh;
use tokio::{
    sync::oneshot,
    task::JoinSet,
    time::{timeout, Instant},
};
use tracing::{debug, info, warn};

use crate::{
    stream::{self, MAX_LINKS, WRITE_TIMEOUT},
    Ble, SERVICE, STREAM,
};

type Key = (Address, bool); // remote address, outbound (both roles may race on one ACL)

/// BlueR 0.17 subtracts five from the ATT MTU, not three. Own its fd so default
/// MTU 23 still sends 20-byte chunks, and MTU 517 never exceeds Android's 512 cap.
struct Writer {
    socket: tokio::net::UnixDatagram,
    payload: usize,
}

impl Writer {
    fn new(wr: CharacteristicWriter) -> Result<Self> {
        let payload = stream::payload_size(wr.mtu() + 2);
        // SAFETY: into_raw_fd transfers sole ownership of this valid SOCK_SEQPACKET fd.
        let socket = unsafe { std::os::unix::net::UnixDatagram::from_raw_fd(wr.into_raw_fd()?) };
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: tokio::net::UnixDatagram::from_std(socket)?,
            payload,
        })
    }
}

impl tokio::io::AsyncWrite for Writer {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.socket
            .poll_send(cx, &bytes[..bytes.len().min(self.payload)])
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

struct Link {
    id: u64,
    since: Instant,
    activity: Arc<Mutex<Instant>>,
    reader: Option<CharacteristicReader>,
    writer: Option<CharacteristicWriter>,
    cancel: Option<oneshot::Sender<()>>,
}

#[derive(Default)]
struct Links {
    entries: BTreeMap<Key, Link>,
    serial: u64,
}

impl Links {
    /// Outbound discovery never evicts healthy links; an incoming GATT setup evicts the
    /// longest-idle slot. Pending setups count toward the same six-link bound.
    fn reserve(&mut self, key: Key) -> Option<&mut Link> {
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= MAX_LINKS {
                if key.1 {
                    return None;
                }
                let oldest = *self
                    .entries
                    .iter()
                    .min_by_key(|(_, l)| *l.activity.lock().unwrap())?
                    .0;
                self.entries.remove(&oldest); // dropping cancel closes the old stream
            }
            self.serial += 1;
            self.entries.insert(
                key,
                Link {
                    id: self.serial,
                    since: Instant::now(),
                    activity: Arc::new(Mutex::new(Instant::now())),
                    reader: None,
                    writer: None,
                    cancel: None,
                },
            );
        }
        self.entries.get_mut(&key)
    }

    fn finished(&mut self, key: Key, id: u64) {
        if self.entries.get(&key).is_some_and(|l| l.id == id) {
            self.entries.remove(&key);
        }
    }

    fn contains_address(&self, addr: Address) -> bool {
        self.entries.keys().any(|(a, _)| *a == addr)
    }
}

pub(crate) async fn start(mesh: Mesh, hint: [u8; 4]) -> Result<Ble> {
    let session = bluer::Session::new()
        .await
        .context("BLE: connect to system D-Bus / bluetoothd")?;
    let adapter = session
        .default_adapter()
        .await
        .context("BLE: no BlueZ adapter; check bluetoothd and USB adapter")?;
    ensure!(
        adapter.is_powered().await?,
        "BLE: adapter is off; enable Bluetooth before starting"
    );
    let (mut control, handle) = local::characteristic_control();
    let app = local::Application {
        services: vec![local::Service {
            uuid: SERVICE,
            primary: true,
            characteristics: vec![local::Characteristic {
                uuid: STREAM,
                write: Some(local::CharacteristicWrite {
                    write_without_response: true,
                    method: local::CharacteristicWriteMethod::Io,
                    ..Default::default()
                }),
                notify: Some(local::CharacteristicNotify {
                    notify: true,
                    method: local::CharacteristicNotifyMethod::Io,
                    ..Default::default()
                }),
                control_handle: handle,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let application = adapter
        .serve_gatt_application(app)
        .await
        .context("BLE: register GATT server (check D-Bus permissions)")?;
    // Only service data in the primary advertisement: UUID list + data exceeds legacy 31 bytes.
    // Android filters on service data, not the UUID list. BlueZ supplies the CCCD automatically.
    let advertisement = adapter
        .advertise(Advertisement {
            service_data: [(SERVICE, hint.to_vec())].into_iter().collect(),
            discoverable: Some(true),
            ..Default::default()
        })
        .await
        .context("BLE: advertise (adapter must support LE peripheral advertising)")?;
    adapter
        .set_discovery_filter(DiscoveryFilter {
            transport: DiscoveryTransport::Le,
            duplicate_data: true,
            ..Default::default()
        })
        .await
        .context("BLE: configure LE scan")?;
    let mut scan = adapter
        .discover_devices_with_changes()
        .await
        .context("BLE: start LE scan")?;
    info!(adapter = adapter.name(), "BLE scanning and advertising");
    let task = tokio::spawn(async move {
        // Keep all registrations alive until shutdown; JoinSet drops/aborts every stream on exit.
        let _resources = (session, application, advertisement);
        let mut links = Links::default();
        let mut tasks = JoinSet::new();
        let mut retry = BTreeMap::<Address, Instant>::new();
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                event = control.next() => {
                    let Some(event) = event else { break; };
                    let addr = match &event {
                        Event::Write(req) => req.device_address(),
                        Event::Notify(wr) => wr.device_address(),
                    };
                    let key = (addr, false);
                    // Reserve before accepting either fd, including half-open inbound setups.
                    let l = links.reserve(key).expect("inbound reserves a slot");
                    if l.cancel.is_some() { continue; }
                    match event {
                        Event::Write(req) => match req.accept() {
                            Ok(rd) => l.reader = Some(rd),
                            Err(e) => { debug!(%addr, error = %e, "BLE accept failed"); links.entries.remove(&key); continue; }
                        },
                        Event::Notify(wr) => l.writer = Some(wr),
                    }
                    if l.reader.is_some() && l.writer.is_some() {
                        let rd = l.reader.take().unwrap();
                        let wr = l.writer.take().unwrap();
                        let activity = l.activity.clone();
                        let m = mesh.clone();
                        spawn_link(&mut tasks, key, l, async move {
                            let wr = Writer::new(wr)?;
                            let payload = wr.payload;
                            stream::attach(&m, rd, wr, payload, false, activity).await
                        });
                    }
                },
                event = scan.next() => {
                    let Some(event) = event else { break; };
                    if let AdapterEvent::DeviceAdded(addr) = event {
                        if links.contains_address(addr) || retry.get(&addr).is_some_and(|t| *t > Instant::now()) { continue; }
                        let Ok(device) = adapter.device(addr) else { continue; };
                        let data = device.service_data().await.ok().flatten().unwrap_or_default();
                        if !stream::same_fleet(data.get(&SERVICE).map(Vec::as_slice), &hint) { continue; }
                        let key = (addr, true);
                        if let Some(l) = links.reserve(key) {
                            let m = mesh.clone();
                            let activity = l.activity.clone();
                            spawn_link(&mut tasks, key, l, async move {
                                let (rd, wr) = timeout(WRITE_TIMEOUT, connect(&device)).await.context("BLE connect/setup timed out")??;
                                let wr = Writer::new(wr)?;
                                let payload = wr.payload;
                                stream::attach(&m, rd, wr, payload, true, activity).await
                            });
                        }
                    }
                },
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    if let Ok((key, id, result)) = result {
                        if let Err(e) = result { debug!(addr = %key.0, error = %e, "BLE link ended"); }
                        links.finished(key, id);
                        if !links.contains_address(key.0) {
                            disconnect(&adapter, key.0).await;
                            retry.insert(key.0, Instant::now() + Duration::from_secs(3));
                        }
                    }
                },
                _ = tick.tick() => {
                    if mesh.is_shutdown() { break; }
                    let expired: Vec<_> = links.entries.iter().filter(|(_, l)| l.cancel.is_none() && l.since.elapsed() >= WRITE_TIMEOUT).map(|(k, _)| *k).collect();
                    for key in expired {
                        links.entries.remove(&key);
                        if !links.contains_address(key.0) { disconnect(&adapter, key.0).await; }
                    }
                    retry.retain(|_, t| *t > Instant::now());
                },
            }
        }
        warn!("BLE stopped; TCP/LAN remains available");
    });
    Ok(Ble { task })
}

fn spawn_link<F>(tasks: &mut JoinSet<(Key, u64, Result<()>)>, key: Key, link: &mut Link, future: F)
where
    F: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    link.cancel = Some(tx);
    let id = link.id;
    tasks.spawn(async move {
        let result = tokio::select! { r = future => r, _ = rx => Ok(()) };
        (key, id, result)
    });
}

async fn connect(device: &bluer::Device) -> Result<(CharacteristicReader, CharacteristicWriter)> {
    if !device.is_connected().await? {
        device.connect().await?;
    }
    for service in device.services().await? {
        if service.uuid().await? != SERVICE {
            continue;
        }
        for c in service.characteristics().await? {
            if c.uuid().await? == STREAM {
                let flags = c.flags().await?;
                ensure!(
                    flags.notify && flags.write_without_response,
                    "BLE stream has incompatible GATT properties"
                );
                // Subscribe before writing Hello, so the peripheral's first bytes cannot be lost.
                let rd = c.notify_io().await?;
                let wr = c.write_io().await?;
                return Ok((rd, wr));
            }
        }
    }
    bail!("BLE stream service/characteristic missing")
}

async fn disconnect(adapter: &Adapter, addr: Address) {
    if let Ok(device) = adapter.device(addr) {
        let _ = device.disconnect().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn slots_count_pending_skip_full_evict_idle_and_ignore_old_completion() {
        let mut links = Links::default();
        for i in 0..6 {
            links
                .reserve((Address::new([i, 0, 0, 0, 0, 0]), true))
                .unwrap();
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        let oldest = (Address::new([0; 6]), true);
        let id = links.entries[&oldest].id;
        // Recent traffic saves the oldest connection; the second-oldest is evicted instead.
        *links.entries[&oldest].activity.lock().unwrap() = Instant::now();
        let seventh = Address::new([7; 6]);
        assert!(links.reserve((seventh, true)).is_none());
        assert_eq!(links.entries.len(), 6);
        assert!(links.reserve((seventh, false)).is_some());
        assert!(!links.contains_address(Address::new([1, 0, 0, 0, 0, 0])));
        assert!(links.entries.contains_key(&oldest));
        links.finished(oldest, id);
        let replacement = links.reserve(oldest).unwrap().id;
        links.finished(oldest, id);
        assert_eq!(links.entries[&oldest].id, replacement);
        assert_eq!(links.entries.len(), 6);
    }
}
