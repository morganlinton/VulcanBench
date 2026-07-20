#![allow(clippy::module_name_repetitions)]

use crate::algebraic_laws::extract_laws_attribute;
use crate::pass::{boundary_class_tokens, cost_model_family_tokens, pass_phase_tokens, PassArgs};
use quote::quote;
use syn::{DeriveInput, LitStr};

#[test]
fn pass_args_parse_full_metadata_contract() {
    let args = syn::parse2::<PassArgs>(quote! {
        name = "canonical_fold",
        requires = ["domtree", "alias"],
        invalidates = ["cfg"],
        phase = "dataflow",
        boundary_class = "backend_aware",
        requires_caps = ["resident_buffers"],
        preserves_abi = false,
        cost_model_family = "megakernel",
        analyze = "always",
    })
    .expect("Fix: full pass metadata should parse");

    assert_eq!(args.name.value(), "canonical_fold");
    assert_eq!(
        args.requires.iter().map(LitStr::value).collect::<Vec<_>>(),
        vec!["domtree", "alias"]
    );
    assert_eq!(
        args.invalidates
            .iter()
            .map(LitStr::value)
            .collect::<Vec<_>>(),
        vec!["cfg"]
    );
    assert_eq!(
        args.requires_caps
            .iter()
            .map(LitStr::value)
            .collect::<Vec<_>>(),
        vec!["resident_buffers"]
    );
    assert_eq!(
        args.phase.as_ref().map(LitStr::value),
        Some("dataflow".to_string())
    );
    assert_eq!(
        args.boundary_class.as_ref().map(LitStr::value),
        Some("backend_aware".to_string())
    );
    assert_eq!(
        args.cost_model_family.as_ref().map(LitStr::value),
        Some("megakernel".to_string())
    );
    assert_eq!(args.preserves_abi.map(|lit| lit.value), Some(false));
    assert!(args.analyze_always);
}

#[test]
fn pass_args_reject_unknown_argument_with_actionable_fix() {
    let err = syn::parse2::<PassArgs>(quote! {
        name = "bad",
        scheduler = "late",
    })
    .err()
    .expect("Fix: unknown pass argument must fail at macro parse time");

    assert!(err.to_string().contains("unsupported vyre_pass argument"));
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn pass_args_reject_duplicate_top_level_argument() {
    let err = syn::parse2::<PassArgs>(quote! {
        name = "bad",
        requires = [],
        requires = ["late_override"],
        invalidates = [],
    })
    .err()
    .expect("Fix: vyre_pass must reject duplicate top-level arguments");

    assert!(err
        .to_string()
        .contains("duplicate macro argument `requires`"));
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn pass_args_reject_non_string_metadata_arrays() {
    let err = syn::parse2::<PassArgs>(quote! {
        name = "bad",
        requires = [123],
    })
    .err()
    .expect("Fix: metadata arrays must accept only string literals");

    assert!(err.to_string().contains("only string literals"));
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn pass_phase_rejects_consumer_prefixed_phase_names() {
    let phase = LitStr::new("consumer-dataflow", proc_macro2::Span::call_site());
    let err = pass_phase_tokens(Some(&phase))
        .expect_err("Fix: platform pass phases must remain consumer neutral");

    assert!(err.to_string().contains("unsupported pass phase"));
    assert!(err.to_string().contains("Fix:"));
}

#[test]
fn extract_laws_accepts_identifier_and_string_forms() {
    let input = syn::parse2::<DeriveInput>(quote! {
        #[vyre(laws = [Commutative, "Associative"])]
        struct Xor;
    })
    .expect("Fix: derive input should parse");

    let laws = extract_laws_attribute(&input.attrs)
        .expect("Fix: AlgebraicLaws should accept identifier and string law forms");

    assert_eq!(
        laws.iter().map(LitStr::value).collect::<Vec<_>>(),
        vec!["Commutative", "Associative"]
    );
}

#[test]
fn extract_laws_rejects_unknown_vyre_attribute_argument() {
    let input = syn::parse2::<DeriveInput>(quote! {
        #[vyre(rulez = [Commutative])]
        struct Xor;
    })
    .expect("Fix: derive input should parse");

    let err = extract_laws_attribute(&input.attrs)
        .err()
        .expect("Fix: unknown vyre attribute arguments must fail");

    assert!(err.to_string().contains("unknown vyre() argument"));
}

#[test]
fn generated_pass_args_matrix_covers_every_metadata_enum_combination() {
    const PHASES: &[(&str, &str)] = &[
        ("unclassified", "Unclassified"),
        ("canonicalization", "Canonicalization"),
        ("scalar_algebra", "ScalarAlgebra"),
        ("loop", "Loop"),
        ("memory", "Memory"),
        ("fusion_cse", "FusionCse"),
        ("sync", "Sync"),
        ("specialization", "Specialization"),
        ("cleanup", "Cleanup"),
        ("dataflow", "Dataflow"),
        ("megakernel", "Megakernel"),
    ];
    const BOUNDARIES: &[(&str, &str)] = &[
        ("unknown", "Unknown"),
        ("abi_preserving", "AbiPreserving"),
        ("abi_changing", "AbiChanging"),
        ("backend_aware", "BackendAware"),
        ("runtime_aware", "RuntimeAware"),
        ("domain_specific", "DomainSpecific"),
    ];
    const COSTS: &[(&str, &str)] = &[
        ("unknown", "Unknown"),
        ("scalar", "Scalar"),
        ("loop", "Loop"),
        ("memory", "Memory"),
        ("fusion", "Fusion"),
        ("sync", "Sync"),
        ("dataflow", "Dataflow"),
        ("megakernel", "Megakernel"),
    ];

    let mut assertions = 0usize;
    for seed in 0usize..4096 {
        let (phase, phase_variant) = PHASES[seed % PHASES.len()];
        let (boundary, boundary_variant) = BOUNDARIES[(seed / PHASES.len()) % BOUNDARIES.len()];
        let (cost, cost_variant) = COSTS[(seed / (PHASES.len() * BOUNDARIES.len())) % COSTS.len()];
        let analyze = if seed & 1 == 0 {
            quote! { , analyze = "always" }
        } else {
            quote! {}
        };
        let tokens = quote! {
            name = "generated_parse_case",
            requires = ["domtree", "alias"],
            invalidates = ["cfg"],
            phase = #phase,
            boundary_class = #boundary,
            requires_caps = ["cuda", "resident"],
            preserves_abi = false,
            cost_model_family = #cost
            #analyze
        };
        let args = syn::parse2::<PassArgs>(tokens)
            .expect("Fix: generated pass metadata parser case should parse");

        assert_eq!(args.name.value(), "generated_parse_case");
        assert_eq!(args.requires.len(), 2);
        assert_eq!(args.invalidates.len(), 1);
        assert_eq!(args.requires_caps.len(), 2);
        assert_eq!(args.preserves_abi.map(|value| value.value), Some(false));
        assert_eq!(
            args.phase.as_ref().map(LitStr::value).as_deref(),
            Some(phase)
        );
        assert_eq!(
            pass_phase_tokens(args.phase.as_ref())
                .expect("Fix: generated phase must lower")
                .to_string(),
            format!(":: vyre :: optimizer :: PassPhase :: {phase_variant}")
        );
        assert_eq!(
            boundary_class_tokens(args.boundary_class.as_ref())
                .expect("Fix: generated boundary must lower")
                .to_string(),
            format!(":: vyre :: optimizer :: PassBoundaryClass :: {boundary_variant}")
        );
        assert_eq!(
            cost_model_family_tokens(args.cost_model_family.as_ref())
                .expect("Fix: generated cost family must lower")
                .to_string(),
            format!(":: vyre :: optimizer :: CostModelFamily :: {cost_variant}")
        );
        assert_eq!(args.analyze_always, seed & 1 == 0);
        assertions += 10;
    }
    assert_eq!(assertions, 4096 * 10);
}
