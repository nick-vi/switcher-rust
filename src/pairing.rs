use crate::config::ConfigManager;
use crate::device::SwitcherDevice;
use crate::utils::current_timestamp;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub device: SwitcherDevice,
    pub alias: String,
    pub paired_at: u64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingConfig {
    pub devices: HashMap<String, PairedDevice>, // device_id -> PairedDevice
    pub aliases: HashMap<String, String>,       // alias -> device_id
    pub last_updated: u64,
}

impl PairingConfig {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            aliases: HashMap::new(),
            last_updated: current_timestamp(),
        }
    }

    pub fn pair_device(&mut self, device: SwitcherDevice, alias: String) -> Result<(), String> {
        debug!(
            "Attempting to pair device {} with alias '{}'",
            device.device_id, alias
        );

        if self.aliases.contains_key(&alias) {
            warn!("Pairing failed: alias '{}' is already in use", alias);
            return Err(format!("Alias '{}' is already in use", alias));
        }

        let device_id = device.device_id.clone();

        // Remove old pairing if device was already paired
        if let Some(old_paired) = self.devices.get(&device_id) {
            info!(
                "Removing old pairing for device {}: alias '{}'",
                device_id, old_paired.alias
            );
            self.aliases.remove(&old_paired.alias);
        }

        let paired_device = PairedDevice {
            device,
            alias: alias.clone(),
            paired_at: current_timestamp(),
            last_seen: current_timestamp(),
        };

        self.devices.insert(device_id.clone(), paired_device);
        self.aliases.insert(alias.clone(), device_id.clone());
        self.last_updated = current_timestamp();

        info!(
            "Successfully paired device {} with alias '{}'",
            device_id, alias
        );
        Ok(())
    }

    pub fn unpair_device(&mut self, alias: &str) -> Result<(), String> {
        debug!("Attempting to unpair device with alias '{}'", alias);

        let device_id = self
            .aliases
            .get(alias)
            .ok_or_else(|| {
                warn!("Unpair failed: no device found with alias '{}'", alias);
                format!("No device found with alias '{}'", alias)
            })?
            .clone();

        self.devices.remove(&device_id);
        self.aliases.remove(alias);
        self.last_updated = current_timestamp();

        info!(
            "Successfully unpaired device {} (alias: '{}')",
            device_id, alias
        );
        Ok(())
    }

    pub fn get_device_by_alias(&self, alias: &str) -> Option<&PairedDevice> {
        let device_id = self.aliases.get(alias)?;
        self.devices.get(device_id)
    }

    pub fn get_paired_devices(&self) -> Vec<&PairedDevice> {
        self.devices.values().collect()
    }

    /// Update device information and last_seen timestamp for a paired device
    pub fn update_device_info(&mut self, device: &SwitcherDevice) -> bool {
        if let Some(paired_device) = self.devices.get_mut(&device.device_id) {
            paired_device.device = device.clone();
            paired_device.last_seen = current_timestamp();
            self.last_updated = current_timestamp();
            true
        } else {
            false
        }
    }
}

pub struct PairingManager {
    config_manager: ConfigManager,
}

impl PairingManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config_manager = ConfigManager::new()?;
        Ok(Self { config_manager })
    }

    pub fn load_pairing(&self) -> Result<PairingConfig, Box<dyn std::error::Error>> {
        debug!("Loading pairing configuration");
        self.config_manager.load_pairing_data()
    }

    pub fn save_pairing(&self, pairing: &PairingConfig) -> Result<(), Box<dyn std::error::Error>> {
        debug!(
            "Saving pairing configuration with {} devices",
            pairing.devices.len()
        );
        self.config_manager.save_pairing_data(pairing)
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
    fn test_pair_device() {
        let mut pairing = PairingConfig::new();
        let device = create_test_device("123", "Test Device", "192.168.1.100");

        let result = pairing.pair_device(device, "Test Alias".to_string());
        assert!(result.is_ok());
        assert_eq!(pairing.devices.len(), 1);
        assert_eq!(pairing.aliases.len(), 1);
        assert!(pairing.aliases.contains_key("Test Alias"));
    }

    #[test]
    fn test_duplicate_alias() {
        let mut pairing = PairingConfig::new();
        let device1 = create_test_device("123", "Test Device 1", "192.168.1.100");
        let device2 = create_test_device("456", "Test Device 2", "192.168.1.101");

        pairing
            .pair_device(device1, "Test Alias".to_string())
            .unwrap();
        let result = pairing.pair_device(device2, "Test Alias".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_unpair_device() {
        let mut pairing = PairingConfig::new();
        let device = create_test_device("123", "Test Device", "192.168.1.100");

        pairing
            .pair_device(device, "Test Alias".to_string())
            .unwrap();
        assert_eq!(pairing.devices.len(), 1);

        let result = pairing.unpair_device("Test Alias");
        assert!(result.is_ok());
        assert_eq!(pairing.devices.len(), 0);
        assert_eq!(pairing.aliases.len(), 0);
    }
}
