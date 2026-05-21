use crate::args::OutputFormat;
use crate::file;
use crate::models::Record;
use crate::otp;
use crate::ui::Table;
use data_encoding::BASE32_NOPAD;
use std::io;
use std::path::{Path, PathBuf};

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
        .or_else(|| std::env::var("HERMES_PASSWORD").ok())
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
pub fn add(
    path: &Path,
    alias: &str,
    code: &str,
    is_unencrypt: &bool,
    password: &Option<String>,
) -> Result<(), String> {
    let clean_code = sanitize_and_validate_code(code)?;

    // for Legacy file format
    if alias.contains(':') {
        return Err("Error: Alias cannot contain ':'".into());
    }

    if file::file_exists(path) && file::alias_exists(alias, path) {
        return Err(format!("Error: Alias '{alias}' already exists."));
    }

    // encrypt if necessary
    let secret = if *is_unencrypt {
        clean_code.to_string()
    } else {
        otp::encrypt(&clean_code.to_string(), &get_effective_password(password)?)
    };

    // serialize and save
    let record = Record::new(alias.to_string(), secret.to_string(), *is_unencrypt);
    let json_data = serde_json::to_string(&record).map_err(|e| e.to_string())?;

    file::ensure_dir_exists(path).map_err(|e| e.to_string())?;

    if file::file_exists(path) {
        file::create_routine_backup(path).map_err(|e| format!("Warning: Backup failed: {}", e))?;
        file::append_to_file(path, &json_data).map_err(|e| e.to_string())?;
    } else {
        file::overwrite_file(path, &json_data).map_err(|e| e.to_string())?;
    }

    println!("Record saved.");

    match otp::generate_otp(&clean_code) {
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

    // Check if the alias even exists before we do anything else
    if !file::alias_exists(alias, path) {
        return Err(format!("No record for '{alias}' found."));
    }

    // Resolve password once (if needed)
    let pass = (!*is_unencrypt)
        .then(|| get_effective_password(password))
        .transpose()?;

    // Do the swap
    remove(path, alias)?;
    add(path, alias, &clean_code, is_unencrypt, &pass)?;
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

pub fn ls(
    path: &Path,
    alias_filter: &Option<String>,
    is_unencrypt: bool,
    password: &Option<String>,
    format: &OutputFormat,
    quiet: bool,
    exact_match: bool,
) -> Result<(), String> {
    let records = file::read_codex(path)?;

    // apply search filter
    let filtered: Vec<&Record> = if let Some(filter) = alias_filter {
        let filter_lower = filter.to_lowercase();
        records
            .iter()
            .filter(|r| {
                let alias_lower = r.alias.to_lowercase();
                if exact_match {
                    alias_lower == filter_lower
                } else {
                    alias_lower.contains(&filter_lower)
                }
            })
            .collect()
    } else {
        records.iter().collect()
    };

    if filtered.is_empty() {
        return Err(if let Some(filter) = alias_filter {
            if exact_match {
                format!("No record found with exact alias '{}'", filter)
            } else {
                format!("No records found matching '{}'", filter)
            }
        } else {
            "No records found".into()
        });
    }

    let needs_password = !is_unencrypt && filtered.iter().any(|r| !r.is_unencrypted);

    let pass = needs_password
        .then(|| get_effective_password(password))
        .transpose()?
        .unwrap_or_default();

    let rem = otp::get_remaining_seconds();
    match format {
        OutputFormat::Json => print_json(&filtered, &pass, rem),
        OutputFormat::Table => {
            if quiet {
                print_table(&filtered, &pass, rem, alias_filter.is_some(), quiet);
            } else {
                let table = Table::new(&filtered, &pass, alias_filter.is_some());
                table.render();
            }
        }
    }

    Ok(())
}

fn get_otp_display(record: &Record, pass: &str) -> String {
    let secret = if record.is_unencrypted {
        Ok(record.secret.clone())
    } else {
        otp::decrypt(&record.secret, pass)
    };

    secret
        .and_then(|s| otp::generate_otp(&s).map_err(|_| otp::OtpError::InvalidBase32))
        .unwrap_or_else(|_| "Error Invalid secret or decryption failed".to_string())
}

pub fn print_table(records: &[&Record], pass: &str, rem: u64, is_single_alias: bool, quiet: bool) {
    let mut bar = String::from("");
    if !quiet {
        let bar_width = 20;
        let safe_rem = rem.min(30) as usize;

        let filled = (safe_rem * bar_width) / 30;
        let empty = bar_width - filled;
        bar = format!("[{0}{1}]", "#".repeat(filled), ".".repeat(empty));
    }
    if is_single_alias && records.len() == 1 {
        let otp = get_otp_display(records[0], pass);
        if !quiet {
            eprintln!("{0} {1: <3} remaining", bar, rem.to_string() + "s");
        }
        println!("{}", otp);
    } else {
        println!("{0: <15} | {1: <10} | {2: <4}", "Alias", "OTP", "Rem");
        println!("{:-<15}-|-{:-<10}-|-{:-<4}", "", "", "");
        for r in records {
            let otp = get_otp_display(r, pass);
            println!(
                "{0: <15} | {1: <10} | {2: <3} {3}",
                r.alias,
                otp,
                rem.to_string() + "s",
                bar
            );
        }
    }
}

fn print_json(records: &[&Record], pass: &str, rem: u64) {
    let list: Vec<serde_json::Value> = records
        .iter()
        .map(|r| {
            serde_json::json!({
                "alias": r.alias,
                "otp": get_otp_display(r, pass),
                "remaining_secs": rem,
                "is_encrypted": !r.is_unencrypted,
                "created_at": r.created_at
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&list).unwrap());
}

pub fn migrate(path: &PathBuf) -> io::Result<()> {
    // create backup
    let backup_path = file::create_snapshot_backup(path)?;
    println!("Backup created at {:?}", backup_path);

    // read and parse everything using the hybrid parser
    let records =
        file::read_legacy_raw(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Serialize to JSON format
    let migrated_records: Vec<String> = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<String>, serde_json::Error>>()
        .map_err(|e| io::Error::other(e))?;

    let count = migrated_records.len();
    let new_content = migrated_records.join("\n") + "\n";

    file::overwrite_file(path, &new_content)?;

    println!("Successfully migrated {count} records to JSON format.");

    Ok(())
}

pub fn rename(path: &PathBuf, old_alias: &str, new_alias: &str) -> Result<(), String> {
    // for Legacy file format
    if new_alias.contains(':') {
        return Err("The new alias cannot contain ':'".to_string());
    }

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
