use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{ensure, Context, Result};
use ozm_mesh::Mesh;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

pub(crate) const WRITE_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const MAX_LINKS: usize = 6;

/// Same attribute limit as Android, even when ATT MTU is 517.
pub(crate) fn payload_size(att_payload: usize) -> usize {
    att_payload.clamp(20, 512)
}

pub(crate) fn same_fleet(data: Option<&[u8]>, hint: &[u8; 4]) -> bool {
    data.is_some_and(|d| d.len() == 4 && (d == hint || d == [0; 4] || *hint == [0; 4]))
}

/// One bounded, flow-controlled chunk at a time. A completed write means local acceptance,
/// not delivery; inventory and end-to-end mesh acks repair loss after reconnect.
async fn pump<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut rd: R,
    mut wr: W,
    payload: usize,
    activity: &Mutex<Instant>,
) -> Result<()> {
    let mut buf = vec![0; payload_size(payload)];
    loop {
        let n = rd.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        *activity.lock().unwrap() = Instant::now();
        tokio::time::timeout(WRITE_TIMEOUT, wr.write_all(&buf[..n]))
            .await
            .context("BLE local write timed out")??;
    }
}

/// Both halves end together, including on mesh duplicate rejection and cancellation.
pub(crate) async fn attach<R, W>(
    mesh: &Mesh,
    rd: R,
    wr: W,
    payload: usize,
    outbound: bool,
    activity: Arc<Mutex<Instant>>,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    ensure!(payload >= 20, "invalid ATT payload {payload}");
    let (local, radio) = tokio::io::duplex(64 * 1024);
    let (from_mesh, to_mesh) = tokio::io::split(radio);
    tokio::select! {
        _ = mesh.attach(local, outbound) => Ok(()),
        r = pump(rd, to_mesh, 512, &activity) => r,
        r = pump(from_mesh, wr, payload, &activity) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ozm_mesh::{LinkKind, MeshConfig, MeshEvent};
    use tokio::{
        io::duplex,
        time::{timeout, Duration},
    };

    #[test]
    fn android_payload_and_fleet_filter() {
        assert_eq!(payload_size(20), 20);
        assert_eq!(payload_size(182), 182);
        assert_eq!(payload_size(514), 512);
        assert!(same_fleet(Some(&[1, 2, 3, 4]), &[1, 2, 3, 4]));
        assert!(same_fleet(Some(&[0; 4]), &[1, 2, 3, 4]));
        assert!(same_fleet(Some(&[1, 2, 3, 4]), &[0; 4]));
        assert!(!same_fleet(Some(&[0; 3]), &[0; 4]));
        for data in [
            None,
            Some(&[1, 2, 3][..]),
            Some(&[1, 2, 3, 5][..]),
            Some(&[1, 2, 3, 4, 5][..]),
        ] {
            assert!(!same_fleet(data, &[1, 2, 3, 4]));
        }
    }

    #[tokio::test]
    async fn chunk_boundaries_and_bytes_match_android() {
        struct Capture(Vec<Vec<u8>>);
        impl AsyncWrite for Capture {
            fn poll_write(
                mut self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                bytes: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                self.0.push(bytes.to_vec());
                std::task::Poll::Ready(Ok(bytes.len()))
            }
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        }
        for (requested, expected) in [(20, 20), (182, 182), (514, 512)] {
            let bytes: Vec<u8> = (0..expected * 2 + 17)
                .map(|i| (i * 19 % 251) as u8)
                .collect();
            let (mut source, rd) = duplex(bytes.len());
            source.write_all(&bytes).await.unwrap();
            drop(source);
            let mut capture = Capture(vec![]);
            pump(rd, &mut capture, requested, &Mutex::new(Instant::now()))
                .await
                .unwrap();
            assert_eq!(
                capture.0.iter().map(Vec::len).collect::<Vec<_>>(),
                [expected, expected, 17]
            );
            assert_eq!(capture.0.concat(), bytes);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_writer_times_out_and_eof_finishes() {
        let (mut source, rd) = duplex(64);
        let (wr, mut sink) = duplex(20);
        source.write_all(&[7; 41]).await.unwrap();
        let task = tokio::spawn(async move { pump(rd, wr, 20, &Mutex::new(Instant::now())).await });
        let mut first = [0; 20];
        sink.read_exact(&mut first).await.unwrap();
        assert_eq!(first, [7; 20]);
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("timed out"));
        let (source, rd) = duplex(20);
        drop(source);
        let (wr, _sink) = duplex(20);
        pump(rd, wr, 20, &Mutex::new(Instant::now())).await.unwrap();
    }

    #[tokio::test]
    async fn bounded_chunks_multihop_repair_ack_and_disconnect() {
        for payload in [20, 182, 512] {
            let (a, mut ae) = Mesh::start(MeshConfig::default()).await.unwrap();
            let (r, mut re) = Mesh::start(MeshConfig::default()).await.unwrap();
            let (b, mut be) = Mesh::start(MeshConfig::default()).await.unwrap();
            let connect = |left: Mesh, right: Mesh| {
                let (l, rr) = duplex(payload);
                let (lr, lw) = tokio::io::split(l);
                let (rr, rw) = tokio::io::split(rr);
                tokio::spawn(async move {
                    tokio::join!(
                        attach(
                            &left,
                            lr,
                            lw,
                            payload,
                            true,
                            Arc::new(Mutex::new(Instant::now()))
                        ),
                        attach(
                            &right,
                            rr,
                            rw,
                            payload,
                            false,
                            Arc::new(Mutex::new(Instant::now()))
                        )
                    )
                })
            };
            let ar = connect(a.clone(), r.clone());
            timeout(Duration::from_secs(5), async {
                while !matches!(ae.recv().await, Some(MeshEvent::NeighborUp(_))) {}
            })
            .await
            .unwrap();
            assert_eq!(a.neighbor_links(), vec![(r.peer_id(), LinkKind::Ble)]);
            let bytes: Vec<u8> = (0..20_123).map(|i| (i * 17 % 251) as u8).collect();
            let id = a.send(b.peer_id(), bytes.clone()).await.unwrap();
            timeout(Duration::from_secs(5), async {
                while r.stats().stored == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let rb = connect(r.clone(), b.clone());
            timeout(Duration::from_secs(5), async {
                loop {
                    if let Some(MeshEvent::Message { payload, msg_id, hops, .. }) = be.recv().await {
                        assert_eq!(payload, bytes);
                        assert_eq!(msg_id, id);
                        assert_eq!(hops, 2);
                        break;
                    }
                }
                loop { if matches!(ae.recv().await, Some(MeshEvent::Delivered { msg_id, .. }) if msg_id == id) { break; } }
            }).await.unwrap();
            ar.abort();
            rb.abort();
            let _ = ar.await;
            let _ = rb.await;
            timeout(Duration::from_secs(5), async {
                while !matches!(re.recv().await, Some(MeshEvent::NeighborDown(_))) {}
            })
            .await
            .unwrap();
            assert!(a.neighbors().is_empty());
            assert!(b.neighbors().is_empty());
            a.shutdown();
            r.shutdown();
            b.shutdown();
        }
    }
}
