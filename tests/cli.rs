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

    let output = hermes(path)
        .arg("ls")
        .args(["aws", "--password", "wrong-password", "--format", "json"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // the failure path keeps the object shape and reports the reason
    assert!(json.is_object(), "single alias must be a JSON object");
    assert_eq!(json["alias"], "aws");
    assert_eq!(json["error"], "decryption failed");

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

    // each record has its own password
    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", "pw-github"])
        .assert()
        .success();

    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", "pw-aws"])
        .assert()
        .success();

    hermes(path)
        .arg("update")
        .args([
            "github",
            "-c",
            "GEZDGNBVGY3TQOJQ",
            "--password",
            "pw-github",
        ])
        .assert()
        .success();

    // the untouched record still opens with its own password
    hermes(path)
        .arg("ls")
        .args(["aws", "-q", "--password", "pw-aws"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

    Ok(())
}

#[test]
fn add_accepts_a_different_password_per_record() -> Result<(), Box<dyn std::error::Error>> {
    let file = NamedTempFile::new()?;
    let path = file.path();

    hermes(path)
        .arg("add")
        .args(["github", "-c", CODE, "--password", "pw-github"])
        .assert()
        .success();

    // a second record with a different password must be accepted: there is no
    // vault-wide password to verify against
    hermes(path)
        .arg("add")
        .args(["aws", "-c", CODE, "--password", "pw-aws"])
        .assert()
        .success();

    hermes(path)
        .arg("ls")
        .args(["aws", "-q", "--password", "pw-aws"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^[0-9]{6}")?);

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
