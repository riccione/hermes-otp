use crate::models::Record;
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const FILE_CODEX: &str = "codex";
const PROJECT: &str = "hermes-otp";

/// First codex line that wraps the vault key with the master password.
#[derive(Serialize, Deserialize)]
struct VaultHeader {
    version: u8,
    vault: String,
}

/// The codex contents: an optional vault header plus the records.
#[derive(Default)]
pub struct Codex {
    /// Wrapped vault key, present once the file uses the vault format.
    pub vault: Option<String>,
    pub records: Vec<Record>,
}

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

/// Returns the default codex path, or an error when no config directory is
/// available (e.g. a minimal container without HOME).
pub fn get_default_path() -> Result<PathBuf, String> {
    // using dirs fn to get location of config directory
    default_config_dir()
        .map(|mut path| {
            path.push(FILE_CODEX);
            path
        })
        .ok_or_else(|| {
            "Could not determine the config directory; \
             set HERMES_OTP_PATH or pass --path."
                .to_string()
        })
}

pub fn file_exists(path: &Path) -> bool {
    path.exists()
}

/// Exclusive advisory lock protecting the codex for one read-modify-write.
///
/// The lock lives on a sidecar (`<codex>.lock`) because the codex inode is
/// replaced by a rename on every write; locking the codex itself would leave a
/// waiter holding a lock on the unlinked inode while another process locks the
/// new one.
#[must_use = "the lock is released when this guard is dropped"]
pub struct CodexLock {
    file: fs::File,
}

impl Drop for CodexLock {
    fn drop(&mut self) {
        let _ = fs4::FileExt::unlock(&self.file);
    }
}

/// Blocks until this process holds the codex write lock.
pub fn lock_codex(codex_path: &Path) -> Result<CodexLock, String> {
    let lock_path = lock_path(codex_path);

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let file = options
        .open(&lock_path)
        .map_err(|e| format!("could not open lock file '{}': {e}", lock_path.display()))?;

    // UFCS is required: std::fs::File::lock shadows the fs4 trait method on
    // newer toolchains.
    fs4::FileExt::lock(&file).map_err(|e| format!("could not lock the codex: {e}"))?;

    Ok(CodexLock { file })
}

fn lock_path(codex_path: &Path) -> PathBuf {
    let mut name = codex_path.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

pub fn read_codex(path: &Path) -> Result<Codex, String> {
    let content = std::fs::read_to_string(path).map_err(|_| "Codex not found.")?;

    if content.trim().is_empty() {
        return Ok(Codex::default());
    }

    // detect non-JSON format early
    if !content.trim_start().starts_with('{') {
        return Err("Invalid codex format: expected JSON.".to_string());
    }

    let mut codex = Codex::default();
    let mut rest = content.as_str();

    // The vault header, when present, occupies the first line.
    let first_line_end = content.find('\n').unwrap_or(content.len());
    let first_line = content[..first_line_end].trim();

    if let Ok(header) = serde_json::from_str::<VaultHeader>(first_line) {
        if header.version != 3 {
            return Err(format!(
                "Unsupported vault version {}; upgrade hermes-otp.",
                header.version
            ));
        }
        codex.vault = Some(header.vault);
        rest = &content[first_line_end..];
    }

    codex.records = Deserializer::from_str(rest)
        .into_iter::<Record>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Invalid codex record: {e}"))?;

    Ok(codex)
}

/// Writes the header (if any) followed by every record, atomically.
pub fn write_codex_atomic(path: &Path, codex: &Codex) -> Result<(), String> {
    // Test-only: widen the read-to-write window so the concurrency test can
    // prove the lock is held. Release builds ignore it.
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("HERMES_OTP_TEST_WRITE_DELAY_MS")
        && let Ok(ms) = ms.parse::<u64>()
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    let mut lines = Vec::with_capacity(codex.records.len() + 1);

    if let Some(vault) = &codex.vault {
        let header = VaultHeader {
            version: 3,
            vault: vault.clone(),
        };
        lines.push(serde_json::to_string(&header).map_err(|e| e.to_string())?);
    }

    for record in &codex.records {
        lines.push(serde_json::to_string(record).map_err(|e| e.to_string())?);
    }

    overwrite_file_atomic(path, &lines.join("\n"))
        .map_err(|e| format!("Failed to save changes: {e}"))
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

/// Copies the codex to `<name>.pre-v3.bak` once, before the first migration to
/// the vault format.
///
/// The snapshot is only created when it does not exist yet, so repeated or
/// partial migrations never overwrite the original v2 vault.
pub fn backup_pre_v3(path: &Path) -> io::Result<Option<PathBuf>> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No codex file found to back up.",
        ));
    }

    let mut backup_path = path.to_path_buf();
    backup_path.set_extension("pre-v3.bak");

    if backup_path.exists() {
        return Ok(None);
    }

    std::fs::copy(path, &backup_path)?;
    secure_file(&backup_path)?;

    Ok(Some(backup_path))
}

fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_CODEX.to_string());

    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("randomness unavailable"))?;
    let suffix = u64::from_ne_bytes(bytes);

    Ok(path.with_file_name(format!(".{file_name}.{suffix:016x}.tmp")))
}

fn create_temp_file(path: &Path) -> io::Result<(fs::File, PathBuf)> {
    for _ in 0..16 {
        let tmp = temp_path(path)?;

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(&tmp) {
            Ok(file) => return Ok((file, tmp)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique temporary file",
    ))
}

/// Writes `data` atomically: a temp file in the same directory is written,
/// flushed and renamed over `path`, so a crash cannot truncate the codex.
pub fn overwrite_file_atomic(path: &Path, data: &str) -> io::Result<()> {
    let content = format!("{}\n", data.trim());
    let (mut file, tmp) = create_temp_file(path)?;

    let write_result = file
        .write_all(content.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);

    let result = write_result
        .and_then(|()| secure_file(&tmp))
        .and_then(|()| std::fs::rename(&tmp, path))
        .and_then(|()| secure_codex_paths(path));

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{overwrite_file_atomic, temp_path};
    use std::path::Path;

    #[test]
    fn temp_paths_are_random_and_hidden() {
        let path = Path::new("/tmp/codex");
        let first = temp_path(path).unwrap();
        let second = temp_path(path).unwrap();

        assert_ne!(first, second);
        let name = first.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".codex."), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_does_not_follow_planted_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codex");
        let victim = dir.path().join("victim");
        std::fs::write(&victim, "sentinel").unwrap();

        // The old implementation used .<name>.<pid>.tmp.
        let old_tmp = dir
            .path()
            .join(format!(".codex.{}.tmp", std::process::id()));
        symlink(&victim, &old_tmp).unwrap();

        overwrite_file_atomic(&path, "payload").unwrap();

        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "sentinel");
        assert!(path.exists());
    }
}
