use crate::device::{DeviceState, DeviceStatus};
use crate::utils::current_timestamp_hex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

const SWITCHER_PORT: u16 = 9957;
const LOGIN_TIMEOUT_SECS: u64 = 3;
const CONNECT_TIMEOUT_SECS: u64 = 5;
const MIN_LOGIN_RESPONSE_LEN: usize = 20;
const DEVICE_STATE_BYTE_POS: usize = 75;
const POWER_BYTE_POS: usize = 77;
const COMMAND_VERIFY_DELAY_MS: u64 = 500;
const COMMAND_RETRY_DELAY_MS: u64 = 1000;

pub struct SwitcherController {
    ip_address: String,
    device_id: String,
    port: u16,
}

impl SwitcherController {
    pub fn new(ip_address: String, device_id: String) -> Self {
        Self {
            ip_address,
            device_id,
            port: SWITCHER_PORT,
        }
    }

    pub async fn turn_on(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control_command("1").await?;

        // Verify the command worked by checking status (with retry)
        tokio::time::sleep(tokio::time::Duration::from_millis(COMMAND_VERIFY_DELAY_MS)).await;
        let mut status = self.get_status().await?;

        if status.state != DeviceState::On {
            // Device might need more time, try once more
            tokio::time::sleep(tokio::time::Duration::from_millis(COMMAND_RETRY_DELAY_MS)).await;
            status = self.get_status().await?;

            if status.state != DeviceState::On {
                return Err("Command sent but device did not turn ON (invalid device ID?)".into());
            }
        }

        Ok(())
    }

    pub async fn turn_off(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_control_command("0").await?;

        // Verify the command worked by checking status (with retry)
        tokio::time::sleep(tokio::time::Duration::from_millis(COMMAND_VERIFY_DELAY_MS)).await;
        let mut status = self.get_status().await?;

        if status.state != DeviceState::Off {
            // Device might need more time, try once more
            tokio::time::sleep(tokio::time::Duration::from_millis(COMMAND_RETRY_DELAY_MS)).await;
            status = self.get_status().await?;

            if status.state != DeviceState::Off {
                return Err("Command sent but device did not turn OFF (invalid device ID?)".into());
            }
        }

        Ok(())
    }

    pub async fn set_device_name(&self, new_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            TcpStream::connect(format!("{}:{}", self.ip_address, self.port)),
        )
        .await??;

        let (timestamp, session_id) = self.login(&mut stream).await?;
        let packet = self.build_set_name_packet(&session_id, &timestamp, new_name)?;

        let signed_packet = self.sign_packet(&packet);
        stream.write_all(&hex::decode(signed_packet)?).await?;

        // Read response to confirm command was received
        let mut response = [0; 1024];
        let len = stream.read(&mut response).await?;

        if len < 20 {
            return Err("Device did not respond to name change command".into());
        }

        // Wait a moment for the device to process the name change
        tokio::time::sleep(tokio::time::Duration::from_millis(COMMAND_VERIFY_DELAY_MS)).await;

        Ok(())
    }

    pub async fn get_status(&self) -> Result<DeviceStatus, Box<dyn std::error::Error>> {
        let mut stream = timeout(
            Duration::from_secs(CONNECT_TIMEOUT_SECS),
            TcpStream::connect(format!("{}:{}", self.ip_address, self.port)),
        )
        .await??;

        let (timestamp, session_id) = self.login(&mut stream).await?;
        let packet = self.build_get_state_packet(&session_id, &timestamp);

        let signed_packet = self.sign_packet(&packet);
        stream.write_all(&hex::decode(signed_packet)?).await?;

        let mut response = [0; 1024];
        let len = stream.read(&mut response).await?;

        // Check if we got a valid response (should be > 100 bytes for real device)
        if len < 50 {
            return Err("Device did not respond or invalid device ID".into());
        }

        let state = if len > DEVICE_STATE_BYTE_POS {
            match response[DEVICE_STATE_BYTE_POS] {
                0x01 => DeviceState::On,
                0x00 => DeviceState::Off,
                _ => DeviceState::Unknown,
            }
        } else {
            DeviceState::Off
        };

        let power = if len > POWER_BYTE_POS + 1 {
            u16::from_le_bytes([response[POWER_BYTE_POS], response[POWER_BYTE_POS + 1]])
        } else {
            0
        };

        Ok(DeviceStatus {
            state,
            power_consumption: power,
        })
    }

    async fn send_control_command(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip_address, self.port)).await?;

        let (timestamp, session_id) = self.login(&mut stream).await?;
        let packet = self.build_control_packet(&session_id, &timestamp, command);
        let signed_packet = self.sign_packet(&packet);

        stream.write_all(&hex::decode(signed_packet)?).await?;

        Ok(())
    }

    async fn login(
        &self,
        stream: &mut TcpStream,
    ) -> Result<(String, String), Box<dyn std::error::Error>> {
        let timestamp = self.get_timestamp();
        let packet = self.build_login_packet(&timestamp);
        let signed_packet = self.sign_packet(&packet);

        stream.write_all(&hex::decode(signed_packet)?).await?;

        let mut response = [0; 1024];
        let len = timeout(
            Duration::from_secs(LOGIN_TIMEOUT_SECS),
            stream.read(&mut response),
        )
        .await??;

        if len < MIN_LOGIN_RESPONSE_LEN {
            return Err("Login response too short".into());
        }

        let session_id = hex::encode(&response[16..20]);

        Ok((timestamp, session_id))
    }

    fn get_timestamp(&self) -> String {
        current_timestamp_hex()
    }

    fn build_login_packet(&self, timestamp: &str) -> String {
        format!(
            "fef052000232a10000000000340001000000000000000000{}00000000000000000000f0fe00{}00",
            timestamp,
            "0".repeat(72)
        )
    }

    fn build_control_packet(&self, session_id: &str, timestamp: &str, command: &str) -> String {
        format!(
            "fef05d0002320102{}340001000000000000000000{}00000000000000000000f0fe{}{}000106000{}00{}",
            session_id,
            timestamp,
            &self.device_id,
            "0".repeat(72),
            command,
            "00000000"
        )
    }

    fn build_get_state_packet(&self, session_id: &str, timestamp: &str) -> String {
        format!(
            "fef0300002320103{}340001000000000000000000{}00000000000000000000f0fe{}00",
            session_id, timestamp, &self.device_id
        )
    }

    fn build_set_name_packet(
        &self,
        session_id: &str,
        timestamp: &str,
        new_name: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Convert name to hex and pad to 32 bytes (following aioswitcher implementation)
        let name_hex = self.string_to_hexadecimal_device_name(new_name)?;

        // Build packet following aioswitcher UPDATE_DEVICE_NAME_PACKET format
        Ok(format!(
            "fef0740002320202{}340001000000000000000000{}00000000000000000000f0fe{}{}00{}",
            session_id,
            timestamp,
            &self.device_id,
            "0".repeat(72), // PAD_72_ZEROS
            name_hex
        ))
    }

    fn string_to_hexadecimal_device_name(
        &self,
        name: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let length = name.len();
        if length < 2 || length > 32 {
            return Err(format!(
                "Device name length must be between 2 and 32 characters, got {}",
                length
            )
            .into());
        }

        let name_bytes = name.as_bytes();
        let mut hex_name = hex::encode(name_bytes);

        // Pad with zeros to 64 hex characters (32 bytes)
        let zeros_needed = 64 - hex_name.len();
        hex_name.push_str(&"00".repeat(zeros_needed / 2));

        Ok(hex_name)
    }

    fn sign_packet(&self, hex_packet: &str) -> String {
        use crc::{Crc, CRC_16_XMODEM};

        let binary_packet = hex::decode(hex_packet).unwrap();
        let crc_algo = Crc::<u16>::new(&CRC_16_XMODEM);

        let mut digest = crc_algo.digest_with_initial(0x1021);
        digest.update(&binary_packet);
        let packet_crc = digest.finalize();

        let binary_packet_crc = (packet_crc as u32).to_be_bytes();
        let hex_packet_crc = hex::encode(binary_packet_crc);
        let hex_packet_crc_sliced = format!("{}{}", &hex_packet_crc[6..8], &hex_packet_crc[4..6]);

        let key_hex = format!("{}{}", hex_packet_crc_sliced, "30".repeat(32));
        let binary_key = hex::decode(key_hex).unwrap();

        let mut key_digest = crc_algo.digest_with_initial(0x1021);
        key_digest.update(&binary_key);
        let key_crc = key_digest.finalize();

        let binary_key_crc = (key_crc as u32).to_be_bytes();
        let hex_key_crc = hex::encode(binary_key_crc);
        let hex_key_crc_sliced = format!("{}{}", &hex_key_crc[6..8], &hex_key_crc[4..6]);

        format!(
            "{}{}{}",
            hex_packet, hex_packet_crc_sliced, hex_key_crc_sliced
        )
    }
}
