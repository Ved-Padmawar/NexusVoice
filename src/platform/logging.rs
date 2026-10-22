//! Structured logging: one JSON object per line in the log dir, plus stdout in
//! dev. The file rolls over at 10 MB, keeping the one before it.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const FILE_NAME: &str = "nexusvoice.log";

/// Targets too chatty at `info` to be worth a line each.
const QUIET_TARGETS: [&str; 5] = ["transcribe_cpp", "wgpu", "naga", "gpui", "cosmic_text"];

struct Logger {
    path: PathBuf,
    file: Mutex<Option<File>>,
    session: String,
}

impl Logger {
    fn open(&self) -> Option<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()
    }

    /// Keep one previous file, as `nexusvoice.log.1`.
    fn rotate_if_full(&self, file: &mut Option<File>) {
        let full = file
            .as_ref()
            .and_then(|f| f.metadata().ok())
            .is_some_and(|m| m.len() >= MAX_FILE_SIZE);
        if full {
            *file = None;
            let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
            *file = self.open();
        }
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let quiet = QUIET_TARGETS
            .iter()
            .any(|target| metadata.target().starts_with(target));
        metadata.level()
            <= if quiet {
                LevelFilter::Warn
            } else {
                LevelFilter::Info
            }
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = serde_json::json!({
            "ts": chrono::Utc::now().to_rfc3339(),
            "level": record.level().as_str(),
            "target": record.target(),
            "session": self.session,
            "msg": record.args().to_string(),
        });
        if cfg!(debug_assertions) {
            println!("{line}");
        }
        let mut file = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.rotate_if_full(&mut file);
        if let Some(file) = file.as_mut() {
            let _ = writeln!(file, "{line}");
        }
    }

    fn flush(&self) {
        if let Some(file) = self
            .file
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            let _ = file.flush();
        }
    }
}

/// A short id for this run, so lines from one session can be grouped when logs
/// from several runs interleave.
fn session_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{:08x}", nanos & 0xffff_ffff)
}

pub fn init() {
    let dir = super::paths::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let logger = Logger {
        path: dir.join(FILE_NAME),
        file: Mutex::new(None),
        session: session_id(),
    };
    *logger
        .file
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = logger.open();
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
