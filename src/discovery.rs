use crate::device::SwitcherDevice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

pub struct SwitcherDiscovery;

impl SwitcherDiscovery {
    pub fn new() -> Self {
        Self
    }

    pub async fn discover(
        &self,
        duration: Duration,
    ) -> Result<Vec<SwitcherDevice>, Box<dyn std::error::Error>> {
        let discovered_devices = Arc::new(Mutex::new(HashMap::new()));

        // Power Plug devices broadcast on port 10002 only
        let socket = UdpSocket::bind("0.0.0.0:10002").await?;
        socket.set_broadcast(true)?;
        println!("🔍 Listening for Power Plug devices...");

        let devices_clone = Arc::clone(&discovered_devices);
        let handle = tokio::spawn(async move {
            let mut buf = [0; 1024];

            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, _addr)) => {
                        if let Some(device) = SwitcherDevice::from_discovery_packet(&buf[..len]) {
                            let mut devices = devices_clone.lock().unwrap();
                            if !devices.contains_key(&device.device_id) {
                                println!(
                                    "📱 Found Power Plug: {} at {}",
                                    device.name, device.ip_address
                                );
                                devices.insert(device.device_id.clone(), device);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        sleep(duration).await;
        handle.abort();

        let devices = discovered_devices.lock().unwrap();
        Ok(devices.values().cloned().collect())
    }
}
