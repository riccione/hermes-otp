use data_encoding::BASE32_NOPAD;
use magic_crypt::{MagicCryptTrait, new_magic_crypt};
use std::time::{SystemTime, UNIX_EPOCH};
use totp_lite::{DEFAULT_STEP, Sha1, totp_custom};

pub enum OtpError {
    DecryptionFailed,
    InvalidBase32,
    SystemTimeError,
}

// helper fn to get UNIX timestamp
fn get_current_timestamp() -> Result<u64, OtpError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| OtpError::SystemTimeError)
}

pub fn get_remaining_seconds() -> u64 {
    let now = get_current_timestamp().unwrap_or(0);
    // DEFAULT_STEP == 30s
    DEFAULT_STEP - (now % DEFAULT_STEP)
}

/*
 * Alternative: totp-rs.
 */
pub fn generate_otp(x: &str) -> Result<String, OtpError> {
    // decode Base32
    let decoded = BASE32_NOPAD
        .decode(x.as_bytes())
        .map_err(|_| OtpError::InvalidBase32)?;

    // get current timestamp
    let now = get_current_timestamp()?;

    // generate OTP
    let otp = totp_custom::<Sha1>(DEFAULT_STEP, 6, &decoded, now);

    Ok(otp)
}

pub fn encrypt(code: &str, password: &str) -> String {
    let mc = new_magic_crypt!(password.trim(), 256);
    mc.encrypt_str_to_base64(code)
}

pub fn decrypt(encrypted_code: &str, password: &str) -> Result<String, OtpError> {
    let mc = new_magic_crypt!(password.trim(), 256);
    mc.decrypt_base64_to_string(encrypted_code)
        .map_err(|_| OtpError::DecryptionFailed)
}
