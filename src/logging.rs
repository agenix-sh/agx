use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool) {
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn info(message: &str) {
    if DEBUG_ENABLED.load(Ordering::Relaxed) {
        eprintln!("[agx] {message}");
    }
}
