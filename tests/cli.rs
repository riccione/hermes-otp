use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::NamedTempFile;

const CODE: &str = "BQZH47HMIUUQOQVAXO3MCRUP3OGR3OIL";
const ALIAS: &str = "test_simple";
const PASSWORD: &str = "password";

/// helper fn hermes pointing to a temp file
fn hermes(path: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = cargo_bin_cmd!("hermes-otp");
    cmd.arg("--path").arg(path);
    cmd
}

/// Flips one ciphertext character of the first record, keeping it decodable.
fn tamper_first_record(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut lines = content.lines().filter(|line| !line.trim().is_empty());
    let header = lines.next().ok_or("missing vault header")?;
    let record_line = lines.next().ok_or("missing record")?;
    let mut record: serde_json::Value = serde_json::from_str(record_line)?;

    let secret = record["secret"].as_str().unwrap().to_string();
    let parts: Vec<&str> = secret.split('$').collect();
    assert_eq!(parts.len(), 3, "expected a v3 payload");

    let mut ciphertext: Vec<char> = parts[2].chars().collect();
    let position = ciphertext.iter().position(|c| *c != '=').unwrap();
    ciphertext[position] = if ciphertext[position] == 'A' {
        'B'
    } else {
        'A'
    };

    record["secret"] = serde_json::Value::String(format!(
        "{}${}${}",
        parts[0],
        parts[1],
        ciphertext.into_iter().collect::<String>()
    ));

    std::fs::write(
        path,
        format!("{}\n{}\n", header, serde_json::to_string(&record)?),
    )?;
    Ok(())
}

#[test]
fn fail_run_with_no_args() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Usage:"))
        .stderr(predicate::str::contains("<COMMAND>"));

    Ok(())
}

#[test]
fn fail_add_missing_args() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("alias is required"));

    Ok(())
}

#[test]
fn add_remove_isolated_flow() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args([ALIAS, "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("remove")
        .args([ALIAS])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Record for {} removed.",
            ALIAS
        )));

    Ok(())
}

#[test]
fn add_update_remove_isolated_flow() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();
    let alias = "test_update";
    let stdout_removed = format!("Record for {alias} removed.");

    hermes(path)
        .arg("add")
        .args([alias, "-c", CODE, "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("[0-9]{6}")?);

    hermes(path)
        .arg("update")
        .args([alias, "-c", CODE, "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Record for '{alias}' successfully updated."
        )));

    hermes(path)
        .arg("remove")
        .args([alias])
        .assert()
        .success()
        .stdout(predicate::str::contains(stdout_removed));

    Ok(())
}

#[test]
fn rename_alias_isolated_flow() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    // add two initial records
    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["google", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // rename: github -> gh
    hermes(path)
        .arg("rename")
        .args(["github", "gh", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully renamed"));

    // verify: new alias exists, old alias is gone
    hermes(path)
        .arg("ls")
        .args(["gh"])
        .args(["--password", PASSWORD])
        .arg("-q")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^\d{6}")?);

    hermes(path)
        .arg("ls")
        .args(["github"])
        .args(["--password", PASSWORD])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No records found matching 'github'",
        ));

    // collision Check: try to rename 'gh' to 'google' (exists)
    hermes(path)
        .arg("rename")
        .args(["gh", "google", "--password", PASSWORD])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    Ok(())
}

#[test]
fn ls_partial_search_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    // add multiple records with similar prefixes
    hermes(path)
        .arg("add")
        .args(["google", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["goodreads", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // test partial search: "goo" should return google and goodreads only
    hermes(path)
        .arg("ls")
        .args(["goo"])
        .args(["--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::contains("google"))
        .stdout(predicate::str::contains("goodreads"))
        .stdout(predicate::str::contains("github").count(0)); // no github

    // test non-matching search
    hermes(path)
        .arg("ls")
        .args(["no_match"])
        .args(["--password", PASSWORD])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn ls_exact_match_is_case_sensitive() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["Google", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    // exact match with the same case should find the record
    hermes(path)
        .arg("ls")
        .args(["--exact", "Google", "--password", PASSWORD])
        .arg("-q")
        .assert()
        .success();

    // exact match with a different case should not find the record
    hermes(path)
        .arg("ls")
        .args(["--exact", "google", "--password", PASSWORD])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No record found with exact alias 'google'",
        ));

    hermes(path)
        .arg("ls")
        .args(["--exact", "GOOGLE", "--password", PASSWORD])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No record found with exact alias 'GOOGLE'",
        ));

    // partial match stays case-insensitive
    hermes(path)
        .arg("ls")
        .args(["google", "--password", PASSWORD])
        .arg("-q")
        .assert()
        .success();

    Ok(())
}

#[test]
fn ls_fuzzy_search_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["google", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["goodreads", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // fuzzy "ggl" should match "google"
    hermes(path)
        .arg("ls")
        .args(["-z", "ggl", "--password", PASSWORD])
        .arg("-q")
        .assert()
        .success();

    // fuzzy "gih" should match "github" (g, i, h in order)
    hermes(path)
        .arg("ls")
        .args(["-z", "gih", "--password", PASSWORD])
        .arg("-q")
        .assert()
        .success();

    // fuzzy "oo" should match "google" and "goodreads"
    hermes(path)
        .arg("ls")
        .args(["-z", "oo", "--password", PASSWORD])
        .arg("-q")
        .assert()
        .success();

    // fuzzy "nope" should match nothing
    hermes(path)
        .arg("ls")
        .args(["-z", "nope", "--password", PASSWORD])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn ls_json_format_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    // add multiply records
    let entries = vec![("apple", CODE), ("banana", CODE)];
    for (alias, code) in &entries {
        hermes(path)
            .arg("add")
            .args([alias, "-c", code, "--password", PASSWORD])
            .assert()
            .success();
    }

    // ls with JSON format
    let output = hermes(path)
        .arg("ls")
        .args(["--password", PASSWORD])
        .args(["--format", "json"])
        .output()?;

    // parse the actual JSON
    // it will panic if the output is not valid JSON
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // verify structure
    assert!(json.is_array(), "Output should be a JSON array");

    let array = json.as_array().unwrap();
    assert_eq!(array.len(), 2, "Should contain exactly 2 records");

    // check if the first entry contains the expected key
    let first_record = &array[0];
    assert!(
        first_record.get("alias").is_some(),
        "Record missing 'alias' field"
    );

    // verify specific content
    let has_apple = array.iter().any(|r| r["alias"] == "apple");
    assert!(has_apple, "JSON output missing 'apple' alias");

    Ok(())
}

#[test]
fn test_ls_handles_mashed_json_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    // Manually construct raw JSON strings
    let record1 = format!(
        r#"{{"alias":"github","secret":"{}","is_unencrypted":true,"algorithm":"sha256","created_at":1700000000}}"#,
        CODE
    );
    let record2 = format!(
        r#"{{"alias":"google","secret":"{}","is_unencrypted":true,"algorithm":"sha256","created_at":1700000001}}"#,
        CODE
    );
    let mashed_json = format!("{}{}", record1, record2);

    std::fs::write(file.path(), mashed_json)?;

    let assert = hermes(path)
        .arg("ls")
        .args(["--password", PASSWORD])
        .assert();

    assert
        .success()
        .stdout(predicates::str::contains("github"))
        .stdout(predicates::str::contains("google"))
        .stdout(predicates::str::contains("{\"alias\"").not())
        .stdout(predicates::str::contains("Error").not());

    Ok(())
}

#[test]
fn export_creates_valid_json() -> Result<(), Box<dyn std::error::Error>> {
    let codex = NamedTempFile::new()?;
    let export_file = NamedTempFile::new()?;
    let export_path = export_file.path().to_path_buf();

    hermes(codex.path())
        .arg("add")
        .args(["alpha", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(codex.path())
        .arg("add")
        .args(["beta", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(codex.path())
        .arg("export")
        .arg("-o")
        .arg(&export_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported 2 records"));

    let content = std::fs::read_to_string(&export_path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    // a vault is exported as an envelope carrying the wrapped key
    assert_eq!(json["version"], 3);
    assert!(json["vault"].as_str().unwrap().starts_with("v3w$"));
    assert_eq!(json["records"].as_array().unwrap().len(), 2);

    Ok(())
}

#[test]
fn import_from_json_file() -> Result<(), Box<dyn std::error::Error>> {
    let codex = NamedTempFile::new()?;
    let json_file = NamedTempFile::new()?;

    let records = serde_json::json!([
        {
            "alias": "imported_one",
            "secret": CODE,
            "is_unencrypted": true,
            "algorithm": "sha1",
            "digits": 6,
            "period": 30,
            "created_at": 1000000
        },
        {
            "alias": "imported_two",
            "secret": CODE,
            "is_unencrypted": true,
            "algorithm": "sha1",
            "digits": 6,
            "period": 30,
            "created_at": 1000001
        }
    ]);
    std::fs::write(json_file.path(), serde_json::to_string_pretty(&records)?)?;

    hermes(codex.path())
        .arg("import")
        .arg(json_file.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("2 records"));

    hermes(codex.path())
        .arg("ls")
        .arg("imported_one")
        .arg("-q")
        .assert()
        .success();

    hermes(codex.path())
        .arg("ls")
        .arg("imported_two")
        .arg("-q")
        .assert()
        .success();

    Ok(())
}

#[test]
fn import_rejects_duplicate_alias() -> Result<(), Box<dyn std::error::Error>> {
    let codex = NamedTempFile::new()?;
    let json_file = NamedTempFile::new()?;

    hermes(codex.path())
        .arg("add")
        .args(["existing", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    let records = serde_json::json!([{
        "alias": "existing",
        "secret": CODE,
        "is_unencrypted": true,
        "algorithm": "sha1",
        "digits": 6,
        "period": 30,
        "created_at": 1000000
    }]);
    std::fs::write(json_file.path(), serde_json::to_string_pretty(&records)?)?;

    hermes(codex.path())
        .arg("import")
        .arg(json_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    Ok(())
}

#[test]
fn import_rejects_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
    let codex = NamedTempFile::new()?;
    let json_file = NamedTempFile::new()?;

    std::fs::write(json_file.path(), "not valid json {{{")?;

    hermes(codex.path())
        .arg("import")
        .arg(json_file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid JSON"));

    Ok(())
}

#[test]
fn ls_wrong_password_exits_with_error() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // wrong password: the vault key cannot be unwrapped, so this must not exit 0
    hermes(path)
        .arg("ls")
        .args(["github", "-q", "--password", "wrong-password"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Incorrect password"));

    Ok(())
}

#[test]
fn ls_partial_failure_exits_with_partial_code() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    // one working plaintext record
    hermes(path)
        .arg("add")
        .args(["good", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    // one record whose secret is not valid base32
    let mut content = std::fs::read_to_string(path)?;
    content.push_str(
        r#"{"alias":"corrupt","secret":"NOT!BASE32","is_unencrypted":true,"algorithm":"sha1","digits":6,"period":30,"created_at":1}"#,
    );
    std::fs::write(path, content)?;

    // some records worked, some did not -> partial exit code
    hermes(path)
        .arg("ls")
        .args(["-q", "--password", PASSWORD])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("1 of 2 records failed"))
        .stderr(predicate::str::contains("corrupt"));

    Ok(())
}

#[test]
fn ls_json_reports_error_field() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // a tampered record fails authentication while the vault key still opens
    tamper_first_record(path)?;

    let output = hermes(path)
        .arg("ls")
        .args(["--password", PASSWORD, "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let entry = &json.as_array().unwrap()[0];

    // the failure is reported in a dedicated field
    assert_eq!(entry["error"], "authentication failed");
    // and the otp field still carries a visible marker
    assert!(entry["otp"].as_str().unwrap().contains("decryption failed"));

    Ok(())
}

#[test]
fn ls_json_omits_error_field_on_success() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    let output = hermes(path)
        .arg("ls")
        .args(["--password", PASSWORD, "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let entry = &json.as_array().unwrap()[0];

    // successful entries stay unchanged
    assert!(entry.get("error").is_none());

    Ok(())
}

#[test]
fn ls_json_single_alias_is_object() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["git", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    let output = hermes(path)
        .arg("ls")
        .args(["git", "--password", PASSWORD, "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // a unique alias match is a single object, so `jq .otp` works directly
    assert!(json.is_object(), "single alias must be a JSON object");
    assert_eq!(json["alias"], "git");
    assert!(
        json["otp"].as_str().is_some_and(|otp| otp.len() == 6),
        "otp should be a 6 character string"
    );

    Ok(())
}

#[test]
fn ls_json_partial_match_multiple_is_array() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    for alias in ["google", "goodreads", "github"] {
        hermes(path)
            .arg("add")
            .args([alias, "-c", CODE, "--password", PASSWORD, "-u"])
            .assert()
            .success();
    }

    let output = hermes(path)
        .arg("ls")
        .args(["goo", "--password", PASSWORD, "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // an alias matching several records cannot be a single object
    let array = json
        .as_array()
        .expect("multiple matches must stay a JSON array");
    assert_eq!(array.len(), 2, "goo should match google and goodreads");
    assert!(array.iter().all(|r| r["alias"] != "github"));

    Ok(())
}

#[test]
fn ls_json_single_alias_error_is_object() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    tamper_first_record(path)?;

    let output = hermes(path)
        .arg("ls")
        .args(["aws", "--password", PASSWORD, "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // the failure path keeps the object shape and reports the reason
    assert!(json.is_object(), "single alias must be a JSON object");
    assert_eq!(json["alias"], "aws");
    assert_eq!(json["error"], "authentication failed");

    Ok(())
}

#[test]
fn update_wrong_password_leaves_record_intact() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // updating with a wrong password must not replace the secret
    hermes(path)
        .arg("update")
        .args(["github", "-c", CODE, "--password", "wrong-password"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Incorrect password"))
        .stderr(predicate::str::contains("Nothing was changed"));

    // the original password still works
    hermes(path)
        .arg("ls")
        .args(["github", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn update_u_verifies_before_converting_to_plaintext() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // -u must not let anyone overwrite an encrypted record without the password
    hermes(path)
        .arg("update")
        .args(["github", "-c", CODE, "-u", "--password", "wrong-password"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Incorrect password"));

    // the record is still encrypted with the original password
    hermes(path)
        .arg("ls")
        .args(["github", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    // with the right password, -u converts the record to plaintext
    hermes(path)
        .arg("update")
        .args(["github", "-c", CODE, "-u", "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("ls")
        .args(["github", "-q"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn update_reencrypts_with_same_password() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("update")
        .args(["github", "-c", "GEZDGNBVGY3TQOJQ", "--password", PASSWORD])
        .assert()
        .success();

    // the replacement is encrypted with the same password
    hermes(path)
        .arg("ls")
        .args(["github", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn update_leaves_other_records_untouched() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    let before = std::fs::read_to_string(path)?;
    let aws_line = before
        .lines()
        .find(|line| line.contains("\"aws\""))
        .unwrap();

    hermes(path)
        .arg("update")
        .args(["github", "-c", "GEZDGNBVGY3TQOJQ", "--password", PASSWORD])
        .assert()
        .success();

    // the untouched record keeps its ciphertext and still opens
    let after = std::fs::read_to_string(path)?;
    assert!(after.contains(aws_line), "aws record must be untouched");

    hermes(path)
        .arg("ls")
        .args(["aws", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn add_rejects_a_different_master_password() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", "pw-github"])
        .assert()
        .success();

    // the vault has one master password; a second one must not be accepted
    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", "pw-aws"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Incorrect password"));

    Ok(())
}

#[cfg(unix)]
mod unix_permissions {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    fn add_record(path: &std::path::Path, alias: &str) {
        hermes(path)
            .arg("add")
            .args([alias, "-c", CODE, "--password", PASSWORD])
            .assert()
            .success();
    }

    #[test]
    fn codex_file_and_dir_are_owner_only() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let nested = dir.path().join("nested");
        let codex = nested.join("codex");

        add_record(&codex, "github");

        assert_eq!(mode(&codex), 0o600, "codex must be 0600");
        assert_eq!(mode(&nested), 0o700, "config dir must be 0700");

        Ok(())
    }

    #[test]
    fn existing_wide_codex_is_repaired() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let nested = dir.path().join("nested");
        let codex = nested.join("codex");

        std::fs::create_dir_all(&nested)?;
        std::fs::write(&codex, "")?;
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755))?;
        assert_eq!(mode(&codex), 0o755);

        add_record(&codex, "github");

        assert_eq!(mode(&codex), 0o600, "pre-existing codex must be repaired");

        Ok(())
    }

    #[test]
    fn custom_parent_is_not_repermissioned() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let codex = dir.path().join("codex");

        // a user-supplied parent must never be chmod'd
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755))?;

        add_record(&codex, "github");

        assert_eq!(mode(dir.path()), 0o755, "custom parent must be left alone");
        assert_eq!(mode(&codex), 0o600);

        Ok(())
    }

    #[test]
    fn backup_is_owner_only() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let nested = dir.path().join("nested");
        let codex = nested.join("codex");

        add_record(&codex, "github");
        // a second write creates codex.bak
        add_record(&codex, "aws");

        let backup = nested.join("codex.bak");
        assert!(backup.exists(), "backup should exist");
        assert_eq!(mode(&backup), 0o600, "backup must be 0600");

        Ok(())
    }

    #[test]
    fn export_is_owner_only() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let nested = dir.path().join("nested");
        let codex = nested.join("codex");
        let out = dir.path().join("backup.json");

        add_record(&codex, "github");

        hermes(&codex)
            .arg("export")
            .arg("-o")
            .arg(&out)
            .assert()
            .success();

        assert_eq!(mode(&out), 0o600, "export must be 0600");

        Ok(())
    }

    #[test]
    fn default_config_dir_is_repaired() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let config = dir.path().join("hermes-otp");
        std::fs::create_dir_all(&config)?;
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o755))?;

        // no --path: the default config dir is resolved from XDG_CONFIG_HOME
        cargo_bin_cmd!("hermes-otp")
            .env("XDG_CONFIG_HOME", dir.path())
            .args(["add", "github", "-c", CODE, "--password", PASSWORD])
            .assert()
            .success();

        assert_eq!(mode(&config), 0o700, "default config dir must be repaired");
        assert_eq!(mode(&config.join("codex")), 0o600);

        Ok(())
    }
}

#[test]
fn import_honors_sha256_8_digits_60s() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("--otpauth")
        .arg(
            "otpauth://totp/Enterprise:alice?secret=JBSWY3DPEHPK3PXP&issuer=Enterprise\
             &algorithm=SHA256&digits=8&period=60",
        )
        .arg("-u")
        .assert()
        .success();

    let output = hermes(path)
        .arg("ls")
        .args(["Enterprise:alice", "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert!(json.is_object(), "single alias must be a JSON object");
    assert_eq!(json["alias"], "Enterprise:alice");

    // digits=8 must be honored, not hardcoded to 6
    let otp = json["otp"].as_str().expect("otp is a string");
    assert_eq!(otp.len(), 8, "expected an 8 digit code, got {otp}");

    // period=60 must be honored, not hardcoded to 30
    let remaining = json["remaining_secs"].as_u64().expect("remaining_secs");
    assert!(
        (1..=60).contains(&remaining),
        "remaining_secs should be within the 60s period, got {remaining}"
    );

    Ok(())
}

#[test]
fn import_rejects_invalid_digits() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("--otpauth")
        .arg("otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&digits=9")
        .arg("-u")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Unsupported OTP parameter"))
        .stderr(predicate::str::contains("digits=9"))
        .stderr(predicate::str::contains("Only 6 to 8 digits"));

    Ok(())
}

#[test]
fn import_rejects_invalid_period() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("--otpauth")
        .arg("otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&period=0")
        .arg("-u")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("period=0"))
        .stderr(predicate::str::contains("between 1 and 300 seconds"));

    Ok(())
}

#[test]
fn import_rejects_invalid_algorithm() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("--otpauth")
        .arg("otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&algorithm=md5")
        .arg("-u")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("algorithm=md5"))
        .stderr(predicate::str::contains(
            "Only sha1, sha256 and sha512 are supported",
        ));

    Ok(())
}

const OTPAUTH_URI: &str = "otpauth://totp/URI:alice?secret=JBSWY3DPEHPK3PXP&issuer=URI\
&algorithm=SHA256&digits=8&period=60";

fn export_json(path: &std::path::Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let out = NamedTempFile::new()?;

    hermes(path)
        .arg("export")
        .arg("-o")
        .arg(out.path())
        .assert()
        .success();

    let content = std::fs::read_to_string(out.path())?;
    Ok(serde_json::from_str(&content)?)
}

#[test]
fn add_defaults_to_sha1_6_30() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["defaults", "-c", CODE, "--password", PASSWORD, "-u"])
        .assert()
        .success();

    let json = export_json(path)?;
    let record = &json.as_array().unwrap()[0];
    assert_eq!(record["algorithm"], "sha1");
    assert_eq!(record["digits"], 6);
    assert_eq!(record["period"], 30);

    let output = hermes(path)
        .arg("ls")
        .args(["defaults", "-u", "--format", "json"])
        .output()?;
    let entry: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(entry["otp"].as_str().unwrap().len(), 6);

    Ok(())
}

#[test]
fn add_accepts_custom_otp_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["custom", "-c", CODE, "--password", PASSWORD, "-u"])
        .args(["--algorithm", "sha256", "--digits", "8", "--period", "60"])
        .assert()
        .success();

    let json = export_json(path)?;
    let record = &json.as_array().unwrap()[0];
    assert_eq!(record["algorithm"], "sha256");
    assert_eq!(record["digits"], 8);
    assert_eq!(record["period"], 60);

    // the code is generated with the custom parameters immediately
    let output = hermes(path)
        .arg("ls")
        .args(["custom", "-u", "--format", "json"])
        .output()?;
    let entry: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(entry["otp"].as_str().unwrap().len(), 8);
    let remaining = entry["remaining_secs"].as_u64().unwrap();
    assert!(
        (1..=60).contains(&remaining),
        "remaining_secs should be within the 60s period, got {remaining}"
    );

    Ok(())
}

#[test]
fn add_rejects_invalid_otp_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["bad", "-c", CODE, "-u", "--digits", "5"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Only 6 to 8 digits"));

    hermes(path)
        .arg("add")
        .args(["bad", "-c", CODE, "-u", "--period", "0"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("between 1 and 300 seconds"));

    // --algorithm is a clap value enum, so it is rejected before our code
    hermes(path)
        .arg("add")
        .args(["bad", "-c", CODE, "-u", "--algorithm", "md5"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("possible values"));

    Ok(())
}

#[test]
fn add_accepts_otpauth_uri_via_code() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("-c")
        .arg(OTPAUTH_URI)
        .arg("-u")
        .assert()
        .success();

    let output = hermes(path)
        .arg("ls")
        .args(["URI:alice", "-u", "--format", "json"])
        .output()?;
    let entry: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(entry["alias"], "URI:alice");
    assert_eq!(entry["otp"].as_str().unwrap().len(), 8);

    Ok(())
}

#[test]
fn add_rejects_mixing_uri_with_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .arg("-c")
        .arg(OTPAUTH_URI)
        .arg("-u")
        .args(["--digits", "8"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("cannot combine"));

    hermes(path)
        .arg("add")
        .args(["myalias", "-c", OTPAUTH_URI, "-u"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("already contains the alias"));

    Ok(())
}

#[test]
fn add_help_documents_otp_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let output = cargo_bin_cmd!("hermes-otp")
        .arg("add")
        .arg("--help")
        .output()?;
    let help = String::from_utf8_lossy(&output.stdout);

    assert!(help.contains("--algorithm"), "{help}");
    assert!(help.contains("--digits"), "{help}");
    assert!(help.contains("--period"), "{help}");
    assert!(help.contains("[default: sha1]"), "{help}");
    assert!(
        help.contains("[possible values: sha1, sha256, sha512]"),
        "{help}"
    );

    Ok(())
}

#[test]
fn update_preserves_otp_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["keep", "-c", CODE, "--password", PASSWORD, "-u"])
        .args(["--algorithm", "sha512", "--digits", "7", "--period", "45"])
        .assert()
        .success();

    hermes(path)
        .arg("update")
        .args(["keep", "-c", "GEZDGNBVGY3TQOJQ", "-u"])
        .assert()
        .success();

    let json = export_json(path)?;
    let record = &json.as_array().unwrap()[0];
    assert_eq!(record["algorithm"], "sha512");
    assert_eq!(record["digits"], 7);
    assert_eq!(record["period"], 45);

    Ok(())
}

/// A v1 record as written by hermes-otp <= 0.7.x. Its magic-crypt payload is
/// no longer decryptable.
fn legacy_record(alias: &str, created_at: u64) -> serde_json::Value {
    serde_json::json!({
        "alias": alias,
        "secret": "3q2+7w==",
        "is_unencrypted": false,
        "algorithm": "sha1",
        "digits": 6,
        "period": 30,
        "created_at": created_at
    })
}

#[test]
fn ls_rejects_legacy_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    let record = legacy_record("legacy", 1);
    std::fs::write(path, format!("{}\n", serde_json::to_string(&record)?))?;
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("ls")
        .args(["legacy", "--password", PASSWORD])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("legacy v1 record(s)"))
        .stderr(predicate::str::contains("0.8.x"));

    assert_eq!(
        std::fs::read_to_string(path)?,
        before,
        "a rejected legacy record must not be rewritten"
    );

    Ok(())
}

#[test]
fn ls_filtered_alias_ignores_unselected_legacy_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["current", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    let mut content = std::fs::read_to_string(path)?;
    content.push_str(&format!(
        "{}\n",
        serde_json::to_string(&legacy_record("legacy", 2))?
    ));
    std::fs::write(path, content)?;

    hermes(path)
        .arg("ls")
        .args(["current", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn update_rejects_legacy_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    let record = legacy_record("legacy", 1);
    std::fs::write(path, format!("{}\n", serde_json::to_string(&record)?))?;
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("update")
        .args(["legacy", "-c", CODE, "--password", PASSWORD])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "legacy v1 format is no longer supported",
        ));

    assert_eq!(
        std::fs::read_to_string(path)?,
        before,
        "a rejected legacy record must not be rewritten"
    );

    Ok(())
}

#[test]
fn tampered_record_reports_authentication_failure() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["secure", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    tamper_first_record(path)?;

    hermes(path)
        .arg("ls")
        .args(["-q", "-f", "json", "--password", PASSWORD])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("authentication failed"));

    Ok(())
}

#[test]
fn passwd_rotates_the_master_password() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["secure", "-c", CODE, "--password", "old-pass"])
        .assert()
        .success();

    let before = std::fs::read_to_string(path)?;
    let record_line = before.lines().last().unwrap().to_string();

    // add() takes its own backup; drop it so we only observe passwd's
    let _ = std::fs::remove_file(path.with_extension("bak"));

    hermes(path)
        .arg("passwd")
        .args(["--password", "old-pass", "--new-password", "new-pass"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Master password updated."));

    // the new password works
    hermes(path)
        .arg("ls")
        .args(["secure", "-q", "--password", "new-pass"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    // the old password no longer does
    hermes(path)
        .arg("ls")
        .args(["secure", "-q", "--password", "old-pass"])
        .assert()
        .failure();

    // only the wrapper changed; the record ciphertext is untouched
    let after = std::fs::read_to_string(path)?;
    assert_eq!(after.lines().last().unwrap(), record_line);

    assert!(
        path.with_extension("bak").exists(),
        "routine backup should exist"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "codex must stay owner-only");
    }

    Ok(())
}

#[test]
fn passwd_rejects_wrong_current_password() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["secure", "-c", CODE, "--password", "old-pass"])
        .assert()
        .success();
    let before = std::fs::read_to_string(path)?;
    let _ = std::fs::remove_file(path.with_extension("bak"));

    hermes(path)
        .arg("passwd")
        .args(["--password", "wrong", "--new-password", "new-pass"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Incorrect password. Nothing was changed.",
        ));

    assert_eq!(
        std::fs::read_to_string(path)?,
        before,
        "a failed rotation must not modify the codex"
    );
    assert!(!path.with_extension("bak").exists());

    Ok(())
}

#[test]
fn passwd_rejects_unencrypted_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["plain", "-c", CODE, "-u"])
        .assert()
        .success();
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("passwd")
        .args(["--password", "a", "--new-password", "b"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "No encrypted records in this vault.",
        ));

    assert_eq!(std::fs::read_to_string(path)?, before);

    Ok(())
}

#[test]
fn passwd_rejects_legacy_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    let record = legacy_record("legacy", 1);
    std::fs::write(path, format!("{}\n", serde_json::to_string(&record)?))?;
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("passwd")
        .args(["--password", "old-pass", "--new-password", "new-pass"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "legacy v1 format is no longer supported",
        ));

    assert_eq!(
        std::fs::read_to_string(path)?,
        before,
        "a rejected legacy record must not be rewritten"
    );

    Ok(())
}

#[test]
fn passwd_rejects_a_positional_alias() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("passwd")
        .args(["secure", "--password", "a", "--new-password", "b"])
        .assert()
        .failure()
        .code(2);

    Ok(())
}

/// Builds a v2 ciphertext with the frozen Argon2id parameters, for migration
/// tests.
fn v2_ciphertext(secret: &str, password: &str) -> String {
    use argon2::{Algorithm, Argon2, Params, Version};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};

    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).unwrap();

    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).unwrap();

    let params = Params::new(19_456, 2, 1, Some(32)).unwrap();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .unwrap();

    let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
    let ciphertext = cipher
        .encrypt(&XNonce::from(nonce), secret.as_bytes())
        .unwrap();

    format!(
        "v2${}${}${}",
        STANDARD.encode(salt),
        STANDARD.encode(nonce),
        STANDARD.encode(ciphertext)
    )
}

fn v2_record(alias: &str, secret: &str, created_at: u64) -> serde_json::Value {
    serde_json::json!({
        "alias": alias,
        "secret": secret,
        "is_unencrypted": false,
        "algorithm": "sha1",
        "digits": 6,
        "period": 30,
        "created_at": created_at
    })
}

#[test]
fn add_writes_a_vault_header_and_v3_records() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    let content = std::fs::read_to_string(path)?;
    let header: serde_json::Value = serde_json::from_str(content.lines().next().unwrap())?;
    assert_eq!(header["version"], 3);
    assert!(header["vault"].as_str().unwrap().starts_with("v3w$"));

    // one key wraps the vault; every record uses its fast cipher
    assert_eq!(content.matches("v3w$").count(), 1);
    for line in content.lines().skip(1) {
        let record: serde_json::Value = serde_json::from_str(line)?;
        assert!(record["secret"].as_str().unwrap().starts_with("v3$"));
    }

    Ok(())
}

#[test]
fn v2_vault_migrates_to_v3() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    let record = v2_record("legacy", &v2_ciphertext(CODE, PASSWORD), 1);
    std::fs::write(path, format!("{}\n", serde_json::to_string(&record)?))?;
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("ls")
        .args(["legacy", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    let after = std::fs::read_to_string(path)?;
    assert!(after.starts_with("{\"version\":3"));
    assert!(after.contains("\"v3$"));

    let backup = path.with_extension("pre-v3.bak");
    assert!(backup.exists(), "pre-v3 backup should exist");
    assert_eq!(
        std::fs::read_to_string(&backup)?,
        before,
        "pre-v3 backup must hold the original vault"
    );

    // a second read is stable
    hermes(path)
        .arg("ls")
        .args(["legacy", "-q", "--password", PASSWORD])
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(path)?, after);

    Ok(())
}

#[test]
fn v2_migration_aborts_on_mixed_passwords() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    let record_a = v2_record("a", &v2_ciphertext(CODE, "pw-a"), 1);
    let record_b = v2_record("b", &v2_ciphertext(CODE, "pw-b"), 2);
    std::fs::write(
        path,
        format!(
            "{}\n{}\n",
            serde_json::to_string(&record_a)?,
            serde_json::to_string(&record_b)?
        ),
    )?;
    let before = std::fs::read_to_string(path)?;

    hermes(path)
        .arg("ls")
        .args(["--password", "pw-a"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("could not decrypt record(s)"))
        .stderr(predicate::str::contains("b"));

    assert_eq!(
        std::fs::read_to_string(path)?,
        before,
        "a failed migration must not modify the codex"
    );
    assert!(!path.with_extension("pre-v3.bak").exists());

    Ok(())
}

#[test]
fn export_import_v3_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let source = NamedTempFile::new()?;
    let destination = NamedTempFile::new()?;
    let export_file = NamedTempFile::new()?;

    hermes(source.path())
        .arg("add")
        .args(["secure", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(source.path())
        .arg("export")
        .arg("-o")
        .arg(export_file.path())
        .assert()
        .success();

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(export_file.path())?)?;
    assert_eq!(json["version"], 3);
    assert!(json["vault"].as_str().unwrap().starts_with("v3w$"));

    hermes(destination.path())
        .arg("import")
        .arg(export_file.path())
        .args(["--password", PASSWORD])
        .assert()
        .success();

    hermes(destination.path())
        .arg("ls")
        .args(["secure", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    // the record was re-encrypted under the destination vault key
    let source_secret = std::fs::read_to_string(source.path())?
        .lines()
        .nth(1)
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .and_then(|record| record["secret"].as_str().map(str::to_string))
        .unwrap();
    let destination_secret = std::fs::read_to_string(destination.path())?
        .lines()
        .nth(1)
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .and_then(|record| record["secret"].as_str().map(str::to_string))
        .unwrap();
    assert_ne!(source_secret, destination_secret);

    Ok(())
}

#[test]
fn import_rejects_wrong_master_password() -> Result<(), Box<dyn std::error::Error>> {
    let source = NamedTempFile::new()?;
    let destination = NamedTempFile::new()?;
    let export_file = NamedTempFile::new()?;

    hermes(source.path())
        .arg("add")
        .args(["secure", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(source.path())
        .arg("export")
        .arg("-o")
        .arg(export_file.path())
        .assert()
        .success();

    hermes(destination.path())
        .arg("import")
        .arg(export_file.path())
        .args(["--password", "wrong-password"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Incorrect password for the imported vault.",
        ));

    Ok(())
}

#[test]
fn plaintext_records_live_alongside_a_vault() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["secure", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    // plaintext records need no master password
    hermes(path)
        .arg("add")
        .args(["plain", "-c", CODE, "-u"])
        .assert()
        .success();

    hermes(path)
        .arg("ls")
        .args(["plain", "-q", "-u"])
        .assert()
        .success();

    hermes(path)
        .arg("ls")
        .args(["secure", "-q", "--password", PASSWORD])
        .assert()
        .success();

    Ok(())
}

#[test]
fn remove_keeps_the_vault_header() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["keep", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["drop", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path).arg("remove").arg("drop").assert().success();

    let content = std::fs::read_to_string(path)?;
    let header: serde_json::Value = serde_json::from_str(content.lines().next().unwrap())?;
    assert_eq!(header["version"], 3);
    assert!(!content.contains("\"drop\""));

    // the vault key survives, so a new record still needs the password
    hermes(path)
        .arg("add")
        .args(["new", "-c", CODE, "--password", "wrong"])
        .assert()
        .failure();

    hermes(path)
        .arg("add")
        .args(["new", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    Ok(())
}

#[test]
fn rename_rejects_a_wrong_password() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["git", "-c", CODE, "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("rename")
        .args(["git", "gh", "--password", "wrong"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Incorrect password"));

    hermes(path)
        .arg("rename")
        .args(["git", "gh", "--password", PASSWORD])
        .assert()
        .success();

    hermes(path)
        .arg("ls")
        .args(["gh", "-q", "--password", PASSWORD])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}
