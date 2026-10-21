use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

#[derive(Debug, Clone)]
pub struct ParsedUri {
    pub alias: String,
    pub secret: String,
    pub algorithm: String,
    pub digits: u8,
    pub period: u32,
    pub created_at: u64,
}

pub struct OtpAuth;

impl OtpAuth {
    pub fn parse(uri: &str) -> Result<ParsedUri, String> {
        let parsed_url = Url::parse(uri).map_err(|e| format!("Invalid URL: {e}"))?;

        if parsed_url.scheme() != "otpauth" || parsed_url.host_str() != Some("totp") {
            return Err("Only 'otpauth://totp/...' URIs are supported".to_string());
        }

        let raw_label = parsed_url.path().trim_start_matches('/');
        let decoded_label = urlencoding::decode(raw_label)
            .map(|cow| cow.into_owned())
            .unwrap_or_else(|_| raw_label.to_string());

        let query_params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        let secret = query_params
            .get("secret")
            .ok_or("Missing mandatory 'secret' parameter in URI")?
            .clone();

        let issuer = query_params.get("issuer");
        let alias = match issuer {
            Some(iss) if !decoded_label.contains(iss) => format!("{iss}:{decoded_label}"),
            _ => decoded_label,
        };

        let algorithm = query_params
            .get("algorithm")
            .map(|raw| raw.to_lowercase())
            .unwrap_or_else(|| "sha1".to_string());

        // Absent parameters fall back to the defaults; present but unparseable
        // ones are rejected with the same wording as the shared validator.
        let digits = match query_params.get("digits") {
            None => 6,
            Some(raw) => raw.parse::<u8>().map_err(|_| {
                format!(
                    "Unsupported OTP parameter (digits={raw}). \
                     Only 6 to 8 digits are supported."
                )
            })?,
        };

        let period = match query_params.get("period") {
            None => 30,
            Some(raw) => raw.parse::<u32>().map_err(|_| {
                format!(
                    "Unsupported OTP parameter (period={raw}). \
                     Period must be between 1 and 300 seconds."
                )
            })?,
        };

        crate::otp::validate_params(&algorithm, digits as u32, period)?;

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(ParsedUri {
            alias,
            secret,
            algorithm,
            digits,
            period,
            created_at,
        })
    }

    pub fn parse_batch<P: AsRef<Path>>(path: P) -> Result<Vec<ParsedUri>, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open batch file: {e}"))?;
        let reader = BufReader::new(file);

        reader
            .lines()
            .map(|line_res| line_res.map_err(|e| format!("File read error: {e}")))
            .map(|line_res| line_res.map(|s| s.trim().to_string()))
            .filter(|line_res| match line_res {
                Ok(s) => !s.is_empty(),
                Err(_) => true,
            })
            .map(|line_res| {
                let line = line_res?;
                Self::parse(&line)
            })
            .collect::<Result<Vec<ParsedUri>, String>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_standard_totp_success() {
        let uri = "otpauth://totp/GitHub:alice@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub";
        let res = OtpAuth::parse(uri);
        assert!(res.is_ok());
        let parsed = res.unwrap();
        assert_eq!(parsed.alias, "GitHub:alice@gmail.com");
        assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(parsed.algorithm, "sha1");
        assert_eq!(parsed.digits, 6);
        assert_eq!(parsed.period, 30);
    }

    #[test]
    fn test_parse_enterprise_custom_parameters() {
        let uri = "otpauth://totp/Cisco:admin?secret=KVKVEVOTEVKVEVOT&issuer=Cisco&digits=8&period=60&algorithm=SHA256";
        let res = OtpAuth::parse(uri);
        assert!(res.is_ok());
        let parsed = res.unwrap();
        assert_eq!(parsed.alias, "Cisco:admin");
        assert_eq!(parsed.algorithm, "sha256");
        assert_eq!(parsed.digits, 8);
        assert_eq!(parsed.period, 60);
    }

    #[test]
    fn test_url_decoding_and_alias_deduplication() {
        let uri =
            "otpauth://totp/Google:stage%20user@corp.com?secret=JBSWY3DPEHPK3PXP&issuer=Google";
        let parsed = OtpAuth::parse(uri).unwrap();
        assert_eq!(parsed.alias, "Google:stage user@corp.com");
    }

    #[test]
    fn test_missing_mandatory_secret_fails() {
        let uri = "otpauth://totp/App:user?issuer=App";
        let res = OtpAuth::parse(uri);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Missing mandatory 'secret'"));
    }

    #[test]
    fn test_unsupported_hotp_fails() {
        let uri = "otpauth://hotp/Bank:user?secret=JBSWY3DPEHPK3PXP&counter=12";
        let res = OtpAuth::parse(uri);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.to_lowercase().contains("totp") || err.to_lowercase().contains("otpauth"));
    }

    #[test]
    fn test_seven_digits_are_accepted() {
        let uri = "otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&digits=7";
        assert_eq!(OtpAuth::parse(uri).unwrap().digits, 7);
    }

    #[test]
    fn test_rejects_invalid_digits() {
        for raw in ["5", "9", "0", "abc"] {
            let uri = format!("otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&digits={raw}");
            let err = OtpAuth::parse(&uri).unwrap_err();

            assert!(err.contains(&format!("digits={raw}")), "{err}");
            assert!(err.contains("Only 6 to 8 digits"), "{err}");
        }
    }

    #[test]
    fn test_rejects_invalid_period() {
        for raw in ["0", "301", "abc"] {
            let uri = format!("otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&period={raw}");
            let err = OtpAuth::parse(&uri).unwrap_err();

            assert!(err.contains(&format!("period={raw}")), "{err}");
            assert!(err.contains("between 1 and 300 seconds"), "{err}");
        }
    }

    #[test]
    fn test_rejects_unsupported_algorithm() {
        let uri = "otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&algorithm=md5";
        let err = OtpAuth::parse(uri).unwrap_err();

        assert!(err.contains("algorithm=md5"), "{err}");
        assert!(err.contains("Only sha1, sha256 and sha512"), "{err}");
    }

    #[test]
    fn test_defaults_apply_when_parameters_are_absent() {
        let uri = "otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP";
        let parsed = OtpAuth::parse(uri).unwrap();

        assert_eq!(parsed.algorithm, "sha1");
        assert_eq!(parsed.digits, 6);
        assert_eq!(parsed.period, 30);
    }

    #[test]
    fn test_atomic_batch_file_failure() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        let file_contents = "\
            otpauth://totp/Service:A?secret=JBSWY3DPEHPK3PXP\n\
            \n\
            otpauth://hotp/BrokenCounter?secret=JBSWY3DPEHPK3PXP&counter=1\n\
            otpauth://totp/Service:B?secret=JBSWY3DPEHPK3PXP\n\
        ";
        tmp_file.write_all(file_contents.as_bytes()).unwrap();
        let batch_res = OtpAuth::parse_batch(tmp_file.path());
        assert!(batch_res.is_err());
    }
}
