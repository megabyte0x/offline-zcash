use std::{
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use ozm_mesh::{Discovery, Mesh, MeshConfig, MeshEvent, PeerId};
use tokio::{sync::mpsc::Receiver, time::timeout};

const TAG: [u8; 8] = *b"testflt1";

/// A UDP port that was free a moment ago; every node in one test shares it.
fn free_udp_port() -> u16 {
    UdpSocket::bind("0.0.0.0:0").unwrap().local_addr().unwrap().port()
}

async fn start(id: u8, port: u16, tag: [u8; 8]) -> (Mesh, Receiver<MeshEvent>) {
    Mesh::start(MeshConfig {
        peer_id: PeerId([id; 8]),
        listen: Some(SocketAddr::from(([127, 0, 0, 1], 0))),
        discovery: Some(Discovery { port, tag, interval: Duration::from_millis(300), advertise_port: None }),
        ..MeshConfig::default()
    })
    .await
    .expect("start mesh")
}

async fn until(what: &str, mut f: impl FnMut() -> bool) {
    timeout(Duration::from_secs(8), async {
        while !f() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

#[tokio::test(flavor = "multi_thread")]
async fn nodes_on_one_lan_discover_each_other_and_deliver() {
    let port = free_udp_port();
    let (a, _arx) = start(1, port, TAG).await;
    let (b, _brx) = start(2, port, TAG).await;
    let (c, mut crx) = start(3, port, TAG).await;
    let nodes = [&a, &b, &c];

    until("every node to have a neighbour", || nodes.iter().all(|n| !n.neighbors().is_empty())).await;
    // Every pair hears each other, so the mesh becomes a triangle with exactly one link per pair.
    until("a full triangle", || nodes.iter().all(|n| n.neighbors().len() == 2)).await;
    // A link can come up before this node has heard the other side's beacon.
    until("every beacon heard", || nodes.iter().all(|n| n.stats().discovered == 2)).await;
    for n in nodes {
        let found: Vec<PeerId> = n.discovered().into_iter().map(|(p, _)| p).collect();
        assert_eq!(found.len(), 2, "{} discovered {found:?}", n.peer_id());
        for (_, addr) in n.discovered() {
            assert!(addr.ip().is_loopback(), "dial address {addr} should be the listen address");
        }
    }

    let id = a.send(c.peer_id(), b"hello over discovery".to_vec()).await.unwrap();
    let got = timeout(Duration::from_secs(5), async {
        loop {
            if let Some(MeshEvent::Message { msg_id, payload, from, .. }) = crx.recv().await {
                if msg_id == id {
                    return (from, payload);
                }
            }
        }
    })
    .await
    .expect("message not delivered");
    assert_eq!(got, (a.peer_id(), b"hello over discovery".to_vec()));

    // A dropped node is redialed after it comes back (same id, new listen port).
    c.shutdown();
    until("c to drop", || a.neighbors().len() == 1 && b.neighbors().len() == 1).await;
    let (c2, _c2rx) = start(3, port, TAG).await;
    until("c to reconnect", || c2.neighbors().len() == 2).await;
    // Links stay stable: no flapping between duplicates.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(a.neighbors(), vec![b.peer_id(), c2.peer_id()]);
    for n in [&a, &b, &c2] {
        n.shutdown();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn other_fleet_tag_is_never_connected() {
    let port = free_udp_port();
    let (a, _arx) = start(1, port, TAG).await;
    let (b, _brx) = start(2, port, TAG).await;
    let (x, _xrx) = start(9, port, *b"otherflt").await;

    until("a and b to link", || a.neighbors() == vec![b.peer_id()]).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(a.neighbors(), vec![b.peer_id()]);
    assert_eq!(b.neighbors(), vec![a.peer_id()]);
    assert!(x.neighbors().is_empty());
    assert!(x.discovered().is_empty());
    assert_eq!(a.discovered().len(), 1);
    for n in [&a, &b, &x] {
        n.shutdown();
    }
}
