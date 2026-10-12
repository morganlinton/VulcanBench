//! Profile-guided backend routing.
//!
//! A cert gate can feed this module a set of candidate backends and op
//! programs. Each `(op, backend)` pair is measured with the same dispatch
//! inputs, then the fastest backend is persisted under `~/.config/vyre/pgo.toml`.

use crate::{BackendError, DispatchConfig, VyreBackend};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use vyre_foundation::ir::Program;

const MAX_PGO_TABLE_BYTES: u64 = 4 * 1024 * 1024;

/// One backend latency observation for a certified operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BackendLatency {
    /// Backend identifier from [`VyreBackend::id`].
    pub backend: String,
    /// Measured latency in nanoseconds.
    pub latency_ns: u128,
}

/// Fastest backend decision for one op.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteDecision {
    /// Backend chosen for runtime dispatch.
    pub backend: String,
    /// All measurements collected by the cert gate.
    pub observations: Vec<BackendLatency>,
}

/// Persisted PGO routing table.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PgoTable {
    /// Map from stable op id to fastest backend and raw observations.
    pub routes: BTreeMap<String, RouteDecision>,
}

impl PgoTable {
    /// Measure every backend for `op_id`, record the fastest, and return it.
    ///
    /// # Errors
    ///
    /// Returns a backend error if no backend is supplied, dispatch fails, or
    /// the measurements cannot be represented.
    pub fn certify_op(
        &mut self,
        op_id: impl Into<String>,
        program: &Program,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
        backends: &[&dyn VyreBackend],
    ) -> Result<&RouteDecision, BackendError> {
        let borrowed = crate::backend::borrowed_input_slices(inputs, "PGO certification inputs")?;
        self.certify_op_borrowed(op_id, program, &borrowed, config, backends)
    }

    /// Borrowed-input variant of [`Self::certify_op`].
    ///
    /// Use this in hot certification loops so large sample buffers are not
    /// copied just to satisfy the legacy owned-input trait method.
    ///
    /// # Errors
    ///
    /// Returns a backend error if no backend is supplied, dispatch fails, or
    /// the measurements cannot be represented.
    pub fn certify_op_borrowed(
        &mut self,
        op_id: impl Into<String>,
        program: &Program,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        backends: &[&dyn VyreBackend],
    ) -> Result<&RouteDecision, BackendError> {
        let op_id = op_id.into();
        if backends.is_empty() {
            return Err(BackendError::new(format!(
                "PGO cert for `{op_id}` received no backends. Fix: pass every available backend to the cert gate."
            )));
        }

        let mut observations = Vec::new();
        observations
            .try_reserve_exact(backends.len())
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: PGO certification could not reserve {} backend latency observation(s): {error}. Certify fewer backends per pass or shard route calibration.",
                    backends.len()
                ),
            })?;
        for backend in backends {
            let elapsed = measure_backend(*backend, program, inputs, config)?;
            observations.push(BackendLatency {
                backend: backend.id().to_string(),
                latency_ns: elapsed.as_nanos(),
            });
        }
        observations.sort_by(|left, right| {
            left.latency_ns
                .cmp(&right.latency_ns)
                .then_with(|| left.backend.cmp(&right.backend))
        });
        let backend = observations[0].backend.clone();
        self.routes.insert(
            op_id.clone(),
            RouteDecision {
                backend,
                observations,
            },
        );
        self.routes.get(&op_id).ok_or_else(|| {
            BackendError::new(format!(
                "PGO route for `{op_id}` was not retained after insertion. Fix: inspect PgoTable map invariants."
            ))
        })
    }

    /// Return the fastest backend known for `op_id`.
    #[must_use]
    pub fn fastest_backend(&self, op_id: &str) -> Option<&str> {
        self.routes
            .get(op_id)
            .map(|decision| decision.backend.as_str())
    }

    /// Load routing decisions from disk.
    ///
    /// # Errors
    ///
    /// Returns a string with `Fix:` guidance when the TOML cannot be read or
    /// decoded.
    pub fn load(path: &Path) -> Result<Self, String> {
        match read_pgo_table_bounded(path) {
            Ok(text) => toml::from_str(&text).map_err(|error| {
                format!(
                    "failed to parse PGO table `{}`: {error}. Fix: regenerate it with the cert gate.",
                    path.display()
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!(
                "failed to read PGO table `{}`: {error}. Fix: ensure ~/.config/vyre is readable.",
                path.display()
            )),
        }
    }

    /// Save routing decisions to disk.
    ///
    /// # Errors
    ///
    /// Returns a string with `Fix:` guidance when the table cannot be encoded
    /// or written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create PGO config directory `{}`: {error}. Fix: ensure ~/.config/vyre is writable.",
                    parent.display()
                )
            })?;
        }
        let encoded = toml::to_string_pretty(self).map_err(|error| {
            format!("failed to encode PGO table: {error}. Fix: report this vyre routing bug.")
        })?;
        fs::write(path, encoded).map_err(|error| {
            format!(
                "failed to write PGO table `{}`: {error}. Fix: ensure ~/.config/vyre is writable.",
                path.display()
            )
        })
    }
}

/// Default PGO table location, XDG-compliant.
#[must_use]
pub fn default_pgo_path() -> PathBuf {
    // XDG Base Directory spec: prefer XDG_CONFIG_HOME, fall back to $HOME/.config.
    let config_base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
            PathBuf::from(home).join(".config")
        });
    config_base.join("vyre").join("pgo.toml")
}

fn read_pgo_table_bounded(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_PGO_TABLE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("PGO table exceeds {MAX_PGO_TABLE_BYTES} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_PGO_TABLE_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_PGO_TABLE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "PGO table exceeded bounded read limit",
        ));
    }
    Ok(text)
}

/// Number of timed iterations per backend after warmup.
const PGO_TIMED_ITERS: usize = 3;

fn measure_backend(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
) -> Result<Duration, BackendError> {
    // Warmup: one cold dispatch to populate driver caches.
    backend.dispatch_borrowed(program, inputs, config)?;
    // Timed: collect the fixed sample set on the stack and return the median.
    let mut samples = [Duration::ZERO; PGO_TIMED_ITERS];
    for sample in &mut samples {
        let start = Instant::now();
        backend.dispatch_borrowed(program, inputs, config)?;
        *sample = start.elapsed();
    }
    samples.sort();
    Ok(samples[samples.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendError, DispatchConfig};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TimedBackend {
        id: &'static str,
        spin: u32,
    }

    impl crate::backend::private::Sealed for TimedBackend {}

    impl VyreBackend for TimedBackend {
        fn id(&self) -> &'static str {
            self.id
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            let mut value = 0u32;
            for _ in 0..self.spin {
                value = value.wrapping_add(1);
            }
            Ok(vec![value.to_le_bytes().to_vec()])
        }
    }

    #[test]
    fn cert_gate_routes_to_fastest_backend() {
        let program = Program::empty();
        let slow = TimedBackend {
            id: "slow",
            spin: 10_000,
        };
        let fast = TimedBackend {
            id: "fast",
            spin: 1,
        };
        let mut table = PgoTable::default();
        let decision = table
            .certify_op(
                "primitive.test.pgo",
                &program,
                &[],
                &DispatchConfig::default(),
                &[&slow, &fast],
            )
            .expect("Fix: PGO certification must measure both backends");
        assert_eq!(decision.backend, "fast");
        assert_eq!(table.fastest_backend("primitive.test.pgo"), Some("fast"));
    }

    struct BorrowCountingBackend {
        borrowed_calls: AtomicUsize,
        owned_calls: AtomicUsize,
    }

    impl crate::backend::private::Sealed for BorrowCountingBackend {}

    impl VyreBackend for BorrowCountingBackend {
        fn id(&self) -> &'static str {
            "borrow-counting"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            self.owned_calls.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }

        fn dispatch_borrowed(
            &self,
            _program: &Program,
            _inputs: &[&[u8]],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            self.borrowed_calls.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
    }

    #[test]
    fn cert_gate_uses_borrowed_dispatch_path() {
        let backend = BorrowCountingBackend {
            borrowed_calls: AtomicUsize::new(0),
            owned_calls: AtomicUsize::new(0),
        };
        let mut table = PgoTable::default();
        let input = [1u8, 2, 3, 4];
        table
            .certify_op_borrowed(
                "primitive.test.borrowed_pgo",
                &Program::empty(),
                &[input.as_slice()],
                &DispatchConfig::default(),
                &[&backend],
            )
            .expect("Fix: borrowed PGO certification must succeed");

        assert_eq!(backend.owned_calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            backend.borrowed_calls.load(Ordering::Relaxed),
            PGO_TIMED_ITERS + 1,
            "Fix: PGO must measure through dispatch_borrowed to avoid input copies"
        );
    }
}
