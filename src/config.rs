use crate::cache::DeviceCache;
use crate::pairing::PairingConfig;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Unified configuration structure that contains both cache and pairing data
#[derive(Debug, Serialize, Deserialize)]
pub struct UnifiedConfig {
    pub cache: Option<DeviceCache>,
    pub pairing: Option<PairingConfig>,
    pub version: String,
}

impl UnifiedConfig {
    pub fn new() -> Self {
        Self {
            cache: None,
            pairing: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

pub struct ConfigManager {
    config_file_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config_file_path = Self::get_config_file_path()?;
        Ok(Self { config_file_path })
    }

    fn get_config_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let exe_path = std::env::current_exe()?;
        let exe_dir = exe_path
            .parent()
            .ok_or("Could not determine executable directory")?;
        Ok(exe_dir.join("switcher_config.json"))
    }

    pub fn clear_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.config_file_path.exists() {
            fs::remove_file(&self.config_file_path)?;
        }
        Ok(())
    }

    pub fn config_exists(&self) -> bool {
        self.config_file_path.exists()
    }

    pub fn get_config_path(&self) -> &Path {
        &self.config_file_path
    }

    /// Load the unified config, creating a new one if it doesn't exist
    pub fn load_unified_config(&self) -> Result<UnifiedConfig, Box<dyn std::error::Error>> {
        debug!(
            "Loading unified config from: {}",
            self.config_file_path.display()
        );

        if !self.config_file_path.exists() {
            debug!("Config file does not exist, creating new config");
            return Ok(UnifiedConfig::new());
        }

        let content = fs::read_to_string(&self.config_file_path)?;
        let config: UnifiedConfig = serde_json::from_str(&content)?;
        debug!(
            "Successfully loaded config with version: {}",
            config.version
        );

        // Check version compatibility
        if config.version != env!("CARGO_PKG_VERSION") {
            warn!(
                "Config version mismatch (found: {}, expected: {}), starting fresh",
                config.version,
                env!("CARGO_PKG_VERSION")
            );
            return Ok(UnifiedConfig::new());
        }

        Ok(config)
    }

    /// Save the unified config
    pub fn save_unified_config(
        &self,
        config: &UnifiedConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!(
            "Saving unified config to: {}",
            self.config_file_path.display()
        );
        let content = serde_json::to_string_pretty(config)?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.config_file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.config_file_path, content)?;
        debug!("Successfully saved unified config");
        Ok(())
    }

    /// Load cache data from the unified config
    pub fn load_cache_data(&self) -> Result<DeviceCache, Box<dyn std::error::Error>> {
        let config = self.load_unified_config()?;
        Ok(config.cache.unwrap_or_else(DeviceCache::new))
    }

    /// Save cache data to the unified config
    pub fn save_cache_data(&self, cache: &DeviceCache) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load_unified_config()?;
        config.cache = Some(cache.clone());
        self.save_unified_config(&config)
    }

    /// Load pairing data from the unified config
    pub fn load_pairing_data(&self) -> Result<PairingConfig, Box<dyn std::error::Error>> {
        let config = self.load_unified_config()?;
        Ok(config.pairing.unwrap_or_else(PairingConfig::new))
    }

    /// Save pairing data to the unified config
    pub fn save_pairing_data(
        &self,
        pairing: &PairingConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load_unified_config()?;
        config.pairing = Some(pairing.clone());
        self.save_unified_config(&config)
    }
}
