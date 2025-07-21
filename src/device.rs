use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitcherDevice {
    pub device_id: String,
    pub device_key: String,
    pub ip_address: String,
    pub mac_address: String,
    pub name: String,
    pub device_type: String,
    pub state: DeviceState,
    pub power_consumption: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DeviceState {
    On,
    Off,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DeviceStatus {
    pub state: DeviceState,
    pub power_consumption: u16,
}

impl SwitcherDevice {
    pub fn from_discovery_packet(data: &[u8]) -> Option<Self> {
        if data.len() != 165 || &data[0..2] != &[0xfe, 0xf0] {
            return None;
        }

        let hex_data = hex::encode(data);

        let device_id = hex::encode(&data[18..21]);
        let device_key = hex::encode(&data[40..41]);

        let name_bytes = &data[42..74];
        let name_end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(name_bytes.len());
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let device_type_hex = hex::encode(&data[74..76]);
        // Only accept Power Plug devices (01a8)
        if device_type_hex != "01a8" {
            return None;
        }
        let device_type = "Switcher Power Plug".to_string();

        // IP address from hex positions 152:160 (aioswitcher protocol)
        if hex_data.len() < 160 {
            return None;
        }
        let hex_ip = &hex_data[152..160];
        let ip_addr = u32::from_str_radix(
            &format!(
                "{}{}{}{}",
                &hex_ip[6..8],
                &hex_ip[4..6],
                &hex_ip[2..4],
                &hex_ip[0..2]
            ),
            16,
        )
        .ok()?;
        let ip_address = format!(
            "{}.{}.{}.{}",
            ip_addr & 0xFF,
            (ip_addr >> 8) & 0xFF,
            (ip_addr >> 16) & 0xFF,
            (ip_addr >> 24) & 0xFF
        );

        // MAC address (hex positions 160:172 in hex representation)
        if hex_data.len() < 278 {
            return None;
        }
        let hex_mac = &hex_data[160..172];
        let mac_address = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            u8::from_str_radix(&hex_mac[0..2], 16).ok()?,
            u8::from_str_radix(&hex_mac[2..4], 16).ok()?,
            u8::from_str_radix(&hex_mac[4..6], 16).ok()?,
            u8::from_str_radix(&hex_mac[6..8], 16).ok()?,
            u8::from_str_radix(&hex_mac[8..10], 16).ok()?,
            u8::from_str_radix(&hex_mac[10..12], 16).ok()?
        );

        // Device state (hex positions 266:268 in hex representation)
        let hex_device_state = &hex_data[266..268];
        let state = match hex_device_state {
            "01" => DeviceState::On,
            "00" => DeviceState::Off,
            _ => DeviceState::Off, // Default to Off for unknown states
        };

        // Power consumption (hex positions 270:278 in hex representation)
        let hex_power = &hex_data[270..278];
        let power_consumption =
            u16::from_str_radix(&format!("{}{}", &hex_power[2..4], &hex_power[0..2]), 16)
                .unwrap_or(0);

        Some(SwitcherDevice {
            device_id,
            device_key,
            ip_address,
            mac_address,
            name,
            device_type,
            state,
            power_consumption,
        })
    }
}
