//! Tiny file logger (no extra dependencies): `logs/fluss-daemon.log`, rotated
//! at 2 MB to `.1`. Debug builds also print to stderr.

use std::fs::{File, OpenOptions};
use std::io::Write;

use parking_lot::Mutex;

struct FileLog {
    file: Mutex<Option<File>>,
    path: std::path::PathBuf,
}

const MAX_BYTES: u64 = 2 * 1024 * 1024;

impl log::Log for FileLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level() && (m.level() <= log::Level::Info || m.target().starts_with("fluss"))
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} {:5} {}: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            r.level(),
            r.target().trim_start_matches("fluss_daemon::"),
            r.args()
        );
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        let mut g = self.file.lock();
        if let Some(f) = g.as_mut() {
            let _ = f.write_all(line.as_bytes());
            if f.metadata().map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
                *g = None;
                let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
                *g = open(&self.path);
            }
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().as_mut() {
            let _ = f.flush();
        }
    }
}

fn open(path: &std::path::Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

pub fn init() {
    let path = fluss_core::paths::daemon_log();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let logger: &'static FileLog = Box::leak(Box::new(FileLog { file: Mutex::new(open(&path)), path }));
    if log::set_logger(logger).is_ok() {
        let debug = cfg!(debug_assertions) || std::env::var("FLUSS_LOG").is_ok_and(|v| v.eq_ignore_ascii_case("debug"));
        log::set_max_level(if debug { log::LevelFilter::Debug } else { log::LevelFilter::Info });
    }
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));
}
