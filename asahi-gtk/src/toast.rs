use adw::prelude::*;
use libadwaita as adw;

/// Helper: show a success toast.
pub fn success(msg: &str) {
    eprintln!("toast: {}", msg);
    // In GTK4-RS we can't easily get overlay from anywhere; use libnotify
    // as fallback OR rely on status bar.
    // For now: no-op (see below for proper wiring).
    let _ = msg;
}

/// Helper: show an error toast with fallback logging.
/// Returns the error string so callers can choose to also update status.
pub fn error<E: std::fmt::Display>(context: &str, e: E) -> String {
    let msg = format!("{}: {}", context, e);
    eprintln!("toast error: {}", msg);
    msg
}
