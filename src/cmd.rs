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
use std::path::{Path, PathBuf};

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

    if file::file_exists(path) && file::alias_exists(alias, path) {
        return Err(format!("Error: Alias '{alias}' already exists."));
    }

    // encrypt if necessary
    let secret = if *is_unencrypt {
        clean_code.to_string()
    } else {
        crypto::encrypt_v2(&clean_code, &get_effective_password(password)?)
            .map_err(|e| e.as_str().to_string())?
    };

    // serialize and save
    let record = Record::new(
        alias.to_string(),
        secret.to_string(),
        *is_unencrypt,
        algorithm.to_string(),
        digits as u8,
        period,
    );
    let json_data = serde_json::to_string(&record).map_err(|e| e.to_string())?;

    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    if file::file_exists(path) {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
        file::append_to_file(path, &json_data).map_err(|e| e.to_string())?;
    } else {
        file::overwrite_file(path, &json_data).map_err(|e| e.to_string())?;
    }

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
    let records = file::read_codex(path)?;
    let existing = records
        .iter()
        .find(|r| r.alias == *alias)
        .ok_or_else(|| format!("No record for '{alias}' found."))?;
    let target_is_encrypted = !existing.is_unencrypted;

    // The record's password is needed to prove access whenever the record is
    // encrypted, even if the replacement will be stored in plaintext (-u).
    let need_password = target_is_encrypted || !*is_unencrypt;
    let pass = need_password
        .then(|| get_effective_password(password))
        .transpose()?;

    // Verify against this record only: each record is encrypted on its own,
    // so there is no vault-wide password to check.
    if target_is_encrypted {
        let p = pass
            .as_ref()
            .ok_or("Incorrect password. Nothing was changed.")?;
        decrypt_secret(&existing.secret, p)
            .map_err(|_| "Incorrect password. Nothing was changed.".to_string())?;
    }

    // Do the swap, keeping the record's OTP parameters.
    remove(path, alias)?;
    add(
        path,
        alias,
        &clean_code,
        &existing.algorithm,
        existing.digits as u32,
        existing.period,
        is_unencrypt,
        &pass,
    )?;
    println!("Record for '{alias}' successfully updated.");
    Ok(())
}

pub fn remove(path: &Path, alias: &str) -> Result<(), String> {
    file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;

    let records = file::read_codex(path)?;
    let original_len = records.len();

    let filtered_records: Vec<Record> = records.into_iter().filter(|r| r.alias != alias).collect();

    if filtered_records.len() == original_len {
        return Err(format!("Error: No record for '{alias}' found"));
    }

    // Convert the Records back to JSON
    let lines: Vec<String> = filtered_records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| format!("Serialization error: {e}"))?;

    let data = lines.join("\n") + "\n";

    file::overwrite_file(path, &data).map_err(|e| format!("Error: Failed to save changes: {e}"))?;

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
    let mut records = file::read_codex(path)?;

    // Indices of the matching records, in display order.
    let matched: Vec<usize> = if let Some(filter) = alias_filter {
        if fuzzy {
            let mut matcher = nucleo_matcher::Matcher::new(Config::DEFAULT);

            let mut scored: Vec<(u32, usize)> = records
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
            records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.alias == *filter)
                .map(|(index, _)| index)
                .collect()
        } else {
            // partial match is case-insensitive
            let filter_lower = filter.to_lowercase();
            records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.alias.to_lowercase().contains(&filter_lower))
                .map(|(index, _)| index)
                .collect()
        }
    } else {
        (0..records.len()).collect()
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

    let needs_password =
        !is_unencrypt && matched.iter().any(|&index| !records[index].is_unencrypted);

    let pass = needs_password
        .then(|| get_effective_password(password))
        .transpose()?
        .unwrap_or_default();

    // Decrypt each matched record exactly once, migrating legacy payloads in
    // place. This keeps Argon2 out of the render and live-redraw paths.
    let mut plaintext: Vec<Option<String>> = Vec::with_capacity(matched.len());
    let mut decrypt_errors: Vec<Option<String>> = Vec::with_capacity(matched.len());
    let mut migrated = false;

    for &index in &matched {
        let record = &mut records[index];

        if is_unencrypt || record.is_unencrypted {
            plaintext.push(Some(record.secret.clone()));
            decrypt_errors.push(None);
            continue;
        }

        match decrypt_and_migrate(record, &pass, &mut migrated) {
            Ok(secret) => {
                plaintext.push(Some(secret));
                decrypt_errors.push(None);
            }
            Err(e) => {
                plaintext.push(None);
                decrypt_errors.push(Some(e.as_str().to_string()));
            }
        }
    }

    if migrated {
        match save_migrated(path, &records) {
            Ok(_) => {
                if !quiet {
                    eprintln!(
                        "Migrated records to the v2 format; backup at {}",
                        path.with_extension("pre-v2.bak").display()
                    );
                }
            }
            Err(e) => eprintln!("Warning: could not write the migrated codex: {e}"),
        }
    }

    let rows: Vec<Row> = matched
        .iter()
        .enumerate()
        .map(|(position, &index)| Row {
            record: &records[index],
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

/// Decrypts a record's secret, accepting both legacy and v2 payloads.
fn decrypt_secret(secret: &str, password: &str) -> Result<String, crypto::CryptoError> {
    if crypto::is_v2(secret) {
        crypto::decrypt_v2(secret, password)
    } else {
        crypto::decrypt_legacy(secret, password)
    }
}

/// Decrypts `record`, upgrading a legacy payload to v2 when necessary.
fn decrypt_and_migrate(
    record: &mut Record,
    password: &str,
    migrated: &mut bool,
) -> Result<String, crypto::CryptoError> {
    if crypto::is_v2(&record.secret) {
        return crypto::decrypt_v2(&record.secret, password);
    }

    let secret = crypto::decrypt_legacy(&record.secret, password)?;
    let upgraded = crypto::encrypt_v2(&secret, password)?;

    record.secret = upgraded;
    *migrated = true;

    Ok(secret)
}

/// Snapshots the codex once and writes the migrated records atomically.
fn save_migrated(path: &Path, records: &[Record]) -> Result<Option<PathBuf>, String> {
    let backup = file::backup_pre_v2(path).map_err(|e| format!("backup failed: {e}"))?;

    let lines: Vec<String> = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| format!("Serialization error: {e}"))?;

    let content = lines.join("\n") + "\n";
    file::overwrite_file_atomic(path, &content).map_err(|e| format!("write failed: {e}"))?;

    Ok(backup)
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

pub fn rename(path: &Path, old_alias: &str, new_alias: &str) -> Result<(), String> {
    if file::alias_exists(new_alias, path) {
        return Err(format!("Alias '{new_alias}' already exists."));
    }

    // read the file
    let mut records = file::read_codex(path)?;
    let mut found = false;

    for record in records.iter_mut() {
        if record.alias == old_alias {
            record.alias = new_alias.to_string();
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Alias '{}' not found.", old_alias));
    }

    file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;

    let lines: Vec<String> = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| format!("Serialization error: {e}"))?;

    let data = lines.join("\n") + "\n";

    file::overwrite_file(path, &data).map_err(|e| format!("Error saving changes: {e}"))?;

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

    if file::file_exists(path) && file::alias_exists(&alias, path) {
        return Err(format!("Error: Alias '{alias}' already exists."));
    }

    let secret = if *is_unencrypt {
        parsed.secret.clone()
    } else {
        crypto::encrypt_v2(&parsed.secret, &get_effective_password(password)?)
            .map_err(|e| e.as_str().to_string())?
    };

    let record = Record {
        alias: alias.clone(),
        secret,
        is_unencrypted: *is_unencrypt,
        algorithm: parsed.algorithm,
        digits: parsed.digits,
        period: parsed.period,
        created_at: parsed.created_at,
    };

    let json_data = serde_json::to_string(&record).map_err(|e| e.to_string())?;
    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    if file::file_exists(path) {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
        file::append_to_file(path, &json_data).map_err(|e| e.to_string())?;
    } else {
        file::overwrite_file(path, &json_data).map_err(|e| e.to_string())?;
    }

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

    let mut current_records = if file::file_exists(path) {
        file::read_codex(path)?
    } else {
        Vec::new()
    };

    for parsed in &parsed_records {
        if current_records.iter().any(|r| r.alias == parsed.alias) {
            return Err(format!(
                "Error: Alias '{}' already exists. Aborting batch.",
                parsed.alias
            ));
        }
    }

    if file::file_exists(path) {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
    }

    let pass = if *is_unencrypt {
        None
    } else {
        Some(get_effective_password(password)?)
    };

    for parsed in parsed_records {
        let secret = if *is_unencrypt {
            parsed.secret.clone()
        } else {
            crypto::encrypt_v2(&parsed.secret, pass.as_ref().unwrap())
                .map_err(|e| e.as_str().to_string())?
        };

        let record = Record {
            alias: parsed.alias,
            secret,
            is_unencrypted: *is_unencrypt,
            algorithm: parsed.algorithm,
            digits: parsed.digits,
            period: parsed.period,
            created_at: parsed.created_at,
        };

        current_records.push(record);
    }

    let lines: Vec<String> = current_records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| format!("Serialization error: {e}"))?;

    let content = lines.join("\n") + "\n";
    file::overwrite_file(path, &content)
        .map_err(|e| format!("Error: Failed to save changes: {e}"))?;

    println!("Successfully imported {count} records.");
    Ok(())
}

pub fn export(codex_path: &Path, output_path: &Path) -> Result<(), String> {
    let records = file::read_codex(codex_path)?;

    if records.is_empty() {
        return Err("No records to export.".to_string());
    }

    let json =
        serde_json::to_string_pretty(&records).map_err(|e| format!("Serialization error: {e}"))?;

    file::ensure_dir_exists(output_path).map_err(|e| e.to_string())?;
    std::fs::write(output_path, &json)
        .map_err(|e| format!("Error writing to '{}': {e}", output_path.display()))?;
    file::secure_file(output_path).map_err(|e| e.to_string())?;

    println!(
        "Exported {} records to '{}'.",
        records.len(),
        output_path.display()
    );
    Ok(())
}

pub fn import(codex_path: &Path, input_path: &Path) -> Result<(), String> {
    let content = std::fs::read_to_string(input_path)
        .map_err(|e| format!("Error reading '{}': {e}", input_path.display()))?;

    let imported: Vec<Record> = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON in '{}': {e}", input_path.display()))?;

    if imported.is_empty() {
        return Err("No records found in import file.".to_string());
    }

    let existing = if file::file_exists(codex_path) {
        file::read_codex(codex_path)?
    } else {
        Vec::new()
    };

    for record in &imported {
        if existing.iter().any(|r| r.alias == record.alias) {
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

    if file::file_exists(codex_path) {
        file::create_routine_backup(codex_path)
            .map_err(|e| format!("Warning: Backup failed: {e}"))?;
    }

    let mut all_records = existing;
    for record in imported {
        all_records.push(record);
    }

    let lines: Vec<String> = all_records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| format!("Serialization error: {e}"))?;

    let content = lines.join("\n") + "\n";
    file::overwrite_file(codex_path, &content)
        .map_err(|e| format!("Error: Failed to save changes: {e}"))?;

    let count = all_records.len();
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
