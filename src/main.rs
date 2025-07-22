use clap::{Parser, Subcommand};
use log::{debug, error, info};
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

    #[arg(short, long, global = true, help = "Enable verbose logging")]
    verbose: bool,

    #[arg(long, global = true, help = "Enable debug logging")]
    debug: bool,
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

    // Initialize logging based on CLI flags
    init_logging(cli.verbose, cli.debug);

    info!("Starting switcher-rust CLI");
    debug!(
        "CLI arguments parsed: verbose={}, debug={}",
        cli.verbose, cli.debug
    );

    match cli.command {
        Commands::Discover {
            timeout,
            no_cache,
            cache_timeout,
            cache_only,
        } => {
            info!("Starting device discovery - timeout: {}s, no_cache: {}, cache_timeout: {}s, cache_only: {}",
                  timeout, no_cache, cache_timeout, cache_only);

            let discovery = if no_cache {
                debug!("Creating discovery instance without cache");
                SwitcherDiscovery::without_cache()
            } else {
                debug!(
                    "Creating discovery instance with cache settings - use_cache: {}, timeout: {}s",
                    !no_cache, cache_timeout
                );
                SwitcherDiscovery::with_cache_settings(!no_cache, cache_timeout)
            };

            let devices = if cache_only {
                info!("Attempting cache-only discovery");
                discovery.discover_from_cache_only()?
            } else {
                info!("Starting network discovery for {} seconds", timeout);
                discovery.discover(Duration::from_secs(timeout)).await?
            };

            info!("Discovery completed - found {} devices", devices.len());

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
        } => {
            info!(
                "Turning device ON - ip: {:?}, device_id: {:?}, alias: {:?}",
                ip, device_id, alias
            );
            match resolve_device_info(ip, device_id, alias).await {
                Ok((resolved_ip, resolved_device_id)) => {
                    debug!(
                        "Resolved device info - ip: {}, device_id: {}",
                        resolved_ip, resolved_device_id
                    );
                    let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                    match controller.turn_on().await {
                        Ok(_) => {
                            info!("Successfully turned device ON");
                            println!("✅ Device turned ON");
                        }
                        Err(e) => {
                            error!("Failed to turn device on: {}", e);
                            println!("❌ Failed to turn device on: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to resolve device info: {}", e);
                    println!("❌ {}", e);
                }
            }
        }
        Commands::Off {
            ip,
            device_id,
            alias,
        } => {
            info!(
                "Turning device OFF - ip: {:?}, device_id: {:?}, alias: {:?}",
                ip, device_id, alias
            );
            match resolve_device_info(ip, device_id, alias).await {
                Ok((resolved_ip, resolved_device_id)) => {
                    debug!(
                        "Resolved device info - ip: {}, device_id: {}",
                        resolved_ip, resolved_device_id
                    );
                    let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                    match controller.turn_off().await {
                        Ok(_) => {
                            info!("Successfully turned device OFF");
                            println!("✅ Device turned OFF");
                        }
                        Err(e) => {
                            error!("Failed to turn device off: {}", e);
                            println!("❌ Failed to turn device off: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to resolve device info: {}", e);
                    println!("❌ {}", e);
                }
            }
        }
        Commands::Status {
            ip,
            device_id,
            alias,
        } => {
            info!(
                "Getting device status - ip: {:?}, device_id: {:?}, alias: {:?}",
                ip, device_id, alias
            );
            match resolve_device_info(ip, device_id, alias).await {
                Ok((resolved_ip, resolved_device_id)) => {
                    debug!(
                        "Resolved device info - ip: {}, device_id: {}",
                        resolved_ip, resolved_device_id
                    );
                    let controller = SwitcherController::new(resolved_ip, resolved_device_id);
                    match controller.get_status().await {
                        Ok(state) => {
                            info!(
                                "Successfully retrieved device status - state: {:?}, power: {}W",
                                state.state, state.power_consumption
                            );
                            println!("📊 Device Status:");
                            println!("  State: {:?}", state.state);
                            println!("  Power: {}W", state.power_consumption);
                        }
                        Err(e) => {
                            error!("Failed to get device status: {}", e);
                            println!("❌ Failed to get status: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to resolve device info: {}", e);
                    println!("❌ {}", e);
                }
            }
        }
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
            info!(
                "Pairing device - device_id: {}, alias: {}",
                device_id, alias
            );
            // First check if device exists in cache or discover it
            let cache_manager = CacheManager::new()?;
            let mut cache = cache_manager.load_cache()?;

            // Check if device exists in cache
            if !cache.devices.contains_key(&device_id) {
                // Device not in cache, need to discover it
                info!(
                    "Device {} not found in cache, starting discovery",
                    device_id
                );
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
                    info!(
                        "Successfully paired device {} with alias '{}'",
                        device_id, alias
                    );

                    println!("✅ Device paired successfully!");
                    println!("   Device: {} ({})", device.name, device_id);
                    println!("   Alias: {}", alias);
                    println!("   IP: {}", device.ip_address);
                }
                Err(e) => {
                    error!("Failed to pair device {}: {}", device_id, e);
                    println!("❌ {}", e);
                }
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

/// Initialize logging based on CLI flags and environment variables
fn init_logging(verbose: bool, debug: bool) {
    use std::path::PathBuf;
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    // Determine log level based on flags
    let log_level = if debug {
        "debug"
    } else if verbose {
        "info"
    } else {
        "warn"
    };

    // Create log directory next to executable
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let log_dir = exe_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    // Create file appender with daily rotation
    let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "switcher-rust.log");

    // Create console layer
    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false);

    // Create file layer
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(false); // No ANSI colors in log files

    // Create filter
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "switcher_rust={},switcher-rust={}",
            log_level, log_level
        ))
    });

    // Initialize tracing subscriber with both console and file output
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .init();
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
        (None, Some(_), Some(_)) | (Some(_), None, Some(_)) => {
            Err("Cannot mix IP/device-id with alias. Use either --ip and --device-id, or --alias.".into())
        }
    }
}
