use clap::{Parser, Subcommand};
use tokio::time::Duration;

mod control;
mod device;
mod discovery;

use control::SwitcherController;
use discovery::SwitcherDiscovery;

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
    },
    On {
        #[arg(short, long)]
        ip: String,
        #[arg(short, long)]
        device_id: String,
    },
    Off {
        #[arg(short, long)]
        ip: String,
        #[arg(short, long)]
        device_id: String,
    },
    Status {
        #[arg(short, long)]
        ip: String,
        #[arg(short, long)]
        device_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Discover { timeout } => {
            println!("🔍 Discovering Switcher devices for {} seconds...", timeout);

            let discovery = SwitcherDiscovery::new();
            let devices = discovery.discover(Duration::from_secs(timeout)).await?;

            if devices.is_empty() {
                println!(
                    "❌ No devices found. Make sure your Switcher devices are on the same network."
                );
            } else {
                println!("\n📱 Discovered {} device(s):", devices.len());
                for device in devices {
                    println!("  • {} ({})", device.name, device.ip_address);
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
            }
        }
        Commands::On { ip, device_id } => {
            let controller = SwitcherController::new(ip, device_id);
            match controller.turn_on().await {
                Ok(_) => println!("✅ Device turned ON"),
                Err(e) => println!("❌ Failed to turn device on: {}", e),
            }
        }
        Commands::Off { ip, device_id } => {
            let controller = SwitcherController::new(ip, device_id);
            match controller.turn_off().await {
                Ok(_) => println!("✅ Device turned OFF"),
                Err(e) => println!("❌ Failed to turn device off: {}", e),
            }
        }
        Commands::Status { ip, device_id } => {
            let controller = SwitcherController::new(ip, device_id);
            match controller.get_status().await {
                Ok(state) => {
                    println!("📊 Device Status:");
                    println!("  State: {:?}", state.state);
                    println!("  Power: {}W", state.power_consumption);
                }
                Err(e) => println!("❌ Failed to get status: {}", e),
            }
        }
    }

    Ok(())
}
