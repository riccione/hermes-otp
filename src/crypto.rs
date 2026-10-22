use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use magic_crypt::{MagicCryptTrait, new_magic_crypt};

/// Prefix marking an authenticated v2 payload.
pub const V2_PREFIX: &str = "v2$";

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

// Frozen Argon2id parameters for v2 payloads. Changing these would make
// existing records undecryptable, so a change requires a new version prefix.
const M_COST: u32 = 19_456; // 19 MiB
const T_COST: u32 = 2;
const P_COST: u32 = 1;

#[derive(Debug)]
pub enum CryptoError {
    InvalidFormat,
    AuthenticationFailed,
    Kdf,
    Random,
    Base64,
    InvalidUtf8,
    Legacy,
}

impl CryptoError {
    pub fn as_str(&self) -> &'static str {
        match self {
            CryptoError::InvalidFormat => "invalid ciphertext format",
            CryptoError::AuthenticationFailed => "authentication failed",
            CryptoError::Kdf => "key derivation failed",
            CryptoError::Random => "randomness unavailable",
            CryptoError::Base64 => "invalid base64",
            CryptoError::InvalidUtf8 => "invalid plaintext",
            CryptoError::Legacy => "decryption failed",
        }
    }
}

/// True when `secret` is an authenticated v2 payload.
///
/// Neither Base64 (legacy ciphertext) nor Base32 (plaintext) contain `$`, so
/// the prefix is unambiguous.
pub fn is_v2(secret: &str) -> bool {
    secret.starts_with(V2_PREFIX)
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], CryptoError> {
    let params =
        Params::new(M_COST, T_COST, P_COST, Some(KEY_LEN)).map_err(|_| CryptoError::Kdf)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_LEN];
    // The password is used exactly as provided; it is deliberately not trimmed.
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| CryptoError::Kdf)?;

    Ok(key)
}

/// Encrypts a secret with Argon2id and XChaCha20-Poly1305.
///
/// The result is `v2$<base64 salt>$<base64 nonce>$<base64 ciphertext>`.
pub fn encrypt_v2(secret: &str, password: &str) -> Result<String, CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|_| CryptoError::Random)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|_| CryptoError::Random)?;

    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::Kdf)?;
    let nonce = XNonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, secret.as_bytes())
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    Ok(format!(
        "{V2_PREFIX}{}${}${}",
        STANDARD.encode(salt),
        STANDARD.encode(nonce_bytes),
        STANDARD.encode(ciphertext)
    ))
}

/// Decrypts a v2 payload, returning `AuthenticationFailed` if the data was
/// tampered with or the password is wrong.
pub fn decrypt_v2(payload: &str, password: &str) -> Result<String, CryptoError> {
    let rest = payload
        .strip_prefix(V2_PREFIX)
        .ok_or(CryptoError::InvalidFormat)?;

    let parts: Vec<&str> = rest.split('$').collect();
    if parts.len() != 3 {
        return Err(CryptoError::InvalidFormat);
    }

    let salt = STANDARD.decode(parts[0]).map_err(|_| CryptoError::Base64)?;
    let nonce_bytes = STANDARD.decode(parts[1]).map_err(|_| CryptoError::Base64)?;
    let ciphertext = STANDARD.decode(parts[2]).map_err(|_| CryptoError::Base64)?;

    if salt.len() != SALT_LEN || nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidFormat);
    }

    let nonce_array: [u8; NONCE_LEN] = nonce_bytes
        .try_into()
        .map_err(|_| CryptoError::InvalidFormat)?;

    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| CryptoError::Kdf)?;
    let nonce = XNonce::from(nonce_array);

    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidUtf8)
}

/// Decrypts a legacy magic-crypt payload (crypto version 1).
///
/// The password is trimmed here on purpose: every existing v1 ciphertext was
/// produced with a trimmed password, so trimming keeps old vaults readable.
pub fn decrypt_legacy(ciphertext: &str, password: &str) -> Result<String, CryptoError> {
    let mc = new_magic_crypt!(password.trim(), 256);
    mc.decrypt_base64_to_string(ciphertext)
        .map_err(|_| CryptoError::Legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "JBSWY3DPEHPK3PXP";
    const PASSWORD: &str = "correct horse battery staple";

    #[test]
    fn v2_round_trips() {
        let payload = encrypt_v2(SECRET, PASSWORD).unwrap();

        assert!(is_v2(&payload));
        assert_eq!(decrypt_v2(&payload, PASSWORD).unwrap(), SECRET);
    }

    #[test]
    fn v2_uses_a_fresh_salt_and_nonce() {
        let first = encrypt_v2(SECRET, PASSWORD).unwrap();
        let second = encrypt_v2(SECRET, PASSWORD).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn v2_detects_tampering() {
        let payload = encrypt_v2(SECRET, PASSWORD).unwrap();
        let parts: Vec<&str> = payload.split('$').collect();

        let mut ciphertext = STANDARD.decode(parts[3]).unwrap();
        ciphertext[0] ^= 0x01;

        let tampered = format!(
            "{V2_PREFIX}{}${}${}",
            parts[1],
            parts[2],
            STANDARD.encode(&ciphertext)
        );

        assert!(matches!(
            decrypt_v2(&tampered, PASSWORD),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn v2_rejects_a_wrong_password() {
        let payload = encrypt_v2(SECRET, PASSWORD).unwrap();

        assert!(matches!(
            decrypt_v2(&payload, "wrong password"),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn v2_password_is_not_trimmed() {
        let payload = encrypt_v2(SECRET, "  spaced  ").unwrap();

        assert_eq!(decrypt_v2(&payload, "  spaced  ").unwrap(), SECRET);
        assert!(matches!(
            decrypt_v2(&payload, "spaced"),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn v2_rejects_malformed_payloads() {
        for payload in ["v2$", "v2$only-two", "v2$AAAA$BBBB", "v2$!$!$!"] {
            assert!(decrypt_v2(payload, PASSWORD).is_err(), "{payload}");
        }
    }

    #[test]
    fn legacy_round_trips_and_trims_the_password() {
        let mc = new_magic_crypt!("legacy-password", 256);
        let legacy = mc.encrypt_str_to_base64(SECRET);

        assert_eq!(
            decrypt_legacy(&legacy, "  legacy-password  ").unwrap(),
            SECRET
        );
        assert!(matches!(
            decrypt_legacy(&legacy, "wrong"),
            Err(CryptoError::Legacy)
        ));
    }

    #[test]
    fn prefix_detection_is_unambiguous() {
        assert!(is_v2(&encrypt_v2(SECRET, PASSWORD).unwrap()));

        let legacy = new_magic_crypt!("p", 256).encrypt_str_to_base64(SECRET);
        assert!(!is_v2(&legacy));
        assert!(!is_v2(SECRET));
    }
}
