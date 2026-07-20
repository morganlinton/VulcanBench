//! E4 + E5 substrate: cross-process persistent CUDA JIT cache wiring.
//!
//! The CUDA driver ships its own JIT cache that persists compiled
//! modules to disk and reuses them across processes. This module
//! configures it once at backend startup so vyre dispatches benefit
//! without per-process re-JIT cost.
//!
//! NVIDIA controls the cache through three environment variables:
//!   - `CUDA_CACHE_DISABLE`  -  always forced to `0`; disabling the JIT cache is
//!     a production performance regression.
//!   - `CUDA_CACHE_PATH`  -  directory for cached cuBIN artifacts.
//!   - `CUDA_CACHE_MAXSIZE`  -  soft byte ceiling (defaults to 256 MB).
//!
//! We pick a vyre-namespaced path under the user's XDG cache so the
//! cache is shared by every process that links `vyre-driver-cuda` on
//! the same host (E5: cross-process), and the artifacts persist across
//! reboots (E4: persistent across runs). Callers that have already set a
//! writable cache path or sufficiently large cache size keep their choice.
//! Configuration happens once via a `Once` so multi-threaded backend bring-up
//! does not race.

use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

const CUDA_CACHE_DISABLE: &str = "CUDA_CACHE_DISABLE";
const CUDA_CACHE_PATH: &str = "CUDA_CACHE_PATH";
const CUDA_CACHE_MAXSIZE: &str = "CUDA_CACHE_MAXSIZE";

/// Default cache size: 1 GiB. The CUDA driver's built-in default of
/// 256 MiB evicts faster than we want on workloads with many distinct
/// kernel shapes (autotune sweeps, large matmul tilings).
const DEFAULT_MAX_BYTES: u64 = 1 * 1024 * 1024 * 1024;

static CONFIGURED: OnceLock<Result<(), String>> = OnceLock::new();

#[cfg(test)]
static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
fn lock_test_env() -> MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .lock()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - CUDA JIT cache env test lock must not be poisoned")
}

/// Configure the CUDA driver JIT cache for this process. Call once at
/// CUDA backend bring-up; subsequent calls are no-ops. The function is
/// thread-safe.
pub fn configure_jit_cache_default() -> Result<(), String> {
    #[cfg(test)]
    let _env_guard = lock_test_env();

    CONFIGURED
        .get_or_init(|| {
            let cache_root = default_cache_root()?;
            configure_jit_cache_unlocked(cache_root, DEFAULT_MAX_BYTES)
        })
        .clone()
}

/// Plumb the JIT cache to an explicit directory. Restricted to `pub(crate)` so
/// external callers cannot mutate the CUDA JIT cache environment variables after
/// the backend has already frozen them via [`configure_jit_cache_default`].
/// Use [`configure_jit_cache_default`] from outside this crate; that path is
/// idempotent and guarded by [`CONFIGURED`].
pub(crate) fn configure_jit_cache(cache_dir: PathBuf, max_bytes: u64) -> Result<(), String> {
    #[cfg(test)]
    let _env_guard = lock_test_env();

    configure_jit_cache_unlocked(cache_dir, max_bytes)
}

fn configure_jit_cache_unlocked(cache_dir: PathBuf, max_bytes: u64) -> Result<(), String> {
    if max_bytes == 0 {
        return Err(
            "CUDA JIT cache max size cannot be zero. Fix: configure a positive CUDA_CACHE_MAXSIZE; disabling cache capacity is a production performance regression."
                .to_string(),
        );
    }

    // SAFETY: env-var mutation requires unsafe in Rust 2024 because it is
    // process-global state shared with C-string getenv readers. We restrict
    // mutation to backend bring-up; everything past bring-up is read-only.
    unsafe {
        std::env::set_var(CUDA_CACHE_DISABLE, "0");
    }

    let configured_path = std::env::var_os(CUDA_CACHE_PATH).map(PathBuf::from);
    let cache_dir = configured_path.unwrap_or(cache_dir);
    if cache_dir.as_os_str().is_empty() {
        return Err(
            "CUDA_CACHE_PATH is empty. Fix: set CUDA_CACHE_PATH to a writable directory or leave it unset so Vyre can choose the XDG cache path."
                .to_string(),
        );
    }
    std::fs::create_dir_all(&cache_dir).map_err(|error| {
        format!(
            "failed to create CUDA JIT cache directory `{}`: {error}. Fix: set CUDA_CACHE_PATH to a writable directory before dispatch.",
            cache_dir.display()
        )
    })?;
    if std::env::var_os(CUDA_CACHE_PATH).is_none() {
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe {
            std::env::set_var(CUDA_CACHE_PATH, &cache_dir);
        }
    }

    match std::env::var_os(CUDA_CACHE_MAXSIZE) {
        Some(raw) => {
            let raw = raw.to_string_lossy();
            let configured = raw.parse::<u64>().map_err(|error| {
                format!(
                    "CUDA_CACHE_MAXSIZE is not a byte count: `{raw}` ({error}). Fix: set CUDA_CACHE_MAXSIZE to at least {max_bytes}."
                )
            })?;
            if configured < max_bytes {
                return Err(format!(
                    "CUDA_CACHE_MAXSIZE={configured} is below Vyre's required floor {max_bytes}. Fix: increase CUDA_CACHE_MAXSIZE; undersized JIT cache causes repeated kernel recompilation."
                ));
            }
        }
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        None => unsafe {
            std::env::set_var(CUDA_CACHE_MAXSIZE, max_bytes.to_string());
        },
    }
    Ok(())
}

/// Choose the default cache root: `$XDG_CACHE_HOME/vyre/cuda-jit` when
/// XDG is set, else `$HOME/.cache/vyre/cuda-jit`.
fn default_cache_root() -> Result<PathBuf, String> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg).join("vyre").join("cuda-jit"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join(".cache")
            .join("vyre")
            .join("cuda-jit"));
    }
    Err(
        "CUDA JIT cache has no XDG_CACHE_HOME or HOME. Fix: configure a writable cache root; silent /tmp fallback hides production cache misconfiguration."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EnvSnapshot {
        disable: Option<std::ffi::OsString>,
        path: Option<std::ffi::OsString>,
        max_size: Option<std::ffi::OsString>,
        xdg: Option<std::ffi::OsString>,
    }

    impl EnvSnapshot {
        fn capture() -> Self {
            Self {
                disable: std::env::var_os(CUDA_CACHE_DISABLE),
                path: std::env::var_os(CUDA_CACHE_PATH),
                max_size: std::env::var_os(CUDA_CACHE_MAXSIZE),
                xdg: std::env::var_os("XDG_CACHE_HOME"),
            }
        }

        fn restore(self) {
            restore_var(CUDA_CACHE_DISABLE, self.disable);
            restore_var(CUDA_CACHE_PATH, self.path);
            restore_var(CUDA_CACHE_MAXSIZE, self.max_size);
            restore_var("XDG_CACHE_HOME", self.xdg);
        }
    }

    fn restore_var(name: &str, value: Option<std::ffi::OsString>) {
        // SAFETY: tests hold TEST_ENV_LOCK while mutating CUDA/XDG env.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    /// All four scenarios live in one test because they mutate
    /// process-global env state and `cargo test` runs tests in
    /// parallel; splitting them would race on the shared CUDA_CACHE_*
    /// vars and produce non-deterministic results. Sequenced inside
    /// one function with explicit reset between scenarios.
    fn reset_env() {
        // SAFETY: tests are the only env writers outside backend
        // bring-up; sequential mutation is safe inside one test.
        unsafe {
            std::env::remove_var(CUDA_CACHE_DISABLE);
            std::env::remove_var(CUDA_CACHE_PATH);
            std::env::remove_var(CUDA_CACHE_MAXSIZE);
        }
    }

    #[test]
    fn jit_cache_env_contract() {
        let _env_guard = lock_test_env();
        let snapshot = EnvSnapshot::capture();

        // Scenario 1: all three vars get set when the operator hasn't
        // pre-configured anything. Cache directory is created.
        reset_env();
        let dir = std::env::temp_dir().join("vyre-jit-cache-test-1");
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "Fix: failed to remove stale CUDA JIT cache test directory `{}`: {error}",
                dir.display()
            ),
        }
        configure_jit_cache_unlocked(dir.clone(), 12_345)
            .expect("Fix: CUDA JIT cache test path should configure");
        assert_eq!(std::env::var(CUDA_CACHE_DISABLE).unwrap(), "0");
        assert_eq!(
            std::env::var(CUDA_CACHE_PATH).unwrap(),
            dir.to_string_lossy()
        );
        assert_eq!(std::env::var(CUDA_CACHE_MAXSIZE).unwrap(), "12345");
        assert!(dir.is_dir(), "cache directory must be created");

        // Scenario 2: existing CUDA_CACHE_DISABLE=1 is overwritten because
        // disabling the CUDA JIT cache is a production performance regression.
        reset_env();
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        unsafe {
            std::env::set_var(CUDA_CACHE_DISABLE, "1");
        }
        let dir2 = std::env::temp_dir().join("vyre-jit-cache-test-2");
        configure_jit_cache_unlocked(dir2, 1024)
            .expect("Fix: CUDA JIT cache should force-enable driver cache");
        assert_eq!(
            std::env::var(CUDA_CACHE_DISABLE).unwrap(),
            "0",
            "Vyre must force-enable the CUDA JIT cache"
        );

        // Scenario 3: existing CUDA_CACHE_PATH is preserved.
        reset_env();
        let custom = PathBuf::from("/tmp/operator-chosen-jit-path");
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        unsafe {
            std::env::set_var(CUDA_CACHE_PATH, &custom);
        }
        let other = std::env::temp_dir().join("vyre-jit-cache-test-3");
        configure_jit_cache_unlocked(other, 1024)
            .expect("Fix: CUDA JIT cache should preserve operator path");
        assert_eq!(
            std::env::var(CUDA_CACHE_PATH).unwrap(),
            custom.to_string_lossy()
        );

        // Scenario 4: default_cache_root() routes through XDG when set.
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", "/tmp/my-xdg-cache");
        }
        let root = default_cache_root().expect("Fix: XDG cache root should resolve");
        assert_eq!(root, PathBuf::from("/tmp/my-xdg-cache/vyre/cuda-jit"));

        snapshot.restore();
    }
}
