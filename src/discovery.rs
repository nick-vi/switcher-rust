use crate::cache::CacheManager;
use crate::device::SwitcherDevice;
use crate::pairing::PairingManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

pub struct SwitcherDiscovery {
    cache_manager: Option<CacheManager>,
    use_cache: bool,
    cache_max_age: u64, // seconds
}

impl SwitcherDiscovery {
    pub fn new() -> Self {
        Self {
            cache_manager: CacheManager::new().ok(),
            use_cache: true,
            cache_max_age: 3600, // 1 hour default
        }
    }

    pub fn with_cache_settings(use_cache: bool, cache_max_age: u64) -> Self {
        Self {
            cache_manager: if use_cache {
                CacheManager::new().ok()
            } else {
                None
            },
            use_cache,
            cache_max_age,
        }
    }

    pub fn without_cache() -> Self {
        Self {
            cache_manager: None,
            use_cache: false,
            cache_max_age: 0,
        }
    }

    /// Discover devices from cache only (no network scan)
    pub fn discover_from_cache_only(
        &self,
    ) -> Result<Vec<SwitcherDevice>, Box<dyn std::error::Error>> {
        if !self.use_cache {
            return Ok(Vec::new());
        }

        let cache_manager = self
            .cache_manager
            .as_ref()
            .ok_or("Cache manager not available")?;

        let cache = cache_manager.load_cache()?;
        let devices = cache.get_fresh_devices(self.cache_max_age);

        if !devices.is_empty() {
            println!("📦 Found {} cached device(s):", devices.len());
            for device in &devices {
                println!("  • {} ({})", device.name, device.ip_address);
            }
        }

        Ok(devices)
    }

    /// Discover devices with caching support
    pub async fn discover_with_cache(
        &self,
        duration: Duration,
    ) -> Result<Vec<SwitcherDevice>, Box<dyn std::error::Error>> {
        let mut all_devices = Vec::new();

        if self.use_cache {
            if let Some(cache_manager) = &self.cache_manager {
                match cache_manager.load_cache() {
                    Ok(cache) => {
                        let cached_devices = cache.get_fresh_devices(self.cache_max_age);
                        if !cached_devices.is_empty() {
                            println!(
                                "📦 Loaded {} fresh device(s) from cache",
                                cached_devices.len()
                            );
                            all_devices.extend(cached_devices);
                        }
                    }
                    Err(e) => {
                        println!("⚠️  Could not load cache: {}", e);
                    }
                }
            }
        }

        let discovered_devices = self.discover_network(duration).await?;
        let mut device_map: HashMap<String, SwitcherDevice> = HashMap::new();

        // Add cached devices first
        for device in all_devices {
            device_map.insert(device.device_id.clone(), device);
        }

        // Add/update with newly discovered devices
        for device in discovered_devices {
            device_map.insert(device.device_id.clone(), device);
        }

        let final_devices: Vec<SwitcherDevice> = device_map.into_values().collect();

        if self.use_cache {
            if let Some(cache_manager) = &self.cache_manager {
                match cache_manager.load_cache() {
                    Ok(mut cache) => {
                        for device in &final_devices {
                            cache.add_device(device.clone());
                        }

                        cache.remove_old_devices(self.cache_max_age * 2);

                        if let Err(e) = cache_manager.save_cache(&cache) {
                            println!("⚠️  Could not save cache: {}", e);
                        } else {
                            println!("💾 Updated device cache");
                        }
                    }
                    Err(e) => {
                        println!("⚠️  Could not update cache: {}", e);
                    }
                }
            }
        }

        // Update pairing data for discovered devices
        if let Ok(pairing_manager) = PairingManager::new() {
            match pairing_manager.load_pairing() {
                Ok(mut pairing) => {
                    let mut updated = false;

                    for device in &final_devices {
                        if pairing.update_device_info(device) {
                            updated = true;
                        }
                    }

                    if updated {
                        if let Err(e) = pairing_manager.save_pairing(&pairing) {
                            println!("⚠️  Could not update pairing data: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️  Could not load pairing data: {}", e);
                }
            }
        }

        Ok(final_devices)
    }

    pub async fn discover(
        &self,
        duration: Duration,
    ) -> Result<Vec<SwitcherDevice>, Box<dyn std::error::Error>> {
        if self.use_cache {
            self.discover_with_cache(duration).await
        } else {
            self.discover_network(duration).await
        }
    }

    /// Network-only discovery (no caching)
    pub async fn discover_network(
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
