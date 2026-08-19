use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

fn default_digits() -> u8 {
    6
}

fn default_period() -> u32 {
    30
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Record {
    pub alias: String,
    pub secret: String,
    pub is_unencrypted: bool,
    pub algorithm: String,
    #[serde(default = "default_digits")]
    pub digits: u8,
    #[serde(default = "default_period")]
    pub period: u32,
    pub created_at: u64,
}

impl Record {
    pub fn new(alias: String, secret: String, is_unencrypted: bool) -> Self {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        Self {
            alias,
            secret,
            is_unencrypted,
            algorithm: "sha1".to_string(),
            digits: default_digits(),
            period: default_period(),
            created_at: since_the_epoch,
        }
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // serialize the current record to a JSON
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}
