# Switcher Rust

A dead simple, minimal Rust CLI for Switcher Power Plug devices. No bloat, just working code.

## Features

### 🔍 Discovery & Detection

- **Smart Discovery**: UDP broadcast on the correct port (10002) with intelligent caching
- **Cache System**: Automatic device caching with configurable timeouts (default: 1 hour)
- **Cache-Only Mode**: Skip network scanning and use cached devices only
- **Auto-Cleanup**: Old devices automatically removed from cache

### 🔗 Device Pairing & Management

- **Device Pairing**: Pair devices with friendly aliases for easy management
- **Alias Control**: Control devices by memorable names instead of IPs/IDs
- **Persistent Storage**: Paired devices saved in unified configuration
- **Device Renaming**: Update device names directly on the device
- **Pairing Management**: List, add, and remove paired devices

### 🎛️ Device Control

- **Power Control**: Turn devices on/off via IP, device ID, or alias
- **Real-time Status**: Check device state and power consumption
- **Multiple Control Methods**: Support for IP address, device ID, or paired alias
- **Instant Feedback**: Clear status indicators and error messages

### 💾 Configuration & Storage

- **Unified Config**: Single `switcher_config.json` file for all settings
- **Version Safety**: Config cleared automatically when tool version changes
- **Last Seen Tracking**: Track when paired devices were last discovered
- **Auto-Migration**: Seamless config updates between versions

### 🎯 Device Compatibility

- **Power Plug Focus**: Optimized for Switcher Power Plug devices (Type 01a8)
- **Protocol Compliance**: Proper Switcher protocol implementation
- **Network Discovery**: Automatic device detection on local network

## Quick Start

```bash
# Build
cargo build --release

# Find your device (with caching enabled by default)
./target/release/switcher-rust discover --timeout 3

# Quick discovery using cache only (no network scan)
./target/release/switcher-rust discover --cache-only

# Pair a device for easy control
./target/release/switcher-rust pair --device-id 9c4f22 --alias "Living Room Plug"

# Control by alias (no need to remember IP addresses!)
./target/release/switcher-rust on --alias "Living Room Plug"
./target/release/switcher-rust off --alias "Living Room Plug"
./target/release/switcher-rust status --alias "Living Room Plug"

# Or use traditional IP/device-id method
./target/release/switcher-rust on --ip 10.0.0.24 --device-id 9c4f22

# Manage paired devices
./target/release/switcher-rust list-paired
./target/release/switcher-rust unpair --alias "Living Room Plug"

# Clear device cache
./target/release/switcher-rust clear-cache
```

## How It Works

- **Discovery**: UDP broadcast on port 10002 (Power Plugs only)
- **Control**: TCP connection to port 9957 with CRC-signed hex packets
- **Authentication**: None! Any device can be controlled by anyone on the network (no device key needed)

## Device Caching

The tool automatically caches discovered devices to speed up subsequent operations:

- **Config Location**: `switcher_config.json` next to the executable
- **Default Timeout**: 1 hour (3600 seconds)
- **Auto-cleanup**: Old devices are automatically removed from cache
- **Version Safety**: Cache is cleared when tool version changes

### Cache Options

```bash
# Disable caching completely
./target/release/switcher-rust discover --no-cache

# Set custom cache timeout (in seconds)
./target/release/switcher-rust discover --cache-timeout 7200  # 2 hours

# Use only cached devices (no network scan)
./target/release/switcher-rust discover --cache-only

# Clear the cache
./target/release/switcher-rust clear-cache
./target/release/switcher-rust clear-cache --force  # No confirmation
```

## Device Pairing & IP Change Recovery

The most powerful feature! Pair devices once, control them by alias forever - even after power outages change their IP addresses.

### How Pairing Works

1. **Discover devices** to find their current IP and device ID
2. **Pair with a friendly alias** for easy identification
3. **Control by alias** - the tool automatically resolves current IP addresses
4. **Automatic IP updates** when devices are rediscovered

### Pairing Commands

```bash
# Discover and pair a device
./target/release/switcher-rust discover
./target/release/switcher-rust pair --device-id 9c4f22 --alias "Living Room Plug"

# List all paired devices
./target/release/switcher-rust list-paired
./target/release/switcher-rust list-paired --verbose  # Show detailed info

# Control paired devices by alias
./target/release/switcher-rust on --alias "Living Room Plug"
./target/release/switcher-rust off --alias "Living Room Plug"
./target/release/switcher-rust status --alias "Living Room Plug"

# Remove pairing
./target/release/switcher-rust unpair --alias "Living Room Plug"
./target/release/switcher-rust unpair --alias "Living Room Plug" --force  # No confirmation
```

### IP Change Recovery Scenarios

**Scenario 1: Power Outage → Router assigns new IP**

```bash
# Before outage: Device at 192.168.1.100
./target/release/switcher-rust on --name "Living Room Plug"  # ✅ Works

# After outage: Device now at 192.168.1.150 (new DHCP lease)
./target/release/switcher-rust on --alias "Living Room Plug"
# → 🔍 Detects IP change
# → 📍 Updates pairing: 192.168.1.100 → 192.168.1.150
# → ✅ Executes command successfully
```

**Scenario 2: Device temporarily offline**

```bash
./target/release/switcher-rust status --alias "Living Room Plug"
# → Tries last known IP (fails)
# → 🔍 Scans network for device ID
# → 📍 Updates IP if found
# → ✅ Retries command
# → ⚠️  Falls back to last known IP if device not found
```

**Scenario 3: Discovery shows pairing status**

```bash
./target/release/switcher-rust discover
# Output shows:
# 📱 Discovered 2 device(s):
#   • Living Room Plug (192.168.1.150) [PAIRED as 'Living Room Plug'] ✅
#     ID: 9c4f22, Key: a1, MAC: 00:11:22:33:44:55
#
#   • Kitchen Plug (192.168.1.151) [NOT PAIRED]
#     ID: 8b3e11, Key: b2, MAC: 00:11:22:33:44:66
#
# 💡 To pair unpaired devices:
#    switcher-rust pair --device-id 8b3e11 --alias "Kitchen Plug"
```

### Config

- **Location**: `switcher_config.json` next to executable (contains both cache and pairing data)
- **Persistence**: Paired devices remain until manually unpaired
- **Auto-updates**: IP addresses updated during discovery
- **Version safety**: Config cleared when tool version changes

## Testing

Run the comprehensive interactive test suite:

```bash
# Interactive test with device selection
cargo test comprehensive_test_suite -- --nocapture
```

The test will:

- ✅ Discover devices on your network
- 🤔 Ask permission before testing on real devices
- ✅ Test all functionality (discovery, status, control)
- ✅ Restore device state after testing
- ✅ Test error handling with fake devices

## Supported Devices

- ✅ Switcher Power Plug (Type `01a8`) - the common one

## Technical Details

- **CRC Signing**: Implements CRC-CCITT with 0x1021 initialization
- **Session Management**: Login with dummy key (any hex value works)
- **Packet Format**: Hex-encoded binary protocol with checksums
- **Timeouts**: Smart timeouts prevent hanging on network issues

## Limitations

- **Power Plugs only** - no other Switcher device types
- **Local network only** - no cloud/remote access
- **Rate limiting** - device may throttle rapid consecutive commands

## Acknowledgements

- [aioswitcher](https://github.com/TomerFi/aioswitcher) for helping me understand the protocol

## License

MIT License - Feel free to use and modify as needed.
