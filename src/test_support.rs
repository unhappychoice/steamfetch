#![cfg(test)]

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

// Serialize env-mutating tests across the whole crate. A single shared
// mutex prevents distinct module-local mutexes from racing on the same
// process-wide environment (e.g. XDG_CACHE_HOME). Poisoning is recovered
// because env state is restored by RAII guards on the test side.
pub fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
