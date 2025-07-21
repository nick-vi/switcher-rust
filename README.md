# Switcher Rust

A dead simple, minimal Rust CLI for Switcher Power Plug devices. No bloat, just working code.

## Features

- 🔍 **Device Discovery**: UDP broadcast on the correct port (10002)
- 🟢 **Device Control**: Direct TCP commands (on/off)
- 📊 **Status Check**: Real-time device state and power consumption
- 🎯 **Power Plug Only**: Focused on Type 01a8 devices

## Quick Start

```bash
# Build
cargo build --release

# Find your device
./target/release/switcher-rust discover --timeout 3

# Control it (use IP and device ID from discovery)
./target/release/switcher-rust on --ip 10.0.0.24 --device-id 9c4f22
./target/release/switcher-rust off --ip 10.0.0.24 --device-id 9c4f22
./target/release/switcher-rust status --ip 10.0.0.24 --device-id 9c4f22
```

## How It Works

- **Discovery**: UDP broadcast on port 10002 (Power Plugs only)
- **Control**: TCP connection to port 9957 with CRC-signed hex packets
- **Authentication**: None! Any device can be controlled by anyone on the network (no device key needed)

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
