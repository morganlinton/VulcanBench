//! Integration test crate for the containing Vyre package.

use super::*;
use crate::backend::CompiledPipeline;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

mod compiled_pipeline_defaults;

/// Minimal backend that records how many times `dispatch` was called.
/// Used to verify the passthrough pipeline routes every dispatch back
/// through the backend (no inadvertent caching at the framework layer).
#[derive(Default)]
struct CountingBackend {
    calls: std::sync::Mutex<usize>,
}

impl crate::backend::private::Sealed for CountingBackend {}

impl VyreBackend for CountingBackend {
    fn id(&self) -> &'static str {
        "counting"
    }

    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        *self.calls.lock().unwrap() += 1;
        // Echo: each output buffer mirrors the input at the same index.
        Ok(inputs.to_vec())
    }
}

fn empty_program() -> Program {
    // The framework treats Program opaquely for the passthrough path  -
    // we never need to lower or execute. A minimal default value is
    // sufficient to exercise the trait surface.
    Program::default()
}

#[test]
fn passthrough_routes_every_dispatch_to_backend() {
    let backend = Arc::new(CountingBackend::default());
    let pipeline = compile(
        backend.clone(),
        &empty_program(),
        &DispatchConfig::default(),
    )
    .unwrap();
    let inputs = vec![vec![1u8, 2, 3]];
    for _ in 0..10 {
        let out = pipeline
            .dispatch(&inputs, &DispatchConfig::default())
            .unwrap();
        assert_eq!(out, inputs);
    }
    assert_eq!(*backend.calls.lock().unwrap(), 10);
}

#[test]
fn compile_owned_routes_without_borrowed_program_clone() {
    let backend = Arc::new(CountingBackend::default());
    let pipeline =
        compile_owned(backend.clone(), empty_program(), &DispatchConfig::default()).unwrap();
    let inputs = vec![vec![4_u8, 5, 6]];
    let out = pipeline
        .dispatch(&inputs, &DispatchConfig::default())
        .unwrap();
    assert_eq!(out, inputs);
    assert_eq!(*backend.calls.lock().unwrap(), 1);
}

#[test]
fn compile_owned_with_telemetry_returns_pipeline() {
    let backend = Arc::new(CountingBackend::default());
    let build =
        compile_owned_with_telemetry(backend, empty_program(), &DispatchConfig::default()).unwrap();
    assert!(build.pipeline.id().starts_with("counting:"));
    assert_eq!(build.manifest.backend_id, "counting");
    assert_eq!(build.manifest.pipeline_id, build.pipeline.id());
    assert_eq!(build.manifest.schema, PipelineReproManifest::SCHEMA);
    let json = build
        .manifest
        .to_json()
        .expect("manifest JSON must serialize");
    assert!(json.contains("\"program_digest\""));
}

#[test]
fn pipeline_cache_audit_tracks_hits_misses_and_unknowns() {
    let mut audit = PipelineCacheAudit::new();
    audit.observe(Some(true));
    audit.observe(Some(true));
    audit.observe(Some(false));
    audit.observe(None);

    let report = audit.snapshot(7_000);

    assert_eq!(report.hits, 2);
    assert_eq!(report.misses, 1);
    assert_eq!(report.unknowns, 1);
    assert_eq!(report.hit_rate_bps, Some(6_666));
    assert!(report.below_alarm_threshold);
}

#[test]
fn pipeline_cache_audit_no_data_has_no_alarm() {
    let audit = PipelineCacheAudit::new();
    let report = audit.snapshot(9_000);

    assert_eq!(report.hit_rate_bps, None);
    assert!(!report.below_alarm_threshold);
}

#[test]
fn pipeline_cache_audit_zero_threshold_disables_alarm() {
    let mut audit = PipelineCacheAudit::new();
    audit.observe(Some(false));

    let report = audit.snapshot(0);

    assert_eq!(report.hit_rate_bps, Some(0));
    assert!(!report.below_alarm_threshold);
}

#[test]
fn prewarm_materializes_pipeline_without_dispatching() {
    let backend = Arc::new(CountingBackend::default());
    let report = prewarm_owned(backend.clone(), empty_program(), &DispatchConfig::default())
        .expect("prewarm must compile through the same path as pipeline mode");
    assert!(report.pipeline_id.starts_with("counting:"));
    assert_eq!(report.manifest.pipeline_id, report.pipeline_id);
    assert_eq!(
        *backend.calls.lock().unwrap(),
        0,
        "Fix: prewarm must remove compile/reflection from the hot path without running the program."
    );
}

#[test]
fn passthrough_id_includes_backend_id() {
    let backend = Arc::new(CountingBackend::default());
    let pipeline = compile(backend, &empty_program(), &DispatchConfig::default()).unwrap();
    assert!(pipeline.id().starts_with("counting:"));
}

#[test]
fn passthrough_dispatch_borrowed_uses_backend_borrowed_override() {
    #[derive(Default)]
    struct BorrowRecordingBackend {
        owned_calls: std::sync::Mutex<usize>,
        borrowed_calls: std::sync::Mutex<usize>,
    }

    impl crate::backend::private::Sealed for BorrowRecordingBackend {}

    impl VyreBackend for BorrowRecordingBackend {
        fn id(&self) -> &'static str {
            "borrow-recording"
        }

        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            *self.owned_calls.lock().unwrap() += 1;
            Ok(inputs.to_vec())
        }

        fn dispatch_borrowed(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            *self.borrowed_calls.lock().unwrap() += 1;
            Ok(inputs.iter().map(|input| (*input).to_vec()).collect())
        }
    }

    let backend = Arc::new(BorrowRecordingBackend::default());
    let pipeline = compile(
        backend.clone(),
        &empty_program(),
        &DispatchConfig::default(),
    )
    .unwrap();
    let input = [7u8, 8, 9];

    let out = pipeline
        .dispatch_borrowed(&[input.as_slice()], &DispatchConfig::default())
        .unwrap();

    assert_eq!(out, vec![input.to_vec()]);
    assert_eq!(*backend.borrowed_calls.lock().unwrap(), 1);
    assert_eq!(*backend.owned_calls.lock().unwrap(), 0);
}

#[test]
fn per_call_config_overrides_compile_config() {
    // Backend that records the profile string it observed on dispatch.
    struct ProfileEcho {
        seen: std::sync::Mutex<Vec<Option<String>>>,
    }
    impl crate::backend::private::Sealed for ProfileEcho {}
    impl VyreBackend for ProfileEcho {
        fn id(&self) -> &'static str {
            "profile-echo"
        }
        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            self.seen.lock().unwrap().push(config.profile.clone());
            Ok(vec![])
        }
    }
    let backend = Arc::new(ProfileEcho {
        seen: Default::default(),
    });
    let compile_cfg = DispatchConfig {
        profile: Some("compile-time".to_string()),
        ulp_budget: None,
        ..DispatchConfig::default()
    };
    let pipeline = compile(backend.clone(), &empty_program(), &compile_cfg).unwrap();

    // Default per-call config falls back to compile-time profile.
    pipeline.dispatch(&[], &DispatchConfig::default()).unwrap();
    // Non-default per-call config overrides.
    pipeline
        .dispatch(
            &[],
            &DispatchConfig {
                profile: Some("per-call".to_string()),
                ulp_budget: None,
                ..DispatchConfig::default()
            },
        )
        .unwrap();

    let seen = backend.seen.lock().unwrap();
    assert_eq!(seen[0], Some("compile-time".to_string()));
    assert_eq!(seen[1], Some("per-call".to_string()));
}

#[test]

fn native_pipeline_is_used_when_backend_provides_one() {
    // Backend that returns a NoopPipeline from compile_native; verifies
    // the framework hands it back directly instead of wrapping in
    // passthrough.
    struct NativePipeline;
    impl crate::backend::private::Sealed for NativePipeline {}
    impl CompiledPipeline for NativePipeline {
        fn id(&self) -> &str {
            "native-pipeline"
        }
        fn dispatch(
            &self,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(vec![vec![42]])
        }
    }
    struct NativeBackend;
    impl crate::backend::private::Sealed for NativeBackend {}
    impl VyreBackend for NativeBackend {
        fn id(&self) -> &'static str {
            "native"
        }
        fn dispatch(
            &self,
            _: &Program,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "native backend should be reached via compile, not dispatch. \
                 Fix: use vyre::pipeline::compile then call CompiledPipeline::dispatch.",
            ))
        }
        fn compile_native(
            &self,
            _: &Program,
            _: &DispatchConfig,
        ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
            Ok(Some(Arc::new(NativePipeline)))
        }
    }
    let backend = Arc::new(NativeBackend);
    let pipeline = compile(backend, &empty_program(), &DispatchConfig::default()).unwrap();
    assert_eq!(pipeline.id(), "native-pipeline");
    let outputs = pipeline.dispatch(&[], &DispatchConfig::default()).unwrap();
    assert_eq!(outputs, vec![vec![42]]);
}

#[test]
fn prewarm_reports_backend_cache_telemetry() {
    struct WarmPipeline;
    impl crate::backend::private::Sealed for WarmPipeline {}
    impl CompiledPipeline for WarmPipeline {
        fn id(&self) -> &str {
            "warm-native"
        }
        fn dispatch(
            &self,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct WarmBackend {
        compiles: std::sync::Mutex<u64>,
        hits: std::sync::Mutex<u64>,
        misses: std::sync::Mutex<u64>,
    }

    impl crate::backend::private::Sealed for WarmBackend {}

    impl VyreBackend for WarmBackend {
        fn id(&self) -> &'static str {
            "warm"
        }

        fn dispatch(
            &self,
            _: &Program,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "prewarm test backend should never dispatch. Fix: keep prewarm on the compile path.",
            ))
        }

        fn compile_native(
            &self,
            _: &Program,
            _: &DispatchConfig,
        ) -> Result<Option<Arc<dyn CompiledPipeline>>, BackendError> {
            let mut compiles = self.compiles.lock().unwrap();
            if *compiles == 0 {
                *self.misses.lock().unwrap() += 1;
            } else {
                *self.hits.lock().unwrap() += 1;
            }
            *compiles += 1;
            Ok(Some(Arc::new(WarmPipeline)))
        }

        fn pipeline_cache_snapshot(&self) -> Option<PipelineCacheSnapshot> {
            Some(PipelineCacheSnapshot {
                hits: *self.hits.lock().unwrap(),
                misses: *self.misses.lock().unwrap(),
            })
        }
    }

    let backend = Arc::new(WarmBackend::default());
    let cold = prewarm(
        backend.clone(),
        &empty_program(),
        &DispatchConfig::default(),
    )
    .expect("cold prewarm should compile");
    let hot = prewarm(backend, &empty_program(), &DispatchConfig::default())
        .expect("hot prewarm should hit cache telemetry");

    assert_eq!(cold.pipeline_id, "warm-native");
    assert_eq!(cold.cache_hit, Some(false));
    assert_eq!(hot.cache_hit, Some(true));
}

#[test]
#[allow(deprecated)]
fn compile_rejects_non_region_programs() {
    let backend = Arc::new(CountingBackend::default());
    let program = Program::new(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(9)), Node::Return],
    );
    let error = match compile(backend, &program, &DispatchConfig::default()) {
        Ok(_) => panic!("Fix: runtime admission must reject raw top-level statements"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("top-level Region-wrapped Program"),
        "Fix: runtime admission rejection must mention the region invariant, got: {error}"
    );
}
