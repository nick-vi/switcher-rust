use clap::{Parser, Subcommand};
use std::io::Write;
use tokio::time::Duration;

mod cache;
mod config;
mod control;
mod device;
mod discovery;
mod pairing;
mod utils;

use cache::CacheManager;
use control::SwitcherController;
use discovery::SwitcherDiscovery;
use pairing::PairingManager;
use utils::{current_timestamp, format_timestamp};

#[derive(Parser)]
#[command(name = "switcher-rust")]
#[command(about = "A simple Rust CLI for Switcher Power Plug devices")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Discover {
        #[arg(short, long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, help = "Disable device caching")]
        no_cache: bool,
        #[arg(
            long,
            default_value_t = 3600,
            help = "Cache timeout in seconds (default: 3600 = 1 hour)"
        )]
        cache_timeout: u64,
        #[arg(long, help = "Only use cached devices, don't scan network")]
        cache_only: bool,
    },
    On {
        #[arg(short, long, help = "Device IP address")]
        ip: Option<String>,
        #[arg(short, long, help = "Device ID")]
        device_id: Option<String>,
        #[arg(short, long, help = "Paired device alias")]
        alias: Option<String>,
    },
    Off {
        #[arg(short, long, help = "Device IP address")]
        ip: Option<String>,
        #[arg(short, long, help = "Device ID")]
        device_id: Option<String>,
        #[arg(short, long, help = "Paired device alias")]
        alias: Option<String>,
    },
    Status {
        #[arg(short, long, help = "Device IP address")]
        ip: Option<String>,
        #[arg(short, long, help = "Device ID")]
        device_id: Option<String>,
        #[arg(short, long, help = "Paired device alias")]
        alias: Option<String>,
    },
    ClearCache {
        #[arg(long, help = "Clear cache without confirmation")]
        force: bool,
    },
    Pair {
        #[arg(short, long, help = "Device ID to pair")]
        device_id: String,
        #[arg(short, long, help = "Friendly alias for the device")]
        alias: String,
    },
    Unpair {
        #[arg(short, long, help = "Alias of the paired device to remove")]
        alias: String,
        #[arg(long, help = "Remove without confirmation")]
        force: bool,
    },
    ListPaired {
        #[arg(long, help = "Show detailed information")]
        verbose: bool,
    },
    Rename {
        #[arg(short, long, help = "Device IP address")]
        ip: Option<String>,
        #[arg(short, long, help = "Device ID")]
        device_id: Option<String>,
        #[arg(short, long, help = "Paired device alias")]
        alias: Option<String>,
        #[arg(short, long, help = "New name for the device")]
        new_name: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Discover {
            timeout,
            no_cache,
            cache_timeout,
            cache_only,
        } => {
            let discovery = if no_cache {
                SwitcherDiscovery::without_cache()
            } else {
                SwitcherDiscovery::with_cache_settings(!no_cache, cache_timeout)
            };

            let devices = if cache_only {
                println!("📦 Loading devices from cache only...");
                discovery.discover_from_cache_only()?
            } else {
                println!("🔍 Discovering Switcher devices for {} seconds...", timeout);
                discovery.discover(Duration::from_secs(timeout)).await?
            };

            if devices.is_empty() {
                println!(
                    "❌ No devices found. Make sure your Switcher devices are on the same network."
                );
            } else {
                println!("\n📱 Discovered {} device(s):", devices.len());

                // Load pairing to check pairing status
                let pairing_manager = PairingManager::new().ok();
                let pairing = pairing_manager
                    .as_ref()
                    .and_then(|pm| pm.load_pairing().ok());

                let mut unpaired_devices = Vec::new();

                for device in &devices {
                    // Check if device is paired by looking in pairing config
                    let pairing_status = pairing
                        .as_ref()
                        .and_then(|p| p.devices.get(&device.device_id))
                        .map(|paired_device| format!("[PAIRED as '{}'] ✅", paired_device.alias))
                        .unwrap_or_else(|| {
                            unpaired_devices.push(device);
                            "[NOT PAIRED]".to_string()
                        });

                    println!(
                        "  • {} ({}) {}",
                        device.name, device.ip_address, pairing_status
                    );
                    println!(
                        "    ID: {}, Key: {}, MAC: {}",
                        device.device_id, device.device_key, device.mac_address
                    );
                    println!(
                        "    State: {:?}, Power: {}W",
                        device.state, device.power_consumption
                    );
                    println!();
                }

                // Show pairing suggestions for unpaired devices
                if !unpaired_devices.is_empty() {
                    println!("💡 To pair unpaired devices:");
                    for device in unpaired_devices {
                        println!(
                            "   switcher-rust pair --device-id {} --alias \"{}\"",
                            device.device_id, device.name
                        );
                    }
                    println!();
                }
            }
        }
        Commands::On {
            ip,
            device_id,
            alias,
        } => match resolve_device_info(ip, device_id, alias).await {
            Ok((resolved_ip, resolved_device_id)) => {
                let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                match controller.turn_on().await {
                    Ok(_) => println!("✅ Device turned ON"),
                    Err(e) => println!("❌ Failed to turn device on: {}", e),
                }
            }
            Err(e) => println!("❌ {}", e),
        },
        Commands::Off {
            ip,
            device_id,
            alias,
        } => match resolve_device_info(ip, device_id, alias).await {
            Ok((resolved_ip, resolved_device_id)) => {
                let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                match controller.turn_off().await {
                    Ok(_) => println!("✅ Device turned OFF"),
                    Err(e) => println!("❌ Failed to turn device off: {}", e),
                }
            }
            Err(e) => println!("❌ {}", e),
        },
        Commands::Status {
            ip,
            device_id,
            alias,
        } => match resolve_device_info(ip, device_id, alias).await {
            Ok((resolved_ip, resolved_device_id)) => {
                let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                match controller.get_status().await {
                    Ok(state) => {
                        println!("📊 Device Status:");
                        println!("  State: {:?}", state.state);
                        println!("  Power: {}W", state.power_consumption);
                    }
                    Err(e) => println!("❌ Failed to get status: {}", e),
                }
            }
            Err(e) => println!("❌ {}", e),
        },
        Commands::ClearCache { force } => {
            let cache_manager = CacheManager::new()?;

            if !cache_manager.cache_exists() {
                println!("ℹ️  No cache file found");
                return Ok(());
            }

            if !force {
                println!(
                    "⚠️  This will delete the cache file at: {}",
                    cache_manager.get_cache_path().display()
                );
                print!("Are you sure? (y/N): ");
                std::io::stdout().flush()?;

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if !input.trim().to_lowercase().starts_with('y') {
                    println!("❌ Cache clear cancelled");
                    return Ok(());
                }
            }

            match cache_manager.clear_cache() {
                Ok(()) => println!("✅ Cache cleared successfully"),
                Err(e) => println!("❌ Failed to clear cache: {}", e),
            }
        }
        Commands::Pair { device_id, alias } => {
            // First check if device exists in cache or discover it
            let cache_manager = CacheManager::new()?;
            let mut cache = cache_manager.load_cache()?;

            // Check if device exists in cache
            if !cache.devices.contains_key(&device_id) {
                // Device not in cache, need to discover it
                println!("🔍 Device not found in cache, discovering...");
                let discovery = SwitcherDiscovery::new();
                let devices = discovery.discover(Duration::from_secs(10)).await?;

                if !devices.iter().any(|d| d.device_id == device_id) {
                    println!("❌ Device with ID '{}' not found on network", device_id);
                    println!("   Make sure the device is powered on and connected");
                    return Ok(());
                }

                // Reload cache after discovery
                cache = cache_manager.load_cache()?;
            }

            // Get the device from cache
            let device = cache.devices.get(&device_id).unwrap().device.clone();

            // Now pair the device using pairing manager
            let pairing_manager = PairingManager::new()?;
            let mut pairing = pairing_manager.load_pairing()?;

            match pairing.pair_device(device.clone(), alias.clone()) {
                Ok(()) => {
                    pairing_manager.save_pairing(&pairing)?;

                    println!("✅ Device paired successfully!");
                    println!("   Device: {} ({})", device.name, device_id);
                    println!("   Alias: {}", alias);
                    println!("   IP: {}", device.ip_address);
                }
                Err(e) => println!("❌ {}", e),
            }
        }
        Commands::Unpair { alias, force } => {
            let pairing_manager = PairingManager::new()?;
            let mut pairing = pairing_manager.load_pairing()?;

            // Check if device exists
            let device = match pairing.get_device_by_alias(&alias) {
                Some(device) => device.clone(),
                None => {
                    println!("❌ No paired device found with alias '{}'", alias);
                    return Ok(());
                }
            };

            if !force {
                println!(
                    "⚠️  This will unpair device: {} ({})",
                    alias, device.device.device_id
                );
                print!("Are you sure? (y/N): ");
                std::io::stdout().flush()?;

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if !input.trim().to_lowercase().starts_with('y') {
                    println!("❌ Unpair cancelled");
                    return Ok(());
                }
            }

            match pairing.unpair_device(&alias) {
                Ok(()) => {
                    pairing_manager.save_pairing(&pairing)?;
                    println!("✅ Device '{}' unpaired successfully", alias);
                }
                Err(e) => println!("❌ Failed to unpair device: {}", e),
            }
        }
        Commands::ListPaired { verbose } => {
            let pairing_manager = PairingManager::new()?;
            let pairing = pairing_manager.load_pairing()?;

            let paired_devices = pairing.get_paired_devices();

            if paired_devices.is_empty() {
                println!("📱 No paired devices found");
                println!("   Use 'pair --device-id <id> --alias <alias>' to pair a device");
                return Ok(());
            }

            println!("📱 Paired devices ({}):", paired_devices.len());

            for device in paired_devices {
                let recently_seen = (current_timestamp() - device.last_seen) < 3600; // 1 hour
                let status_icon = if recently_seen { "🟢" } else { "🔴" };

                println!(
                    "  {} {} ({})",
                    status_icon, device.alias, device.device.ip_address
                );

                if verbose {
                    println!("     Device ID: {}", device.device.device_id);
                    println!("     MAC: {}", device.device.mac_address);
                    println!("     Type: {}", device.device.device_type);
                    println!("     Paired: {}", format_timestamp(device.paired_at));
                    println!("     Last seen: {}", format_timestamp(device.last_seen));
                    println!();
                }
            }

            if !verbose {
                println!("   Use --verbose for detailed information");
            }
        }
        Commands::Rename {
            ip,
            device_id,
            alias,
            new_name,
        } => match resolve_device_info(ip, device_id, alias).await {
            Ok((resolved_ip, resolved_device_id)) => {
                let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                match controller.set_device_name(&new_name).await {
                    Ok(_) => {
                        println!("✅ Device name changed to '{}'", new_name);
                        println!("   Note: It may take a few moments for the change to appear in discovery");
                    }
                    Err(e) => println!("❌ Failed to change device name: {}", e),
                }
            }
            Err(e) => println!("❌ {}", e),
        },
    }

    Ok(())
}

/// Resolve device IP and ID from either direct parameters or paired device alias
async fn resolve_device_info(
    ip: Option<String>,
    device_id: Option<String>,
    alias: Option<String>,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    match (ip, device_id, alias) {
        // Direct IP and device ID provided
        (Some(ip), Some(device_id), None) => Ok((ip, device_id)),

        // Paired device alias provided
        (None, None, Some(alias)) => {
            let pairing_manager = PairingManager::new()?;
            let pairing = pairing_manager.load_pairing()?;

            let paired_device = pairing.get_device_by_alias(&alias)
                .ok_or_else(|| format!("No paired device found with alias '{}'", alias))?;

            // Use the current IP from cache
            Ok((paired_device.device.ip_address.clone(), paired_device.device.device_id.clone()))
        }

        // Invalid combinations
        (Some(_), Some(_), Some(_)) => {
            Err("Cannot specify both IP/device-id and alias. Use either --ip and --device-id, or --alias.".into())
        }
        (Some(_), None, None) | (None, Some(_), None) => {
            Err("When using IP/device-id, both --ip and --device-id are required.".into())
        }
        (None, None, None) => {
            Err("Must specify either --ip and --device-id, or --alias for a paired device.".into())
        }
        _ => {
            Err("Invalid parameter combination. Use either --ip and --device-id, or --alias.".into())
        }
    }
}
