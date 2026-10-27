use std::{net::SocketAddr, path::PathBuf, time::Duration};

use ozm_mesh::{Mesh, MeshConfig, MeshEvent, MsgId, PeerId};
use tokio::{sync::mpsc::Receiver, time::timeout};

const WAIT: Duration = Duration::from_secs(10);

struct Node {
    mesh: Mesh,
    rx: Receiver<MeshEvent>,
}

impl Node {
    fn id(&self) -> PeerId {
        self.mesh.peer_id()
    }

    fn addr(&self) -> String {
        self.mesh.listen_addr().expect("listening").to_string()
    }

    /// Waits for the first event matching `f`, discarding others.
    async fn wait_for<T>(&mut self, mut f: impl FnMut(&MeshEvent) -> Option<T>) -> T {
        timeout(WAIT, async {
            loop {
                let ev = self.rx.recv().await.expect("event stream closed");
                if let Some(t) = f(&ev) {
                    return t;
                }
            }
        })
        .await
        .expect("timed out waiting for event")
    }

    /// Messages (directed or broadcast) received so far without waiting.
    fn drain_messages(&mut self) -> Vec<MeshEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            if matches!(ev, MeshEvent::Message { .. } | MeshEvent::Delivered { .. }) {
                out.push(ev);
            }
        }
        out
    }

    fn drain_all(&mut self) -> Vec<MeshEvent> {
        std::iter::from_fn(|| self.rx.try_recv().ok()).collect()
    }

    async fn wait_neighbors(&self, n: usize) {
        timeout(WAIT, async {
            while self.mesh.neighbors().len() < n {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("neighbours did not come up");
    }
}

async fn start(peer_id: PeerId, listen: bool, dial: &[String], store_path: Option<PathBuf>) -> Node {
    let cfg = MeshConfig {
        peer_id,
        listen: listen.then(|| SocketAddr::from(([127, 0, 0, 1], 0))),
        dial: dial.to_vec(),
        store_path,
        ..MeshConfig::default()
    };
    let (mesh, rx) = Mesh::start(cfg).await.expect("start mesh");
    Node { mesh, rx }
}

fn pid(b: u8) -> PeerId {
    PeerId([b; 8])
}

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ozm-mesh-{name}-{}", MsgId::random()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A–R–B–D built by dialing: A→R, R→B, B→D.
async fn line() -> (Node, Node, Node, Node) {
    let d = start(pid(0xd), true, &[], None).await;
    let b = start(pid(0xb), true, &[d.addr()], None).await;
    let r = start(pid(0x2), true, &[b.addr()], None).await;
    let a = start(pid(0xa), false, &[r.addr()], None).await;
    a.wait_neighbors(1).await;
    r.wait_neighbors(2).await;
    b.wait_neighbors(2).await;
    d.wait_neighbors(1).await;
    assert_eq!(r.mesh.neighbors(), vec![pid(0xa), pid(0xb)]);
    (a, r, b, d)
}

#[tokio::test]
async fn line_directed_20k_arrives_once_and_is_acked() {
    let (mut a, r, mut b, mut d) = line().await;
    let data = payload(20_000);
    let id = a.mesh.send(b.id(), data.clone()).await.unwrap();

    let (from, got, hops) = b
        .wait_for(|ev| match ev {
            MeshEvent::Message { from, msg_id, payload, hops, broadcast: false } if *msg_id == id => {
                Some((*from, payload.clone(), *hops))
            }
            _ => None,
        })
        .await;
    assert_eq!(from, a.id());
    assert_eq!(got, data);
    assert_eq!(hops, 2, "A -> R -> B crosses two links");

    let by = a
        .wait_for(|ev| match ev {
            MeshEvent::Delivered { msg_id, by } if *msg_id == id => Some(*by),
            _ => None,
        })
        .await;
    assert_eq!(by, b.id());

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(b.drain_messages().is_empty(), "B got the message twice");
    assert!(a.drain_messages().is_empty(), "A got a second Delivered");
    assert!(d.drain_messages().is_empty(), "D must get nothing addressed to it");
    // 20,000 bytes at mtu 512 needs dozens of frames per link.
    assert!(r.mesh.stats().frames_received > 40);
    assert!(r.mesh.stats().packets_relayed >= 2, "R relays the message and the ack");
}

#[tokio::test]
async fn store_and_forward_with_restarts() {
    let dir = temp_dir("sf");
    let a_store = dir.join("a.json");
    let b_store = dir.join("b.json");
    let (a_id, b_id) = (pid(0xa), pid(0xb));

    let r = start(pid(0x2), true, &[], None).await;
    let a = start(a_id, false, &[r.addr()], Some(a_store.clone())).await;
    a.wait_neighbors(1).await;

    // B is down: the message waits at A and R.
    let data = payload(9_166);
    let id = a.mesh.send(b_id, data.clone()).await.unwrap();
    timeout(WAIT, async {
        while r.mesh.stats().stored == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("R never stored the message");
    a.mesh.shutdown();
    drop(a);

    // B comes up later next to R only, and gets the message from R's store.
    let mut b = start(b_id, true, &[r.addr()], Some(b_store.clone())).await;
    let (from, got, hops) = b
        .wait_for(|ev| match ev {
            MeshEvent::Message { from, msg_id, payload, hops, .. } if *msg_id == id => {
                Some((*from, payload.clone(), *hops))
            }
            _ => None,
        })
        .await;
    assert_eq!((from, got, hops), (a_id, data, 2));

    // Restart B from its store: it must not deliver the message again.
    b.mesh.shutdown();
    drop(b);
    let mut b = start(b_id, true, &[r.addr()], Some(b_store.clone())).await;
    b.wait_neighbors(1).await;

    // A comes back from its persisted store (still holding the message) and also meets B directly.
    let mut a = start(a_id, false, &[r.addr(), b.addr()], Some(a_store.clone())).await;
    let by = a
        .wait_for(|ev| match ev {
            MeshEvent::Delivered { msg_id, by } if *msg_id == id => Some(*by),
            _ => None,
        })
        .await;
    assert_eq!(by, b_id);
    a.wait_neighbors(2).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(b.drain_messages().is_empty(), "restarted B re-delivered");
    assert!(a.drain_messages().is_empty(), "A got Delivered twice");
    assert!(a_store.exists() && b_store.exists());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn broadcast_from_d_reaches_everyone() {
    let (mut a, mut r, mut b, d) = line().await;
    let data = b"hello mesh".to_vec();
    let id = d.mesh.broadcast(data.clone()).await.unwrap();
    for (node, hops) in [(&mut b, 1), (&mut r, 2), (&mut a, 3)] {
        let got = node
            .wait_for(|ev| match ev {
                MeshEvent::Message { from, msg_id, payload, hops, broadcast: true } if *msg_id == id => {
                    Some((*from, payload.clone(), *hops))
                }
                _ => None,
            })
            .await;
        assert_eq!(got, (d.id(), data.clone(), hops));
    }
}

#[tokio::test]
async fn triangle_delivers_once() {
    let z = start(pid(3), true, &[], None).await;
    let y = start(pid(2), true, &[z.addr()], None).await;
    // Y dials Z, X dials Y and Z: every pair is connected.
    let mut x = start(pid(1), true, &[y.addr(), z.addr()], None).await;
    let mut z = z;
    let mut y = y;
    x.wait_neighbors(2).await;
    y.wait_neighbors(2).await;
    z.wait_neighbors(2).await;

    let id = x.mesh.send(z.id(), payload(3_000)).await.unwrap();
    let got = z
        .wait_for(|ev| match ev {
            MeshEvent::Message { msg_id, payload, .. } if *msg_id == id => Some(payload.clone()),
            _ => None,
        })
        .await;
    assert_eq!(got, payload(3_000));
    x.wait_for(|ev| matches!(ev, MeshEvent::Delivered { msg_id, .. } if *msg_id == id).then_some(()))
        .await;

    let bid = y.mesh.broadcast(b"tri".to_vec()).await.unwrap();
    for n in [&mut x, &mut z] {
        n.wait_for(|ev| matches!(ev, MeshEvent::Message { msg_id, .. } if *msg_id == bid).then_some(()))
            .await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(z.drain_messages().is_empty(), "Z got a duplicate");
    assert!(x.drain_messages().is_empty(), "X got a duplicate");
    assert!(y.drain_messages().is_empty(), "Y saw a message meant for Z or its own broadcast");
}

#[tokio::test]
async fn mutual_dial_keeps_one_link() {
    let free_port = || std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let (pa, pb) = (free_port(), free_port());
    let cfg = |id: u8, listen: SocketAddr, dial: SocketAddr| MeshConfig {
        peer_id: pid(id),
        listen: Some(listen),
        dial: vec![dial.to_string()],
        ..MeshConfig::default()
    };
    let (p, _prx) = Mesh::start(cfg(1, pa, pb)).await.unwrap();
    let (q, qrx) = Mesh::start(cfg(2, pb, pa)).await.unwrap();
    let mut q = Node { mesh: q, rx: qrx };
    q.wait_neighbors(1).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(p.neighbors(), vec![pid(2)]);
    assert_eq!(q.mesh.neighbors(), vec![pid(1)]);
    let ups = q.drain_all().iter().filter(|e| matches!(e, MeshEvent::NeighborUp(_))).count();
    assert_eq!(ups, 1, "exactly one link to the neighbour");
    let id = p.send(pid(2), b"x".to_vec()).await.unwrap();
    q.wait_for(|ev| matches!(ev, MeshEvent::Message { msg_id, .. } if *msg_id == id).then_some(())).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(q.drain_messages().is_empty());
}

/// A radio surrogate: bounded buffers, partial reads, and callback-sized stalls in both directions.
async fn attach_chunked(a: &Mesh, b: &Mesh, chunk: usize) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (left, radio_left) = tokio::io::duplex(chunk);
    let (right, radio_right) = tokio::io::duplex(chunk);
    let (lr, lw) = tokio::io::split(radio_left);
    let (rr, rw) = tokio::io::split(radio_right);
    async fn pump(mut r: impl tokio::io::AsyncRead + Unpin, mut w: impl tokio::io::AsyncWrite + Unpin, chunk: usize) {
        let mut buf = vec![0; chunk];
        let mut writes = 0;
        loop {
            let n = r.read(&mut buf).await.unwrap();
            if n == 0 { break; }
            writes += 1;
            if writes % 8 == 0 { tokio::time::sleep(Duration::from_millis(2)).await; }
            if w.write_all(&buf[..n]).await.is_err() { break; }
        }
    }
    tokio::spawn(pump(lr, rw, chunk));
    tokio::spawn(pump(rr, lw, chunk));
    for (mesh, stream, outbound) in [(a.clone(), left, true), (b.clone(), right, false)] {
        tokio::spawn(async move { mesh.attach(stream, outbound).await; });
    }
}

#[tokio::test]
async fn attached_streams_chunked_stalled_multihop_and_acked() {
    for chunk in [20, 182, 514] {
        let mut a = start(pid(1), false, &[], None).await;
        let r = start(pid(2), false, &[], None).await;
        let mut b = start(pid(3), false, &[], None).await;
        attach_chunked(&a.mesh, &r.mesh, chunk).await;
        a.wait_neighbors(1).await;
        // Store-and-forward: B is not yet attached when A sends.
        let data = payload(20_123);
        let id = a.mesh.send(b.id(), data.clone()).await.unwrap();
        timeout(WAIT, async {
            while r.mesh.stats().stored == 0 { tokio::time::sleep(Duration::from_millis(10)).await; }
        }).await.unwrap();
        attach_chunked(&r.mesh, &b.mesh, chunk).await;
        let got = b.wait_for(|ev| match ev {
            MeshEvent::Message { msg_id, payload, hops, from, .. } if *msg_id == id => Some((payload.clone(), *hops, *from)),
            _ => None,
        }).await;
        assert_eq!(got, (data, 2, a.id()), "chunk {chunk}");
        a.wait_for(|ev| matches!(ev, MeshEvent::Delivered { msg_id, by } if *msg_id == id && *by == b.id()).then_some(())).await;
        assert_eq!(a.mesh.neighbor_links(), vec![(r.id(), ozm_mesh::LinkKind::Ble)]);
        assert!(r.mesh.stats().packets_synced > 0);
        for node in [&a, &r, &b] { node.mesh.shutdown(); }
    }
}
