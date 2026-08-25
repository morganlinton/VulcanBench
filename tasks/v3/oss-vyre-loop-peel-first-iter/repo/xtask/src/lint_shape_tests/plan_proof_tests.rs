use super::*;

    #[test]
    fn proof_row_backed_only_by_shape_tests_fails() {
        let plan = r#"
| VX-001 | testing_truth | `x.rs` has tests. | `INTERNAL_TEST` | Improvement: tighten proof. | shape tests pass. | Test gate owns proof. |
"#;

        let failures = plan_proof_shape_failures(plan);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("VX-001"));
        assert!(failures[0].contains("shape-style"));
    }

    #[test]
    fn proof_row_with_truth_assertion_marker_passes() {
        let plan = r#"
| VX-001 | testing_truth | `x.rs` has tests. | `INTERNAL_TEST` | Improvement: tighten proof. | shape tests assert exact error bytes and adversarial negative behavior. | Test gate owns proof. |
"#;

        assert!(plan_proof_shape_failures(plan).is_empty());
    }

    #[test]
    fn proof_row_with_generic_asserts_status_only_fails() {
        let plan = r#"
| VX-001 | testing_truth | `x.rs` has tests. | `INTERNAL_TEST` | Improvement: tighten proof. | tests assert status only. | Test gate owns proof. |
"#;

        let failures = plan_proof_shape_failures(plan);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("VX-001"));
        assert!(failures[0].contains("status"));
    }

    #[test]
    fn release_evidence_non_empty_artifact_only_fails() {
        let doc = r#"
# Proof

- CUDA, WGPU, and CPU reference conformance artifacts must exist and be non-empty.
"#;

        let failures = release_evidence_doc_shape_failures("release/evidence/docs/demo.md", doc);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("release/evidence/docs/demo.md:4"));
        assert!(failures[0].contains("non-empty"));
    }

    #[test]
    fn release_evidence_non_empty_with_schema_and_counts_passes() {
        let doc = r#"
# Proof

- CUDA, WGPU, and CPU reference conformance artifacts must exist, be non-empty JSON, use schema_version >= 3, expose backend id/input digest/output digest fields, and report zero failed pairs.
"#;

        assert!(release_evidence_doc_shape_failures("release/evidence/docs/demo.md", doc).is_empty());
    }

    #[test]
    fn proof_row_without_test_claim_is_not_shape_audited() {
        let plan = r#"
| VX-001 | testing_truth | `x.rs` has tests. | `INTERNAL_TEST` | Improvement: tighten proof. | benchmark compares p50 latency. | Bench gate owns proof. |
"#;

        assert!(plan_proof_shape_failures(plan).is_empty());
    }
