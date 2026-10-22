use data_encoding::BASE32_NOPAD;
use std::time::{SystemTime, UNIX_EPOCH};
use totp_lite::{Sha1, Sha256, Sha512, totp_custom};

/// Digit counts accepted by the RFC 4226 / 6238 implementations in the wild.
const MIN_DIGITS: u32 = 6;
const MAX_DIGITS: u32 = 8;
/// Longest accepted time step, in seconds.
const MAX_PERIOD: u32 = 300;

#[derive(Debug)]
pub enum OtpError {
    InvalidBase32,
    SystemTimeError,
    UnsupportedAlgorithm,
    InvalidDigits,
    InvalidPeriod,
}

impl OtpError {
    pub fn as_str(&self) -> &'static str {
        match self {
            OtpError::InvalidBase32 => "invalid base32 secret",
            OtpError::SystemTimeError => "system time error",
            OtpError::UnsupportedAlgorithm => "unsupported algorithm",
            OtpError::InvalidDigits => "unsupported digit count",
            OtpError::InvalidPeriod => "invalid period",
        }
    }
}

// helper fn to get UNIX timestamp
fn get_current_timestamp() -> Result<u64, OtpError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| OtpError::SystemTimeError)
}

/// Seconds remaining in the current window, for the given period.
pub fn get_remaining_seconds(period: u64) -> u64 {
    if period == 0 {
        return 0;
    }

    let now = get_current_timestamp().unwrap_or(0);
    period - (now % period)
}

/// Validates the parameters accepted by [`generate_otp`], returning a message
/// suitable for the user when they are out of range.
pub fn validate_params(algorithm: &str, digits: u32, period: u32) -> Result<(), String> {
    if !matches!(
        algorithm.to_lowercase().as_str(),
        "sha1" | "sha256" | "sha512"
    ) {
        return Err(format!(
            "Unsupported OTP parameter (algorithm={algorithm}). \
             Only sha1, sha256 and sha512 are supported."
        ));
    }

    if !(MIN_DIGITS..=MAX_DIGITS).contains(&digits) {
        return Err(format!(
            "Unsupported OTP parameter (digits={digits}). \
             Only {MIN_DIGITS} to {MAX_DIGITS} digits are supported."
        ));
    }

    if !(1..=MAX_PERIOD).contains(&period) {
        return Err(format!(
            "Unsupported OTP parameter (period={period}). \
             Period must be between 1 and {MAX_PERIOD} seconds."
        ));
    }

    Ok(())
}

/// Generates a code for `secret` using the given parameters, at the current
/// time.
pub fn generate_otp(
    secret: &str,
    algorithm: &str,
    digits: u32,
    period: u64,
) -> Result<String, OtpError> {
    let now = get_current_timestamp()?;
    generate_otp_at(secret, algorithm, digits, period, now)
}

/// Generates a code at a specific timestamp. Used by tests to pin the RFC 6238
/// test vectors.
fn generate_otp_at(
    secret: &str,
    algorithm: &str,
    digits: u32,
    period: u64,
    time: u64,
) -> Result<String, OtpError> {
    if period == 0 {
        return Err(OtpError::InvalidPeriod);
    }

    if !(MIN_DIGITS..=MAX_DIGITS).contains(&digits) {
        return Err(OtpError::InvalidDigits);
    }

    // decode Base32
    let decoded = BASE32_NOPAD
        .decode(secret.as_bytes())
        .map_err(|_| OtpError::InvalidBase32)?;

    match algorithm.to_lowercase().as_str() {
        "sha1" => Ok(totp_custom::<Sha1>(period, digits, &decoded, time)),
        "sha256" => Ok(totp_custom::<Sha256>(period, digits, &decoded, time)),
        "sha512" => Ok(totp_custom::<Sha512>(period, digits, &decoded, time)),
        _ => Err(OtpError::UnsupportedAlgorithm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B seeds, Base32-encoded the way the app expects.
    const SHA1_SEED: &[u8] = b"12345678901234567890";
    const SHA256_SEED: &[u8] = b"12345678901234567890123456789012";
    const SHA512_SEED: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    fn secret(seed: &[u8]) -> String {
        BASE32_NOPAD.encode(seed)
    }

    #[test]
    fn rfc6238_vectors_sha1() {
        let secret = secret(SHA1_SEED);
        let cases = [
            (59, "94287082"),
            (1111111109, "07081804"),
            (1111111111, "14050471"),
            (1234567890, "89005924"),
            (2000000000, "69279037"),
            (20000000000, "65353130"),
        ];

        for (time, expected) in cases {
            let otp = generate_otp_at(&secret, "sha1", 8, 30, time).unwrap();
            assert_eq!(otp, expected, "sha1 at {time}");
        }
    }

    #[test]
    fn rfc6238_vectors_sha256() {
        let secret = secret(SHA256_SEED);
        let cases = [
            (59, "46119246"),
            (1111111109, "68084774"),
            (1111111111, "67062674"),
            (1234567890, "91819424"),
            (2000000000, "90698825"),
            (20000000000, "77737706"),
        ];

        for (time, expected) in cases {
            let otp = generate_otp_at(&secret, "sha256", 8, 30, time).unwrap();
            assert_eq!(otp, expected, "sha256 at {time}");
        }
    }

    #[test]
    fn rfc6238_vectors_sha512() {
        let secret = secret(SHA512_SEED);
        let cases = [
            (59, "90693936"),
            (1111111109, "25091201"),
            (1111111111, "99943326"),
            (1234567890, "93441116"),
            (2000000000, "38618901"),
            (20000000000, "47863826"),
        ];

        for (time, expected) in cases {
            let otp = generate_otp_at(&secret, "sha512", 8, 30, time).unwrap();
            assert_eq!(otp, expected, "sha512 at {time}");
        }
    }

    #[test]
    fn period_windows_are_respected() {
        let secret = secret(SHA1_SEED);

        let start = generate_otp_at(&secret, "sha1", 6, 60, 0).unwrap();
        let last = generate_otp_at(&secret, "sha1", 6, 60, 59).unwrap();
        let next = generate_otp_at(&secret, "sha1", 6, 60, 60).unwrap();

        // the whole 60 second window shares one code
        assert_eq!(start, last);
        assert_ne!(start, next);
    }

    #[test]
    fn digit_counts_from_six_to_eight_are_accepted() {
        let secret = secret(SHA1_SEED);

        for digits in MIN_DIGITS..=MAX_DIGITS {
            let otp = generate_otp_at(&secret, "sha1", digits, 30, 59).unwrap();
            assert_eq!(otp.len(), digits as usize, "digits={digits}");
        }
    }

    #[test]
    fn algorithm_is_case_insensitive() {
        let secret = secret(SHA1_SEED);

        let lower = generate_otp_at(&secret, "sha1", 8, 30, 59).unwrap();
        let upper = generate_otp_at(&secret, "SHA1", 8, 30, 59).unwrap();
        assert_eq!(lower, upper);
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        let secret = secret(SHA1_SEED);

        assert!(matches!(
            generate_otp_at(&secret, "sha1", 5, 30, 59),
            Err(OtpError::InvalidDigits)
        ));
        assert!(matches!(
            generate_otp_at(&secret, "sha1", 9, 30, 59),
            Err(OtpError::InvalidDigits)
        ));
        assert!(matches!(
            generate_otp_at(&secret, "sha1", 6, 0, 59),
            Err(OtpError::InvalidPeriod)
        ));
        assert!(matches!(
            generate_otp_at(&secret, "md5", 6, 30, 59),
            Err(OtpError::UnsupportedAlgorithm)
        ));
    }

    #[test]
    fn remaining_seconds_uses_the_period() {
        for period in [30u64, 60] {
            let rem = get_remaining_seconds(period);
            assert!((1..=period).contains(&rem), "rem={rem} period={period}");
        }

        assert_eq!(get_remaining_seconds(0), 0);
    }

    #[test]
    fn validate_params_accepts_supported_values() {
        for algorithm in ["sha1", "sha256", "sha512", "SHA256"] {
            assert!(validate_params(algorithm, 6, 30).is_ok(), "{algorithm}");
        }

        for digits in MIN_DIGITS..=MAX_DIGITS {
            assert!(
                validate_params("sha1", digits, 30).is_ok(),
                "digits={digits}"
            );
        }

        for period in [1, 30, 300] {
            assert!(
                validate_params("sha1", 6, period).is_ok(),
                "period={period}"
            );
        }
    }

    #[test]
    fn validate_params_rejects_out_of_range_values() {
        let err = validate_params("sha1", 5, 30).unwrap_err();
        assert!(err.contains("digits=5"), "{err}");
        assert!(err.contains("Only 6 to 8 digits"), "{err}");

        let err = validate_params("sha1", 9, 30).unwrap_err();
        assert!(err.contains("digits=9"), "{err}");

        let err = validate_params("sha1", 6, 0).unwrap_err();
        assert!(err.contains("period=0"), "{err}");
        assert!(err.contains("between 1 and 300 seconds"), "{err}");

        let err = validate_params("sha1", 6, 301).unwrap_err();
        assert!(err.contains("period=301"), "{err}");

        let err = validate_params("md5", 6, 30).unwrap_err();
        assert!(err.contains("algorithm=md5"), "{err}");
        assert!(err.contains("Only sha1, sha256 and sha512"), "{err}");
    }
}
