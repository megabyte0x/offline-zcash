//! BLE moves an ordered byte stream only; mesh framing, repair and acks remain in ozm-mesh.
//! Platform backends live in separate modules; non-Linux targets fail explicitly for now.

use anyhow::Result;
use ozm_mesh::Mesh;

pub const SERVICE: uuid::Uuid = uuid::Uuid::from_u128(0xb91d6e31_89f5_4a38_9761_72d1305a4f12);
pub const STREAM: uuid::Uuid = uuid::Uuid::from_u128(0xb91d6e32_89f5_4a38_9761_72d1305a4f12);

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", test))]
mod stream;

/// Owns scanning, advertising, GATT registration and all BLE links. Drop to stop.
pub struct Ble {
    #[cfg(target_os = "linux")]
    task: tokio::task::JoinHandle<()>,
}

impl Ble {
    pub async fn start(mesh: Mesh, fleet_hint: [u8; 4]) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            linux::start(mesh, fleet_hint).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (mesh, fleet_hint);
            anyhow::bail!("Bluetooth is supported only on Linux; use TCP/LAN on this platform")
        }
    }

    pub fn is_running(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            !self.task.is_finished()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}

impl Drop for Ble {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        self.task.abort();
    }
}
