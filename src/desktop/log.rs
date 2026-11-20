//! The server's log file (`openLog` in `desktop/src/server.ts`, decision 6): `fleet-server.log` in the app's own data
//! folder, appended to, with the previous one kept as `.1` once it passes a limit.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Fleet's limit: 10 MB.
pub const LIMIT: u64 = 10 * 1024 * 1024;

/// `<app data folder>/logs/fleet-server.log`.
pub fn log_file(app_dir: &Path) -> PathBuf {
    app_dir.join("logs").join("fleet-server.log")
}

/// A log that several threads write to (the server's two output pipes and the supervisor).
#[derive(Clone)]
pub struct Log(Arc<Mutex<File>>);

impl Log {
    /// Opens `file` for appending, first moving it to `<file>.1` if it is over `limit`. Any failure is the caller's to
    /// show; the server still starts without a log.
    pub fn open(file: &Path, limit: u64) -> std::io::Result<Log> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if std::fs::metadata(file).is_ok_and(|m| m.len() > limit) {
            let old = PathBuf::from(format!("{}.1", file.display()));
            // Windows won't rename over an existing file.
            let _ = std::fs::remove_file(&old);
            let _ = std::fs::rename(file, old);
        }
        Ok(Log(Arc::new(Mutex::new(OpenOptions::new().create(true).append(true).open(file)?))))
    }

    /// A log that goes nowhere, for when the file can't be opened.
    pub fn sink() -> Log {
        Log(Arc::new(Mutex::new(sink_file())))
    }

    pub fn write(&self, text: &str) {
        self.write_bytes(text.as_bytes());
    }

    pub fn write_bytes(&self, bytes: &[u8]) {
        if let Ok(mut file) = self.0.lock() {
            let _ = file.write_all(bytes);
        }
    }
}

fn sink_file() -> File {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    OpenOptions::new().write(true).open(null).expect("the null device opens")
}
