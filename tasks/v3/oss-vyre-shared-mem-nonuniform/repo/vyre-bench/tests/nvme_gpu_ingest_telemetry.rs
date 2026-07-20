//! Tests for release-scale NVMe to GPU ingest telemetry benchmark surfaces.

use std::collections::BTreeSet;

use vyre_runtime::uring::NativeReadPath;

#[test]
fn nvme_gpu_ingest_specs_are_release_scale_and_gpu_resident() {
    let specs = vyre_bench::cases::nvme_gpu_ingest::nvme_gpu_ingest_specs();
    assert!(
        specs.len() >= 2,
        "Fix: benchmark coverage must include both registered mapped reads and native GPUDirect NVMe."
    );

    for spec in specs {
        let total_bytes = spec.total_bytes().unwrap_or_else(|| {
            panic!(
                "Fix: spec `{}` total byte count must not overflow.",
                spec.id
            )
        });
        let resident_bytes = spec.resident_bytes().unwrap_or_else(|| {
            panic!(
                "Fix: spec `{}` resident byte count must not overflow.",
                spec.id
            )
        });
        assert!(
            total_bytes >= 4 * 1024 * 1024 * 1024,
            "Fix: spec `{}` must represent release-scale multi-GiB ingest.",
            spec.id
        );
        assert!(
            resident_bytes <= 2 * 1024 * 1024 * 1024,
            "Fix: spec `{}` staging footprint must fit the RTX 5090 release host with margin.",
            spec.id
        );
        assert!(
            spec.total_reads()
                .is_some_and(|reads| reads >= spec.slot_count),
            "Fix: spec `{}` must cover at least one full ring of submissions.",
            spec.id
        );
    }
}

#[test]
fn nvme_gpu_ingest_registry_contains_release_zero_copy_cases() {
    let registry = vyre_bench::registry::collect_all();
    let ids = registry
        .iter()
        .map(|case| case.id().0)
        .collect::<BTreeSet<_>>();

    for spec in vyre_bench::cases::nvme_gpu_ingest::nvme_gpu_ingest_specs() {
        assert!(
            ids.contains(spec.id),
            "Fix: ingest benchmark spec `{}` must be registered.",
            spec.id
        );
        let case = registry
            .get(&vyre_bench::api::case::BenchId(spec.id.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "Fix: ingest benchmark spec `{}` must be retrievable.",
                    spec.id
                )
            });
        assert!(
            case.active_in_suite(vyre_bench::api::suite::SuiteKind::Release),
            "Fix: ingest benchmark spec `{}` must be part of the release suite.",
            spec.id
        );
        let metadata = case.metadata();
        assert!(
            metadata.tags.iter().any(|tag| tag == "zero-copy"),
            "Fix: ingest benchmark spec `{}` must be discoverable as zero-copy.",
            spec.id
        );
        assert!(
            case.requirements().needs_gpu,
            "Fix: ingest benchmark spec `{}` must be GPU-required.",
            spec.id
        );
    }
}

#[test]
fn nvme_gpu_ingest_metric_points_preserve_zero_cpu_bounce() {
    for spec in vyre_bench::cases::nvme_gpu_ingest::nvme_gpu_ingest_specs() {
        let telemetry =
            vyre_bench::cases::nvme_gpu_ingest::synthesize_completed_ingest_telemetry(*spec)
                .unwrap_or_else(|error| {
                    panic!("Fix: ingest spec `{}` must synthesize: {error}", spec.id)
                });
        vyre_bench::cases::nvme_gpu_ingest::validate_zero_copy_ingest_telemetry(*spec, telemetry)
            .unwrap_or_else(|error| {
                panic!("Fix: ingest spec `{}` must validate: {error}", spec.id)
            });

        let points =
            vyre_bench::cases::nvme_gpu_ingest::ingest_telemetry_metric_points(*spec, telemetry);
        let cpu_bounce = points
            .iter()
            .find(|point| point.name == "cpu_bounce_bytes")
            .expect("Fix: ingest metrics must expose CPU bounce bytes.");
        assert_eq!(
            cpu_bounce.value, 0,
            "Fix: ingest metrics must never hide a CPU bounce copy."
        );
        assert!(
            points
                .iter()
                .any(|point| point.name == "inflight_reads" && point.value == 0),
            "Fix: completed ingest metrics must expose zero inflight reads."
        );
    }
}

#[test]
fn nvme_gpu_ingest_validation_rejects_bounce_and_path_mixing() {
    for spec in vyre_bench::cases::nvme_gpu_ingest::nvme_gpu_ingest_specs() {
        let mut bounced =
            vyre_bench::cases::nvme_gpu_ingest::synthesize_completed_ingest_telemetry(*spec)
                .unwrap_or_else(|error| {
                    panic!("Fix: ingest spec `{}` must synthesize: {error}", spec.id)
                });
        bounced.cpu_bounce_bytes = 1;
        assert!(
            matches!(
                vyre_bench::cases::nvme_gpu_ingest::validate_zero_copy_ingest_telemetry(
                    *spec, bounced
                ),
                Err(_)
            ),
            "Fix: ingest validation must reject CPU bounce bytes for `{}`.",
            spec.id
        );

        let mut mixed =
            vyre_bench::cases::nvme_gpu_ingest::synthesize_completed_ingest_telemetry(*spec)
                .unwrap_or_else(|error| {
                    panic!("Fix: ingest spec `{}` must synthesize: {error}", spec.id)
                });
        match spec.path {
            NativeReadPath::RegisteredMappedRead => {
                mixed.gpudirect_nvme_submissions = 1;
            }
            NativeReadPath::GpuDirectNvmePassthrough => {
                mixed.registered_mapped_read_submissions = 1;
            }
        }
        assert!(
            matches!(
                vyre_bench::cases::nvme_gpu_ingest::validate_zero_copy_ingest_telemetry(
                    *spec, mixed
                ),
                Err(_)
            ),
            "Fix: ingest validation must reject path mixing for `{}`.",
            spec.id
        );
    }
}

#[test]
fn release_criterion_entrypoint_includes_nvme_gpu_ingest_projection() {
    let source = include_str!("../benches/release.rs");
    assert!(
        source.contains("nvme_gpu_ingest_telemetry_projection_scale"),
        "Fix: release Criterion entrypoint must benchmark NVMe to GPU ingest telemetry projection."
    );
    assert!(
        source.contains("runtime_io/nvme_gpu_ingest_telemetry"),
        "Fix: benchmark group must be named as runtime IO ingest coverage."
    );
}
