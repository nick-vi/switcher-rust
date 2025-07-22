use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn current_timestamp_hex() -> String {
    format!("{:08x}", current_timestamp())
}

pub fn format_timestamp(timestamp: u64) -> String {
    let duration = std::time::Duration::from_secs(timestamp);
    let datetime = UNIX_EPOCH + duration;

    match datetime.elapsed() {
        Ok(elapsed) => {
            let secs = elapsed.as_secs();
            if secs < 60 {
                format!("{} seconds ago", secs)
            } else if secs < 3600 {
                format!("{} minutes ago", secs / 60)
            } else if secs < 86400 {
                format!("{} hours ago", secs / 3600)
            } else {
                format!("{} days ago", secs / 86400)
            }
        }
        Err(_) => "in the future".to_string(),
    }
}
