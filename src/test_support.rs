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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_env_recovers_from_poisoned_mutex() {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(|| {
            let _guard = ENV_LOCK.lock().unwrap();
            panic!("poison ENV_LOCK");
        });
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        let _guard = lock_env();
    }
}
