//! Crash reports: a panic anywhere in the game writes
//! `<config>/StarfallDynasty/crashes/crash_<unix>.txt` (what panicked, where, the backtrace, the
//! version and the command line) before the default handler runs, and the next start tells the
//! player where it is (login screen) — a player's crash becomes a file they can send, not a vanished
//! window.

use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    crate::state::config_dir().map(|d| d.join("crashes"))
}

/// The report's text (kept apart from the hook so it is tested without crashing).
pub fn report(message: &str, location: &str, backtrace: &str, args: &[String], unix: u64) -> String {
    format!(
        "STARFALL DYNASTY crash report\nversion: {} ({})\ntime (unix): {unix}\nos: {} {}\ncommand line: {}\n\npanic: {message}\nat: {location}\n\nbacktrace:\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        option_env!("STARFALL_COMMIT").unwrap_or("dev build"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        args.join(" ")
    )
}

/// Install the hook (keeps the default one: the message still reaches the console).
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "(no message)".into());
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let text = report(&message, &location, &std::backtrace::Backtrace::force_capture().to_string(), &std::env::args().collect::<Vec<_>>(), unix);
        if let Some(d) = dir() {
            let _ = std::fs::create_dir_all(&d);
            let path = d.join(format!("crash_{unix}.txt"));
            if std::fs::write(&path, text).is_ok() {
                eprintln!("crash report written to {}", path.display());
            }
        }
        default(info);
    }));
}

/// The newest crash report the player has not been told about yet (marks it as told).
pub fn take_unseen() -> Option<PathBuf> {
    let d = dir()?;
    let seen = d.join("seen.txt");
    let told = std::fs::read_to_string(&seen).unwrap_or_default();
    let newest = std::fs::read_dir(&d).ok()?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash_") && n.ends_with(".txt"))).max()?;
    let name = newest.file_name()?.to_string_lossy().into_owned();
    if told.trim() == name {
        return None;
    }
    let _ = std::fs::write(seen, &name);
    Some(newest)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_report_says_what_where_and_how_to_reproduce() {
        let r = super::report("index out of bounds", "crates/client/src/ui.rs:42", "0: main", &["client.exe".into(), "--server".into(), "ws://x".into()], 1_800_000_000);
        for want in ["panic: index out of bounds", "at: crates/client/src/ui.rs:42", "command line: client.exe --server ws://x", "time (unix): 1800000000", "backtrace:\n0: main", "version: "] {
            assert!(r.contains(want), "{want:?} missing in:\n{r}");
        }
    }
}
