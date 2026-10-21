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
        .stdout(predicate::str::contains(&stdout_removed));

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
        .args(["github", "gh"])
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
        .args(["gh", "google"])
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
    assert!(json.is_array());
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);

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

    // wrong password: the code cannot be produced, so this must not exit 0
    hermes(path)
        .arg("ls")
        .args(["github", "-q", "--password", "wrong-password"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("could not produce a code"))
        .stderr(predicate::str::contains("github"));

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

    let output = hermes(path)
        .arg("ls")
        .args(["--password", "wrong-password", "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let entry = &json.as_array().unwrap()[0];

    // the failure is reported in a dedicated field
    assert_eq!(entry["error"], "decryption failed");
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
