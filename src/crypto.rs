use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};

/// Prefix marking an authenticated v2 payload.
pub const V2_PREFIX: &str = "v2$";

/// Prefix marking a record encrypted with the vault key.
pub const V3_PREFIX: &str = "v3$";

/// Prefix marking the master-password-wrapped vault key.
pub const V3_WRAP_PREFIX: &str = "v3w$";

/// A random symmetric key shared by every record in a vault.
pub type VaultKey = [u8; KEY_LEN];

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// Domain separator so a wrapped vault key can never be confused with a record.
const VAULT_KEY_AAD: &[u8] = b"hermes-otp:v3:vault-key";

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
            CryptoError::Legacy => {
                "legacy v1 format is no longer supported; run hermes-otp 0.8.x to migrate this record first"
            }
        }
    }
}

/// True when `secret` is an authenticated v2 payload.
///
/// Neither Base64 (removed legacy ciphertext) nor Base32 (plaintext) contains
/// `$`, so the prefix is unambiguous.
pub fn is_v2(secret: &str) -> bool {
    secret.starts_with(V2_PREFIX)
}

/// True when `secret` is a record encrypted with the vault key.
pub fn is_v3(secret: &str) -> bool {
    secret.starts_with(V3_PREFIX)
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
///
/// Only used to build fixtures for the migration tests; new records always use
/// [`encrypt_record`].
#[cfg(test)]
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

fn seal(
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::Kdf)?;
    let nonce = XNonce::from(*nonce);

    cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)
}

fn open(key: &[u8], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::Kdf)?;
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::InvalidFormat)?;
    let nonce = XNonce::from(nonce);

    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)
}

/// Generates a fresh random vault key.
pub fn generate_vault_key() -> Result<VaultKey, CryptoError> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).map_err(|_| CryptoError::Random)?;
    Ok(key)
}

/// Wraps `key` with a master password using Argon2id and XChaCha20-Poly1305.
///
/// The result is `v3w$<base64 salt>$<base64 nonce>$<base64 ciphertext>`.
pub fn wrap_vault_key(password: &str, key: &VaultKey) -> Result<String, CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|_| CryptoError::Random)?;

    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| CryptoError::Random)?;

    let kek = derive_key(password, &salt)?;
    let ciphertext = seal(&kek, &nonce, key, VAULT_KEY_AAD)?;

    Ok(format!(
        "{V3_WRAP_PREFIX}{}${}${}",
        STANDARD.encode(salt),
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    ))
}

/// Unwraps the vault key, returning `AuthenticationFailed` when the password is
/// wrong or the wrapper was tampered with.
pub fn unwrap_vault_key(wrapper: &str, password: &str) -> Result<VaultKey, CryptoError> {
    let rest = wrapper
        .strip_prefix(V3_WRAP_PREFIX)
        .ok_or(CryptoError::InvalidFormat)?;

    let parts: Vec<&str> = rest.split('$').collect();
    if parts.len() != 3 {
        return Err(CryptoError::InvalidFormat);
    }

    let salt = STANDARD.decode(parts[0]).map_err(|_| CryptoError::Base64)?;
    let nonce = STANDARD.decode(parts[1]).map_err(|_| CryptoError::Base64)?;
    let ciphertext = STANDARD.decode(parts[2]).map_err(|_| CryptoError::Base64)?;

    if salt.len() != SALT_LEN || nonce.len() != NONCE_LEN {
        return Err(CryptoError::InvalidFormat);
    }

    let kek = derive_key(password, &salt)?;
    let plaintext = open(&kek, &nonce, &ciphertext, VAULT_KEY_AAD)?;

    plaintext
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidFormat)
}

/// Encrypts a record with the vault key, binding it to `alias` as associated
/// data so ciphertexts cannot be swapped between records.
pub fn encrypt_record(secret: &str, key: &VaultKey, alias: &str) -> Result<String, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| CryptoError::Random)?;

    let ciphertext = seal(key, &nonce, secret.as_bytes(), alias.as_bytes())?;

    Ok(format!(
        "{V3_PREFIX}{}${}",
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    ))
}

/// Decrypts a record with the vault key; the same `alias` used to encrypt it is
/// required as associated data.
pub fn decrypt_record(payload: &str, key: &VaultKey, alias: &str) -> Result<String, CryptoError> {
    let rest = payload
        .strip_prefix(V3_PREFIX)
        .ok_or(CryptoError::InvalidFormat)?;

    let parts: Vec<&str> = rest.split('$').collect();
    if parts.len() != 2 {
        return Err(CryptoError::InvalidFormat);
    }

    let nonce = STANDARD.decode(parts[0]).map_err(|_| CryptoError::Base64)?;
    let ciphertext = STANDARD.decode(parts[1]).map_err(|_| CryptoError::Base64)?;

    let plaintext = open(key, &nonce, &ciphertext, alias.as_bytes())?;
    String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidUtf8)
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
    fn legacy_payloads_are_rejected() {
        assert!(matches!(
            decrypt_v2("3q2+7w==", PASSWORD),
            Err(CryptoError::InvalidFormat)
        ));
    }

    #[test]
    fn prefix_detection_is_unambiguous() {
        let key = generate_vault_key().unwrap();

        assert!(is_v2(&encrypt_v2(SECRET, PASSWORD).unwrap()));
        assert!(is_v3(&encrypt_record(SECRET, &key, "github").unwrap()));

        assert!(!is_v2(
            encrypt_record(SECRET, &key, "github").unwrap().as_str()
        ));
        assert!(!is_v3("3q2+7w=="));
        assert!(!is_v3(SECRET));
    }

    #[test]
    fn v3_record_round_trips() {
        let key = generate_vault_key().unwrap();
        let payload = encrypt_record(SECRET, &key, "github").unwrap();

        assert_eq!(decrypt_record(&payload, &key, "github").unwrap(), SECRET);
    }

    #[test]
    fn v3_uses_a_fresh_nonce_per_record() {
        let key = generate_vault_key().unwrap();

        assert_ne!(
            encrypt_record(SECRET, &key, "github").unwrap(),
            encrypt_record(SECRET, &key, "github").unwrap()
        );
    }

    #[test]
    fn v3_rejects_a_wrong_key_or_alias() {
        let key = generate_vault_key().unwrap();
        let payload = encrypt_record(SECRET, &key, "github").unwrap();
        let other_key = generate_vault_key().unwrap();

        assert!(matches!(
            decrypt_record(&payload, &other_key, "github"),
            Err(CryptoError::AuthenticationFailed)
        ));
        assert!(matches!(
            decrypt_record(&payload, &key, "gitlab"),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn v3_detects_tampering() {
        let key = generate_vault_key().unwrap();
        let payload = encrypt_record(SECRET, &key, "github").unwrap();
        let parts: Vec<&str> = payload.split('$').collect();

        let mut ciphertext = STANDARD.decode(parts[2]).unwrap();
        ciphertext[0] ^= 0x01;

        let tampered = format!("{V3_PREFIX}{}${}", parts[1], STANDARD.encode(&ciphertext));

        assert!(matches!(
            decrypt_record(&tampered, &key, "github"),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn vault_key_round_trips_and_rejects_a_wrong_password() {
        let key = generate_vault_key().unwrap();
        let wrapper = wrap_vault_key(PASSWORD, &key).unwrap();

        assert_eq!(unwrap_vault_key(&wrapper, PASSWORD).unwrap(), key);
        assert!(matches!(
            unwrap_vault_key(&wrapper, "wrong password"),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn vault_key_wrapper_uses_a_fresh_salt_and_nonce() {
        let key = generate_vault_key().unwrap();

        assert_ne!(
            wrap_vault_key(PASSWORD, &key).unwrap(),
            wrap_vault_key(PASSWORD, &key).unwrap()
        );
    }

    #[test]
    fn vault_key_wrapper_detects_tampering() {
        let key = generate_vault_key().unwrap();
        let wrapper = wrap_vault_key(PASSWORD, &key).unwrap();
        let parts: Vec<&str> = wrapper.split('$').collect();

        let mut ciphertext = STANDARD.decode(parts[3]).unwrap();
        ciphertext[0] ^= 0x01;

        let tampered = format!(
            "{V3_WRAP_PREFIX}{}${}${}",
            parts[1],
            parts[2],
            STANDARD.encode(&ciphertext)
        );

        assert!(matches!(
            unwrap_vault_key(&tampered, PASSWORD),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn vault_key_rejects_malformed_wrappers() {
        for wrapper in ["v3w$", "v3w$only-two", "v3w$AAAA$BBBB", "v3w$!$!$!"] {
            assert!(unwrap_vault_key(wrapper, PASSWORD).is_err(), "{wrapper}");
        }
    }
}
