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

    // 'add' fails without -a and -c
    hermes(path)
        .arg("--path")
        .arg(file.path())
        .arg("add")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains(
            "the following required arguments were not provided",
        ));

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
