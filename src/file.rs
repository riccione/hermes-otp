use crate::models::Record;
use serde_json::Deserializer;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const FILE_CODEX: &str = "codex";
const PROJECT: &str = "hermes-otp";

pub fn get_default_path() -> PathBuf {
    // using dirs fn to get location of config directory
    dirs::config_dir()
        .map(|mut path| {
            path.push(PROJECT);
            path.push(FILE_CODEX);
            path
        })
        .expect("Error: Failed to get config path")
}

pub fn file_exists(path: &Path) -> bool {
    path.exists()
}

pub fn read_codex(path: &Path) -> Result<Vec<Record>, String> {
    let content = std::fs::read_to_string(path).map_err(|_| "Codex not found.")?;

    // detect non-JSON format early
    if !content.trim_start().starts_with('{') && !content.trim().is_empty() {
        return Err("Invalid codex format: expected JSON.".to_string());
    }

    if content.is_empty() {
        return Ok(Vec::new());
    }

    // JSON parsing
    let records: Vec<Record> = Deserializer::from_str(&content)
        .into_iter::<Record>()
        .filter_map(|r| r.ok())
        .collect();

    Ok(records)
}

pub fn append_to_file(path: &Path, data: &str) -> io::Result<()> {
    let mut data_file = OpenOptions::new().append(true).create(true).open(path)?;
    writeln!(data_file, "{}", data.trim())
}

pub fn overwrite_file(path: &Path, data: &str) -> io::Result<()> {
    let content = format!("{}\n", data.trim());
    std::fs::write(path, content)
}

pub fn alias_exists(alias: &str, path: &Path) -> bool {
    read_codex(path)
        .map(|records| records.iter().any(|r| r.alias == alias))
        .unwrap_or(false)
}

pub fn ensure_dir_exists(path: &Path) -> io::Result<()> {
    // only attempt to create directories if there is a parent component
    if let Some(parent) = path.parent() {
        // if the path is just "test.codex", parent() might be Some("") or empty
        // call create_dir_all if the parent isn't empty
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn perform_backup(path: &Path, extension: &str) -> io::Result<PathBuf> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Error: No Codex file found to backup.",
        ));
    }

    let mut backup_path = path.to_path_buf();
    backup_path.set_extension(extension);

    std::fs::copy(path, &backup_path)?;
    Ok(backup_path)
}

// routine backups for add and remove cmd
pub fn create_routine_backup(path: &Path) -> io::Result<PathBuf> {
    perform_backup(path, "bak")
}
