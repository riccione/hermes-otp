use crate::args::OutputFormat;
use crate::crypto;
use crate::error::AppError;
use crate::file;
use crate::models::Record;
use crate::otp;
use crate::otpauth::OtpAuth;
use crate::ui::Table;
use data_encoding::BASE32_NOPAD;
use nucleo_matcher::{Config, Utf32Str};
use std::path::Path;

// Shown in place of the code when a record cannot be decrypted or the secret
// is not valid base32.
const OTP_ERROR_TEXT: &str = "Error Invalid secret or decryption failed";

fn sanitize_and_validate_code(code: &str) -> Result<String, String> {
    let clean = code.to_uppercase().replace("=", "");
    BASE32_NOPAD
        .decode(clean.as_bytes())
        .map_err(|e| format!("Invalid Base32 code: {e}"))?;

    Ok(clean)
}

fn get_effective_password(password: &Option<String>) -> Result<String, String> {
    password
        .as_deref()
        .map(String::from)
        .or_else(|| std::env::var("HERMES_OTP_PASSWORD").ok())
        .map(Ok)
        .unwrap_or_else(|| {
            rpassword::prompt_password("Enter password: ").map_err(|e| format!("TTY error: {e}"))
        })
}

/// Whether [`vault_key`] may create a key for a vault that has none yet.
#[derive(Clone, Copy)]
enum KeyMode {
    ExistingOnly,
    CreateIfMissing,
}

/// Returns the vault key, unwrapping the header or migrating a v2 vault.
///
/// A file without a header and without encrypted records has no key unless
/// `mode` allows creating one. Migration decrypts every v2 record once with the
/// master password and rewrites the file atomically after a `pre-v3` snapshot.
fn vault_key(
    path: &Path,
    codex: &mut file::Codex,
    password: &Option<String>,
    mode: KeyMode,
    quiet: bool,
) -> Result<Option<crypto::VaultKey>, String> {
    if let Some(wrapper) = &codex.vault {
        let pass = get_effective_password(password)?;
        let key = crypto::unwrap_vault_key(wrapper, &pass)
            .map_err(|_| "Incorrect password. Nothing was changed.".to_string())?;
        return Ok(Some(key));
    }

    let encrypted: Vec<usize> = codex
        .records
        .iter()
        .enumerate()
        .filter(|(_, record)| !record.is_unencrypted)
        .map(|(index, _)| index)
        .collect();

    if encrypted.is_empty() {
        return match mode {
            KeyMode::ExistingOnly => Ok(None),
            KeyMode::CreateIfMissing => {
                let pass = get_effective_password(password)?;
                let key = crypto::generate_vault_key().map_err(|e| e.as_str().to_string())?;
                let wrapper =
                    crypto::wrap_vault_key(&pass, &key).map_err(|e| e.as_str().to_string())?;
                codex.vault = Some(wrapper);
                Ok(Some(key))
            }
        };
    }

    let pass = get_effective_password(password)?;
    let mut plaintext = Vec::with_capacity(encrypted.len());
    let mut failed = Vec::new();

    for &index in &encrypted {
        let record = &codex.records[index];

        if !crypto::is_v2(&record.secret) && !crypto::is_v3(&record.secret) {
            return Err(crypto::CryptoError::Legacy.as_str().to_string());
        }

        match crypto::decrypt_v2(&record.secret, &pass) {
            Ok(secret) => plaintext.push((index, secret)),
            Err(_) => failed.push(record.alias.as_str()),
        }
    }

    if !failed.is_empty() {
        return if failed.len() == encrypted.len() {
            Err("Incorrect password. Nothing was changed.".to_string())
        } else {
            Err(format!(
                "could not decrypt record(s) with this password: {}. \
                 In hermes-otp 0.9.x, run `passwd <alias>` to give them the same password, then retry.",
                failed.join(", ")
            ))
        };
    }

    let key = crypto::generate_vault_key().map_err(|e| e.as_str().to_string())?;

    for (index, secret) in plaintext {
        let alias = codex.records[index].alias.clone();
        codex.records[index].secret =
            crypto::encrypt_record(&secret, &key, &alias).map_err(|e| e.as_str().to_string())?;
    }

    codex.vault = Some(crypto::wrap_vault_key(&pass, &key).map_err(|e| e.as_str().to_string())?);

    let backup = file::backup_pre_v3(path).map_err(|e| format!("backup failed: {e}"))?;
    file::write_codex_atomic(path, codex)?;

    if !quiet {
        match backup {
            Some(backup) => eprintln!(
                "Migrated records to the vault format; backup at {}",
                backup.display()
            ),
            None => eprintln!("Migrated records to the vault format."),
        }
    }

    Ok(Some(key))
}

/* Validate code - check if it is a valid base32
* Here I beleive it is necessary to add some explanation for base32 and TOTP.
* Overtime I forgot what it does and my code comments are not good :/
* This is how it works:
* 1. user enters username and password on website
* 2. website asks for second factor (TOTP)
* 3. TOTP app generates 6-digit (usually) code based on the secret key and current time using
*    SHA1 (usually):
*   - Secret key (code) is base32 encoded
*   - base32 should be valid
*   - base32 based on RFC 4648 https://datatracker.ietf.org/doc/html/rfc4648
*   - it uses alphabet of 32 digits: A-Z, 2-7
*   - in some cases padding (=) used - the length of the string % 8 (every 5 bits to 8 bit
*   output)
*   - correct base32 encoded string should decode without errors
* 4. users enters the TOTP code into the website
* 5. website verifies the code using the same secret key and TOTP generation algorithm (SHA1)
* 6. success or fail
*
* The issue #3 was related to BASE32 method from data-encoding crate.
* BASE32 has auto padding
* BASE32_NOPAD - no padding
* I did test code and notice that some codes produce errors - Invalid length with BASE32,
* switching to BASE32_NOPAD fixed the issue.
* It is interesting, I tried 2 crates: base32 and data-encoding.
* base32 produces same results with padding set to true/false?!
* data-encoding - different results.
* I stick for now with data-encoding only because it more popular.
*/
#[allow(clippy::too_many_arguments)]
pub fn add(
    path: &Path,
    alias: &str,
    code: &str,
    algorithm: &str,
    digits: u32,
    period: u32,
    is_unencrypt: &bool,
    password: &Option<String>,
) -> Result<(), String> {
    let clean_code = sanitize_and_validate_code(code)?;

    // reject parameters the generator could not honor
    otp::validate_params(algorithm, digits, period)?;

    let existed = file::file_exists(path);
    let mut codex = if existed {
        file::read_codex(path)?
    } else {
        file::Codex::default()
    };

    if codex.records.iter().any(|r| r.alias == *alias) {
        return Err(format!("Error: Alias '{alias}' already exists."));
    }

    // encrypt if necessary
    let secret = if *is_unencrypt {
        clean_code.clone()
    } else {
        let key = vault_key(path, &mut codex, password, KeyMode::CreateIfMissing, false)?
            .ok_or("No vault key available.")?;
        crypto::encrypt_record(&clean_code, &key, alias).map_err(|e| e.as_str().to_string())?
    };

    let record = Record::new(
        alias.to_string(),
        secret,
        *is_unencrypt,
        algorithm.to_string(),
        digits as u8,
        period,
    );

    codex.records.push(record.clone());

    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    if existed {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
    }

    file::write_codex_atomic(path, &codex)?;

    println!("Record saved.");

    match otp::generate_otp(
        &clean_code,
        &record.algorithm,
        record.digits as u32,
        record.period as u64,
    ) {
        Ok(code) => println!("{code}"),
        Err(_) => println!("Error: failed to generate OTP"),
    }

    Ok(())
}

pub fn update_code(
    path: &Path,
    alias: &str,
    new_code: &str,
    is_unencrypt: &bool,
    password: &Option<String>,
) -> Result<(), String> {
    let clean_code = sanitize_and_validate_code(new_code)?;

    // Locate the record first: we need to know whether it is encrypted.
    let mut codex = file::read_codex(path)?;
    let index = codex
        .records
        .iter()
        .position(|r| r.alias == *alias)
        .ok_or_else(|| format!("No record for '{alias}' found."))?;

    let target_is_encrypted = !codex.records[index].is_unencrypted;

    // The master password is needed to prove access whenever the record is
    // encrypted, even if the replacement will be stored in plaintext (-u).
    let need_password = target_is_encrypted || !*is_unencrypt;
    let pass = need_password
        .then(|| get_effective_password(password))
        .transpose()?;

    if target_is_encrypted {
        let key = vault_key(path, &mut codex, &pass, KeyMode::ExistingOnly, false)?
            .ok_or("No vault key available.")?;
        crypto::decrypt_record(
            &codex.records[index].secret,
            &key,
            &codex.records[index].alias,
        )
        .map_err(|e| match e {
            crypto::CryptoError::Legacy => e.as_str().to_string(),
            _ => "Incorrect password. Nothing was changed.".to_string(),
        })?;
    }

    let secret = if *is_unencrypt {
        clean_code
    } else {
        let key = vault_key(path, &mut codex, &pass, KeyMode::CreateIfMissing, false)?
            .ok_or("No vault key available.")?;
        crypto::encrypt_record(&clean_code, &key, alias).map_err(|e| e.as_str().to_string())?
    };

    let record = &mut codex.records[index];
    record.secret = secret;
    record.is_unencrypted = *is_unencrypt;

    file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
    file::write_codex_atomic(path, &codex)?;

    println!("Record for '{alias}' successfully updated.");
    Ok(())
}

/// Changes the master password by re-wrapping the vault key.
pub fn passwd(path: &Path, current: &Option<String>, new: &Option<String>) -> Result<(), String> {
    let mut codex = file::read_codex(path)?;

    if codex.vault.is_none() && codex.records.iter().all(|record| record.is_unencrypted) {
        return Err("No encrypted records in this vault. Nothing to change.".into());
    }

    let current_password = resolve_current_password(current)?;
    let key = vault_key(
        path,
        &mut codex,
        &Some(current_password),
        KeyMode::ExistingOnly,
        false,
    )?
    .ok_or("No vault key available.")?;

    let new_password = match new {
        Some(password) => password.clone(),
        None => prompt_new_password()?,
    };

    // Only the wrapper changes; record ciphertexts stay untouched.
    codex.vault =
        Some(crypto::wrap_vault_key(&new_password, &key).map_err(|e| e.as_str().to_string())?);

    save_records_routine(path, &codex)?;

    println!("Master password updated.");
    Ok(())
}

fn resolve_current_password(current: &Option<String>) -> Result<String, String> {
    current
        .as_deref()
        .map(String::from)
        .or_else(|| std::env::var("HERMES_OTP_PASSWORD").ok())
        .map(Ok)
        .unwrap_or_else(|| {
            rpassword::prompt_password("Enter current password: ")
                .map_err(|e| format!("TTY error: {e}"))
        })
}

fn prompt_new_password() -> Result<String, String> {
    let first = rpassword::prompt_password("Enter new password: ")
        .map_err(|e| format!("TTY error: {e}"))?;
    let confirmation = rpassword::prompt_password("Confirm new password: ")
        .map_err(|e| format!("TTY error: {e}"))?;

    if first != confirmation {
        return Err("Passwords do not match.".to_string());
    }

    Ok(first)
}

/// Writes the codex after taking the routine `codex.bak` snapshot.
fn save_records_routine(path: &Path, codex: &file::Codex) -> Result<(), String> {
    file::create_routine_backup(path).map_err(|e| format!("Could not create backup: {e}"))?;

    file::write_codex_atomic(path, codex).map_err(|e| format!("Could not save changes: {e}"))
}

pub fn remove(path: &Path, alias: &str) -> Result<(), String> {
    file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;

    let mut codex = file::read_codex(path)?;
    let original_len = codex.records.len();

    codex.records.retain(|r| r.alias != alias);

    if codex.records.len() == original_len {
        return Err(format!("Error: No record for '{alias}' found"));
    }

    file::write_codex_atomic(path, &codex)?;

    println!("Record for {alias} removed.");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn ls(
    path: &Path,
    alias_filter: &Option<String>,
    is_unencrypt: bool,
    password: &Option<String>,
    format: &OutputFormat,
    quiet: bool,
    exact_match: bool,
    fuzzy: bool,
) -> Result<(), AppError> {
    let mut codex = file::read_codex(path)?;

    // Indices of the matching records, in display order.
    let matched: Vec<usize> = if let Some(filter) = alias_filter {
        if fuzzy {
            let mut matcher = nucleo_matcher::Matcher::new(Config::DEFAULT);

            let mut scored: Vec<(u32, usize)> = codex
                .records
                .iter()
                .enumerate()
                .filter_map(|(index, r)| {
                    let mut buf = Vec::new();
                    let haystack = Utf32Str::new(&r.alias, &mut buf);
                    let mut fbuf = Vec::new();
                    let needle = Utf32Str::new(filter, &mut fbuf);
                    matcher
                        .fuzzy_match(haystack, needle)
                        .map(|score| (score as u32, index))
                })
                .collect();

            scored.sort_by_key(|b| std::cmp::Reverse(b.0));
            scored.into_iter().map(|(_, index)| index).collect()
        } else if exact_match {
            // exact match is case-sensitive, like remove/rename/update
            codex
                .records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.alias == *filter)
                .map(|(index, _)| index)
                .collect()
        } else {
            // partial match is case-insensitive
            let filter_lower = filter.to_lowercase();
            codex
                .records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.alias.to_lowercase().contains(&filter_lower))
                .map(|(index, _)| index)
                .collect()
        }
    } else {
        (0..codex.records.len()).collect()
    };

    if matched.is_empty() {
        return Err(if let Some(filter) = alias_filter {
            if exact_match {
                format!("No record found with exact alias '{}'", filter)
            } else {
                format!("No records found matching '{}'", filter)
            }
        } else {
            "No records found".into()
        }
        .into());
    }

    // The legacy v1 format was removed: fail before rendering instead of
    // silently producing an empty or partial table.
    if !is_unencrypt {
        let legacy: Vec<&str> = matched
            .iter()
            .map(|&index| &codex.records[index])
            .filter(|record| {
                !record.is_unencrypted
                    && !crypto::is_v2(&record.secret)
                    && !crypto::is_v3(&record.secret)
            })
            .map(|record| record.alias.as_str())
            .collect();

        if !legacy.is_empty() {
            return Err(AppError::from(format!(
                "legacy v1 record(s) are no longer supported: {}. \
                 Run hermes-otp 0.8.x to migrate them first.",
                legacy.join(", ")
            )));
        }
    }

    let needs_password = !is_unencrypt
        && matched
            .iter()
            .any(|&index| !codex.records[index].is_unencrypted);

    let pass = needs_password
        .then(|| get_effective_password(password))
        .transpose()?;

    // Argon2 runs once here, when the vault key is unwrapped (or a v2 vault is
    // migrated); the records themselves only need fast XChaCha20.
    let key = if needs_password {
        vault_key(path, &mut codex, &pass, KeyMode::ExistingOnly, quiet)?
    } else {
        None
    };

    // Decrypt each matched record exactly once. This keeps decryption out of
    // the render and live-redraw paths.
    let mut plaintext: Vec<Option<String>> = Vec::with_capacity(matched.len());
    let mut decrypt_errors: Vec<Option<String>> = Vec::with_capacity(matched.len());

    for &index in &matched {
        let record = &codex.records[index];

        if is_unencrypt || record.is_unencrypted {
            plaintext.push(Some(record.secret.clone()));
            decrypt_errors.push(None);
            continue;
        }

        match key.as_ref() {
            Some(key) => match crypto::decrypt_record(&record.secret, key, &record.alias) {
                Ok(secret) => {
                    plaintext.push(Some(secret));
                    decrypt_errors.push(None);
                }
                Err(e) => {
                    plaintext.push(None);
                    decrypt_errors.push(Some(e.as_str().to_string()));
                }
            },
            None => {
                plaintext.push(None);
                decrypt_errors.push(Some("vault key unavailable".to_string()));
            }
        }
    }

    let rows: Vec<Row> = matched
        .iter()
        .enumerate()
        .map(|(position, &index)| Row {
            record: &codex.records[index],
            secret: plaintext[position].as_deref(),
            error: decrypt_errors[position].as_deref(),
        })
        .collect();

    match format {
        OutputFormat::Json => print_json(&rows, alias_filter.is_some()),
        OutputFormat::Table => {
            if quiet {
                print_table(&rows, alias_filter.is_some(), quiet);
            } else {
                let table = Table::new(&rows, alias_filter.is_some());
                table.render();
            }
        }
    }

    // A record that could not produce a code is a failure, even though the
    // output was already rendered. Report it so scripts can react.
    let failed: Vec<&str> = rows
        .iter()
        .filter(|row| row.otp().is_err())
        .map(|row| row.record.alias.as_str())
        .collect();

    if !failed.is_empty() {
        let aliases = failed.join(", ");
        let total = rows.len();

        return if failed.len() == total {
            Err(AppError::from(format!(
                "could not produce a code for {failed_len} record(s) \
                 (wrong password, corrupt secret, or unsupported parameters): {aliases}",
                failed_len = failed.len()
            )))
        } else {
            Err(AppError::partial(format!(
                "{failed_len} of {total} records failed \
                 (wrong password, corrupt secret, or unsupported parameters): {aliases}",
                failed_len = failed.len()
            )))
        };
    }

    Ok(())
}

/// A matched record together with its decrypted secret, or a failure reason.
pub(crate) struct Row<'a> {
    record: &'a Record,
    secret: Option<&'a str>,
    error: Option<&'a str>,
}

impl Row<'_> {
    /// Generates the current code for this row.
    fn otp(&self) -> Result<String, &str> {
        match (self.secret, self.error) {
            (Some(secret), _) => otp::generate_otp(
                secret,
                &self.record.algorithm,
                self.record.digits as u32,
                self.record.period as u64,
            )
            .map_err(|e| e.as_str()),
            (None, Some(error)) => Err(error),
            (None, None) => Err(OTP_ERROR_TEXT),
        }
    }

    /// Renders the OTP cell of the row, falling back to an error marker.
    fn cell(&self) -> String {
        self.otp().unwrap_or_else(|_| OTP_ERROR_TEXT.to_string())
    }
}

/// Renders the countdown bar for `rem` seconds out of `period`.
fn progress_bar(rem: u64, period: u64, width: usize) -> String {
    let period = period.max(1);
    let safe_rem = rem.min(period) as usize;
    let filled = safe_rem * width / period as usize;
    let empty = width - filled;

    format!("[{0}{1}]", "#".repeat(filled), ".".repeat(empty))
}

pub(crate) fn print_table(rows: &[Row], is_single_alias: bool, quiet: bool) {
    if is_single_alias && rows.len() == 1 {
        let row = &rows[0];
        let otp = row.cell();
        if !quiet {
            let period = row.record.period as u64;
            let rem = otp::get_remaining_seconds(period);
            let bar = progress_bar(rem, period, 20);
            eprintln!("{0} {1: <3} remaining", bar, rem.to_string() + "s");
        }
        println!("{}", otp);
    } else {
        println!("{0: <15} | {1: <10} | {2: <4}", "Alias", "OTP", "Rem");
        println!("{:-<15}-|-{:-<10}-|-{:-<4}", "", "", "");
        for row in rows {
            let otp = row.cell();
            let period = row.record.period as u64;
            let rem = otp::get_remaining_seconds(period);
            let bar = if quiet {
                String::new()
            } else {
                progress_bar(rem, period, 20)
            };
            println!(
                "{0: <15} | {1: <10} | {2: <4} {3}",
                row.record.alias,
                otp,
                rem.to_string() + "s",
                bar
            );
        }
    }
}

fn row_to_json(row: &Row) -> serde_json::Value {
    let rem = otp::get_remaining_seconds(row.record.period as u64);

    let (otp, error) = match row.otp() {
        Ok(otp) => (otp, None),
        Err(e) => (OTP_ERROR_TEXT.to_string(), Some(e)),
    };

    let mut entry = serde_json::json!({
        "alias": row.record.alias,
        "otp": otp,
        "remaining_secs": rem,
        "is_encrypted": !row.record.is_unencrypted,
        "created_at": row.record.created_at
    });

    // only present on failures, so successful entries stay unchanged
    if let Some(message) = error {
        entry["error"] = serde_json::Value::String(message.to_string());
    }

    entry
}

/// Prints rows as JSON. A single alias that matches exactly one record is
/// printed as an object, so `jq .otp` works without indexing.
fn print_json(rows: &[Row], single: bool) {
    let entries: Vec<serde_json::Value> = rows.iter().map(row_to_json).collect();

    let output = if single && entries.len() == 1 {
        entries[0].clone()
    } else {
        serde_json::Value::Array(entries)
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

pub fn rename(
    path: &Path,
    old_alias: &str,
    new_alias: &str,
    password: &Option<String>,
) -> Result<(), String> {
    if file::alias_exists(new_alias, path) {
        return Err(format!("Alias '{new_alias}' already exists."));
    }

    // read the file
    let mut codex = file::read_codex(path)?;
    let index = codex
        .records
        .iter()
        .position(|record| record.alias == old_alias)
        .ok_or_else(|| format!("Alias '{}' not found.", old_alias))?;

    // Encrypted records are bound to their alias, so the ciphertext has to be
    // re-encrypted under the new name.
    if !codex.records[index].is_unencrypted {
        let pass = get_effective_password(password)?;
        let key = vault_key(path, &mut codex, &Some(pass), KeyMode::ExistingOnly, false)?
            .ok_or("No vault key available.")?;
        let plaintext = crypto::decrypt_record(
            &codex.records[index].secret,
            &key,
            &codex.records[index].alias,
        )
        .map_err(|_| "Incorrect password. Nothing was changed.".to_string())?;

        codex.records[index].secret = crypto::encrypt_record(&plaintext, &key, new_alias)
            .map_err(|e| e.as_str().to_string())?;
    }

    codex.records[index].alias = new_alias.to_string();

    file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;

    file::write_codex_atomic(path, &codex)?;

    println!("Successfully renamed '{}' to '{}'", old_alias, new_alias);
    Ok(())
}

pub fn add_from_otpauth(
    path: &Path,
    uri: &str,
    is_unencrypt: &bool,
    password: &Option<String>,
) -> Result<(), String> {
    let parsed = OtpAuth::parse(uri)?;
    let alias = parsed.alias.clone();

    let existed = file::file_exists(path);
    let mut codex = if existed {
        file::read_codex(path)?
    } else {
        file::Codex::default()
    };

    if codex.records.iter().any(|r| r.alias == alias) {
        return Err(format!("Error: Alias '{alias}' already exists."));
    }

    let secret = if *is_unencrypt {
        parsed.secret.clone()
    } else {
        let key = vault_key(path, &mut codex, password, KeyMode::CreateIfMissing, false)?
            .ok_or("No vault key available.")?;
        crypto::encrypt_record(&parsed.secret, &key, &alias).map_err(|e| e.as_str().to_string())?
    };

    codex.records.push(Record {
        alias: alias.clone(),
        secret,
        is_unencrypted: *is_unencrypt,
        algorithm: parsed.algorithm,
        digits: parsed.digits,
        period: parsed.period,
        created_at: parsed.created_at,
    });

    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    if existed {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
    }

    file::write_codex_atomic(path, &codex)?;

    println!("Record saved: {alias}");
    Ok(())
}

pub fn add_batch(
    path: &Path,
    file_path: &str,
    is_unencrypt: &bool,
    password: &Option<String>,
) -> Result<(), String> {
    let parsed_records = OtpAuth::parse_batch(file_path)?;
    let count = parsed_records.len();

    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    let existed = file::file_exists(path);
    let mut codex = if existed {
        file::read_codex(path)?
    } else {
        file::Codex::default()
    };

    for parsed in &parsed_records {
        if codex.records.iter().any(|r| r.alias == parsed.alias) {
            return Err(format!(
                "Error: Alias '{}' already exists. Aborting batch.",
                parsed.alias
            ));
        }
    }

    let pass = if *is_unencrypt {
        None
    } else {
        Some(get_effective_password(password)?)
    };

    let key = if *is_unencrypt {
        None
    } else {
        vault_key(path, &mut codex, &pass, KeyMode::CreateIfMissing, false)?
    };

    for parsed in parsed_records {
        let secret = if *is_unencrypt {
            parsed.secret.clone()
        } else {
            crypto::encrypt_record(&parsed.secret, key.as_ref().unwrap(), &parsed.alias)
                .map_err(|e| e.as_str().to_string())?
        };

        codex.records.push(Record {
            alias: parsed.alias,
            secret,
            is_unencrypted: *is_unencrypt,
            algorithm: parsed.algorithm,
            digits: parsed.digits,
            period: parsed.period,
            created_at: parsed.created_at,
        });
    }

    if existed {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
    }

    file::write_codex_atomic(path, &codex)?;

    println!("Successfully imported {count} records.");
    Ok(())
}

pub fn export(codex_path: &Path, output_path: &Path) -> Result<(), String> {
    let codex = file::read_codex(codex_path)?;

    if codex.records.is_empty() {
        return Err("No records to export.".to_string());
    }

    let count = codex.records.len();

    // A headerless vault exports as a plain array (the old format); a v3 vault
    // exports its wrapped key so it can be restored with the master password.
    let json = if let Some(vault) = &codex.vault {
        let envelope = serde_json::json!({
            "version": 3,
            "vault": vault,
            "records": codex.records,
        });
        serde_json::to_string_pretty(&envelope)
    } else {
        serde_json::to_string_pretty(&codex.records)
    }
    .map_err(|e| format!("Serialization error: {e}"))?;

    file::ensure_dir_exists(output_path).map_err(|e| e.to_string())?;
    std::fs::write(output_path, &json)
        .map_err(|e| format!("Error writing to '{}': {e}", output_path.display()))?;
    file::secure_file(output_path).map_err(|e| e.to_string())?;

    println!("Exported {count} records to '{}'.", output_path.display());
    Ok(())
}

pub fn import(
    codex_path: &Path,
    input_path: &Path,
    password: &Option<String>,
) -> Result<(), String> {
    let content = std::fs::read_to_string(input_path)
        .map_err(|e| format!("Error reading '{}': {e}", input_path.display()))?;

    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON in '{}': {e}", input_path.display()))?;

    let (source_vault, imported): (Option<String>, Vec<Record>) = match value {
        serde_json::Value::Array(_) => {
            let records = serde_json::from_value(value)
                .map_err(|e| format!("Invalid JSON in '{}': {e}", input_path.display()))?;
            (None, records)
        }
        serde_json::Value::Object(_) => {
            #[derive(serde::Deserialize)]
            struct Envelope {
                version: u8,
                vault: String,
                records: Vec<Record>,
            }

            let envelope: Envelope = serde_json::from_value(value)
                .map_err(|e| format!("Invalid vault file '{}': {e}", input_path.display()))?;

            if envelope.version != 3 {
                return Err(format!("Unsupported vault version {}.", envelope.version));
            }

            (Some(envelope.vault), envelope.records)
        }
        _ => return Err("Invalid JSON: expected an array or a vault object.".to_string()),
    };

    if imported.is_empty() {
        return Err("No records found in import file.".to_string());
    }

    let mut codex = if file::file_exists(codex_path) {
        file::read_codex(codex_path)?
    } else {
        file::Codex::default()
    };

    for record in &imported {
        if codex.records.iter().any(|r| r.alias == record.alias) {
            return Err(format!(
                "Error: Alias '{}' already exists in codex. Aborting import.",
                record.alias
            ));
        }
    }

    let duplicates: Vec<&str> = imported
        .iter()
        .enumerate()
        .filter(|(i, r)| imported.iter().skip(i + 1).any(|r2| r2.alias == r.alias))
        .map(|(_, r)| r.alias.as_str())
        .collect();

    if !duplicates.is_empty() {
        return Err(format!(
            "Error: Duplicate aliases in import file: {}. Aborting.",
            duplicates.join(", ")
        ));
    }

    file::ensure_dir_exists(codex_path).map_err(|e| e.to_string())?;

    // A vault envelope carries records encrypted with its own key, so they
    // have to be re-encrypted under the destination vault key.
    let imported = if let Some(source_vault) = source_vault {
        let pass = get_effective_password(password)?;
        let source_key = crypto::unwrap_vault_key(&source_vault, &pass)
            .map_err(|_| "Incorrect password for the imported vault.".to_string())?;
        let destination_key = vault_key(
            codex_path,
            &mut codex,
            &Some(pass),
            KeyMode::CreateIfMissing,
            false,
        )?
        .ok_or("No vault key available.")?;

        let mut converted = Vec::with_capacity(imported.len());
        for mut record in imported {
            if !record.is_unencrypted {
                let plaintext = crypto::decrypt_record(&record.secret, &source_key, &record.alias)
                    .map_err(|_| {
                        format!("Could not decrypt imported record '{}'.", record.alias)
                    })?;
                record.secret = crypto::encrypt_record(&plaintext, &destination_key, &record.alias)
                    .map_err(|e| e.as_str().to_string())?;
            }
            converted.push(record);
        }
        converted
    } else {
        imported
    };

    if file::file_exists(codex_path) {
        file::create_routine_backup(codex_path)
            .map_err(|e| format!("Warning: Backup failed: {e}"))?;
    }

    for record in imported {
        codex.records.push(record);
    }

    file::write_codex_atomic(codex_path, &codex)?;

    let count = codex.records.len();
    println!("Successfully imported {count} records total.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::progress_bar;

    #[test]
    fn bar_fills_against_the_record_period() {
        // 30 second period
        assert_eq!(progress_bar(30, 30, 20), format!("[{}]", "#".repeat(20)));
        assert_eq!(progress_bar(0, 30, 20), format!("[{}]", ".".repeat(20)));

        // 60 second period: half way is half the bar, not a full one
        assert_eq!(progress_bar(60, 60, 20), format!("[{}]", "#".repeat(20)));
        assert_eq!(
            progress_bar(30, 60, 20),
            format!("[{}]", "#".repeat(10) + &".".repeat(10))
        );
        assert_eq!(progress_bar(0, 60, 20), format!("[{}]", ".".repeat(20)));
    }

    #[test]
    fn bar_does_not_divide_by_zero() {
        assert_eq!(progress_bar(0, 0, 20), format!("[{}]", ".".repeat(20)));
    }
}
