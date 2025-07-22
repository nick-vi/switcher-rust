use crate::config::ConfigManager;
use crate::device::SwitcherDevice;
use crate::utils::current_timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cached device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDevice {
    pub device: SwitcherDevice,
    pub last_seen: u64,
    pub discovery_count: u32,
}

/// Device cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCache {
    pub devices: HashMap<String, CachedDevice>, // device_id -> CachedDevice
    pub last_updated: u64,
}

impl DeviceCache {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            last_updated: current_timestamp(),
        }
    }

    pub fn add_device(&mut self, device: SwitcherDevice) {
        let now = current_timestamp();
        let device_id = &device.device_id;

        if let Some(cached) = self.devices.get_mut(device_id) {
            cached.device = device;
            cached.last_seen = now;
            cached.discovery_count += 1;
        } else {
            self.devices.insert(
                device.device_id.clone(),
                CachedDevice {
                    device,
                    last_seen: now,
                    discovery_count: 1,
                },
            );
        }
        self.last_updated = now;
    }

    pub fn get_fresh_devices(&self, max_age_seconds: u64) -> Vec<SwitcherDevice> {
        let now = current_timestamp();
        let cutoff = now.saturating_sub(max_age_seconds);

        self.devices
            .values()
            .filter(|cached| cached.last_seen >= cutoff)
            .map(|cached| cached.device.clone())
            .collect()
    }

    pub fn remove_old_devices(&mut self, max_age_seconds: u64) {
        let now = current_timestamp();
        let cutoff = now.saturating_sub(max_age_seconds);

        self.devices.retain(|_, cached| cached.last_seen >= cutoff);
        self.last_updated = now;
    }
}

pub struct CacheManager {
    config_manager: ConfigManager,
}

impl CacheManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config_manager = ConfigManager::new()?;
        Ok(Self { config_manager })
    }

    pub fn load_cache(&self) -> Result<DeviceCache, Box<dyn std::error::Error>> {
        self.config_manager.load_cache_data()
    }

    pub fn save_cache(&self, cache: &DeviceCache) -> Result<(), Box<dyn std::error::Error>> {
        self.config_manager.save_cache_data(cache)
    }

    pub fn clear_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.config_manager.clear_config()
    }

    pub fn cache_exists(&self) -> bool {
        self.config_manager.config_exists()
    }

    pub fn get_cache_path(&self) -> &std::path::Path {
        self.config_manager.get_config_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DeviceState, SwitcherDevice};

    fn create_test_device(id: &str, name: &str, ip: &str) -> SwitcherDevice {
        SwitcherDevice {
            device_id: id.to_string(),
            name: name.to_string(),
            ip_address: ip.to_string(),
            mac_address: "00:11:22:33:44:55".to_string(),
            device_key: "a1".to_string(),
            device_type: "Switcher Power Plug".to_string(),
            state: DeviceState::Off,
            power_consumption: 0,
        }
    }

    #[test]
    fn test_cache_add_device() {
        let mut cache = DeviceCache::new();
        let device = create_test_device("123", "Test Device", "192.168.1.100");

        cache.add_device(device.clone());

        assert_eq!(cache.devices.len(), 1);
        assert!(cache.devices.contains_key("123"));
    }

    #[test]
    fn test_cache_fresh_devices() {
        let mut cache = DeviceCache::new();
        let device = create_test_device("123", "Test Device", "192.168.1.100");

        cache.add_device(device);

        // Should be fresh within 1 hour
        let fresh = cache.get_fresh_devices(3600);
        assert_eq!(fresh.len(), 1);

        // Manually set an old timestamp to test freshness
        if let Some(cached_device) = cache.devices.get_mut("123") {
            cached_device.last_seen = current_timestamp() - 10; // 10 seconds ago
        }

        // Should not be fresh if we set max age to 5 seconds
        let not_fresh = cache.get_fresh_devices(5);
        assert_eq!(not_fresh.len(), 0);

        // Should still be fresh if we set max age to 15 seconds
        let still_fresh = cache.get_fresh_devices(15);
        assert_eq!(still_fresh.len(), 1);
    }

    #[test]
    fn test_cache_remove_old_devices() {
        let mut cache = DeviceCache::new();
        let device = create_test_device("123", "Test Device", "192.168.1.100");

        cache.add_device(device);
        assert_eq!(cache.devices.len(), 1);

        // Manually set an old timestamp
        if let Some(cached_device) = cache.devices.get_mut("123") {
            cached_device.last_seen = current_timestamp() - 100; // 100 seconds ago
        }

        // Remove devices older than 50 seconds (should remove the device)
        cache.remove_old_devices(50);
        assert_eq!(cache.devices.len(), 0);
    }
}
