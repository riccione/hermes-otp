use crate::models::Record;
use serde_json::Deserializer;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const FILE_CODEX: &str = "codex";
const PROJECT: &str = "hermes-otp";

/// Directory this app owns for its own data, e.g. `~/.config/hermes-otp`.
fn default_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|mut path| {
        path.push(PROJECT);
        path
    })
}

/// True when `dir` is the directory this app owns for its own data.
fn is_default_config_dir(dir: &Path) -> bool {
    default_config_dir().map(|d| d == dir).unwrap_or(false)
}

/// Restricts `path` to owner read/write (0600). No-op on non-Unix platforms.
pub fn secure_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if path.exists() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

/// Restricts `path` to owner read/write/execute (0700). No-op on non-Unix
/// platforms.
pub fn secure_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if path.exists() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

/// Secures a codex file, and its directory when that directory belongs to this
/// app. A user-supplied parent (e.g. `/tmp`, `$HOME`) is never re-permissioned.
fn secure_codex_paths(path: &Path) -> io::Result<()> {
    secure_file(path)?;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && is_default_config_dir(parent)
    {
        secure_dir(parent)?;
    }

    Ok(())
}

pub fn get_default_path() -> PathBuf {
    // using dirs fn to get location of config directory
    default_config_dir()
        .map(|mut path| {
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
    let mut options = OpenOptions::new();
    options.append(true).create(true);

    // create the file owner-only from the start, so it is never briefly
    // world-readable
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut data_file = options.open(path)?;
    writeln!(data_file, "{}", data.trim())?;
    drop(data_file);

    secure_codex_paths(path)
}

pub fn overwrite_file(path: &Path, data: &str) -> io::Result<()> {
    let content = format!("{}\n", data.trim());

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut data_file = options.open(path)?;
    data_file.write_all(content.as_bytes())?;
    drop(data_file);

    secure_codex_paths(path)
}

pub fn alias_exists(alias: &str, path: &Path) -> bool {
    read_codex(path)
        .map(|records| records.iter().any(|r| r.alias == alias))
        .unwrap_or(false)
}

pub fn ensure_dir_exists(path: &Path) -> io::Result<()> {
    // only attempt to create directories if there is a parent component
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    // if the path is just "test.codex", parent() might be Some("") or empty
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    let existed = parent.exists();

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);

    // new directories are created owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }

    builder.create(parent)?;

    // Tighten a directory we just created, or the directory this app owns.
    // Never chmod an arbitrary pre-existing parent: it may be /tmp or $HOME.
    if !existed || is_default_config_dir(parent) {
        secure_dir(parent)?;
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

    // std::fs::copy copies the source's mode bits, so a codex that was
    // world-readable would otherwise produce a world-readable backup
    std::fs::copy(path, &backup_path)?;
    secure_file(&backup_path)?;

    Ok(backup_path)
}

// routine backups for add and remove cmd
pub fn create_routine_backup(path: &Path) -> io::Result<PathBuf> {
    perform_backup(path, "bak")
}

/// Writes `data` atomically: a temp file in the same directory is written,
/// flushed and renamed over `path`, so a crash cannot truncate the codex.
pub fn overwrite_file_atomic(path: &Path, data: &str) -> io::Result<()> {
    let content = format!("{}\n", data.trim());

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_CODEX.to_string());
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));

    let result = (|| -> io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut file = options.open(&tmp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);

        secure_file(&tmp)?;
        std::fs::rename(&tmp, path)?;
        secure_codex_paths(path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }

    result
}

/// Copies the codex to `<name>.pre-v2.bak` before the first v2 migration.
///
/// The snapshot is only created when it does not exist yet, so repeated or
/// partial migrations never overwrite the original v1 vault.
pub fn backup_pre_v2(path: &Path) -> io::Result<Option<PathBuf>> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Error: No Codex file found to backup.",
        ));
    }

    let mut backup_path = path.to_path_buf();
    backup_path.set_extension("pre-v2.bak");

    if backup_path.exists() {
        return Ok(None);
    }

    std::fs::copy(path, &backup_path)?;
    secure_file(&backup_path)?;

    Ok(Some(backup_path))
}
