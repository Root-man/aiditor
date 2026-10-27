use std::sync::{Mutex, MutexGuard};

/// FSEvents startup competes across streams, even when each watches a different directory.
pub fn lock() -> Option<MutexGuard<'static, ()>> {
    static NATIVE: Mutex<()> = Mutex::new(());
    cfg!(target_os = "macos").then(|| NATIVE.lock().unwrap_or_else(|err| err.into_inner()))
}
