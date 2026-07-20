//! Surface and generated-matrix tests for Category C intrinsic descriptors.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

use smallvec::smallvec;
use vyre_spec::{
    Backend, BackendId, CapabilityId, CostHint, DeterminismClass, IntrinsicDescriptor,
    IntrinsicLowering, IntrinsicTable, OperationContract, SideEffectClass,
};

fn append_input_len_and_xor(input: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(input.len() as u32).to_le_bytes());
    output.push(input.iter().fold(0u8, |acc, byte| acc ^ byte));
}

fn append_reversed(input: &[u8], output: &mut Vec<u8>) {
    output.extend(input.iter().rev().copied());
}

#[test]
fn intrinsic_descriptor_new_binds_name_hardware_and_cpu_reference() {
    let descriptor =
        IntrinsicDescriptor::new("warp_popcount", "cuda.warp.vote", append_input_len_and_xor);

    let mut output = Vec::new();
    (descriptor.cpu_fn())(&[0xAA, 0xCC, 0x55], &mut output);

    assert_eq!(descriptor.name(), "warp_popcount");
    assert_eq!(descriptor.hardware(), "cuda.warp.vote");
    assert_eq!(descriptor.contract(), None);
    assert_eq!(output, [3, 0, 0, 0, 0x33]);
}

#[test]
fn intrinsic_descriptor_with_contract_preserves_capability_metadata() {
    let contract = OperationContract {
        capability_requirements: Some(smallvec![
            CapabilityId::new("cuda.sm_90"),
            CapabilityId::new("warp.vote"),
        ]),
        determinism: Some(DeterminismClass::Deterministic),
        side_effect: Some(SideEffectClass::Pure),
        cost_hint: Some(CostHint::Cheap),
    };

    let descriptor = IntrinsicDescriptor::with_contract(
        "warp_reverse",
        "cuda.warp.shuffle",
        append_reversed,
        contract,
    );

    let mut output = Vec::new();
    (descriptor.cpu_fn())(&[1, 2, 3, 4], &mut output);
    let bound_contract = descriptor
        .contract()
        .expect("Fix: descriptor contract must survive construction.");

    assert_eq!(descriptor.name(), "warp_reverse");
    assert_eq!(descriptor.hardware(), "cuda.warp.shuffle");
    assert_eq!(output, [4, 3, 2, 1]);
    assert_eq!(
        bound_contract.determinism,
        Some(DeterminismClass::Deterministic)
    );
    assert_eq!(bound_contract.side_effect, Some(SideEffectClass::Pure));
    assert_eq!(bound_contract.cost_hint, Some(CostHint::Cheap));
    assert_eq!(
        bound_contract
            .capability_requirements
            .as_ref()
            .expect("Fix: intrinsic capability requirements must be retained.")
            .iter()
            .map(CapabilityId::as_str)
            .collect::<Vec<_>>(),
        ["cuda.sm_90", "warp.vote"]
    );
}

#[test]
fn backend_identity_is_opaque_hashable_and_displayable() {
    let owned = String::from("cuda:sm_90");
    let from_owned = BackendId::from(owned.clone());
    let from_borrowed = BackendId::from(owned.as_str());
    let from_arc = BackendId::new(Arc::<str>::from("cuda:sm_90"));

    assert_eq!(from_owned, from_borrowed);
    assert_eq!(from_borrowed, from_arc);
    assert_eq!(from_owned.as_str(), "cuda:sm_90");
    assert_eq!(from_owned.to_string(), "cuda:sm_90");
    assert_eq!(hash_of(&from_owned), hash_of(&from_borrowed));

    let unnamed = Backend::new(from_owned.clone());
    let named = Backend::named(from_owned.clone(), Arc::<str>::from("NVIDIA CUDA SM90"));
    let round_trip_id = BackendId::from(&named);

    assert_eq!(unnamed.id(), "cuda:sm_90");
    assert_eq!(unnamed.name(), "cuda:sm_90");
    assert_eq!(named.id(), "cuda:sm_90");
    assert_eq!(named.name(), "NVIDIA CUDA SM90");
    assert_eq!(round_trip_id, from_owned);
}

#[test]
fn generated_backend_intrinsic_matrix_preserves_required_backend_semantics() {
    const NON_EMPTY_INTRINSICS: [&str; 4] = [
        "vote.sync.popc",
        "subgroupBallot",
        "OpGroupNonUniformBallotBitCount",
        "simd_sum",
    ];
    const EMPTY_INTRINSICS: [&str; 4] = ["", " ", "\t", "\n"];

    for seed in 0..4096u32 {
        let present_name = format!("cuda.generated.present.{seed:04x}");
        let missing_name = format!("cuda.generated.missing.{seed:04x}");
        let blank_name = format!("cuda.generated.blank.{seed:04x}");
        let present = BackendId::from(present_name.clone());
        let missing = BackendId::from(missing_name.clone());
        let blank = BackendId::from(blank_name.clone());
        let intrinsic = NON_EMPTY_INTRINSICS[(seed as usize) % NON_EMPTY_INTRINSICS.len()];
        let blank_intrinsic = EMPTY_INTRINSICS[(seed as usize) % EMPTY_INTRINSICS.len()];
        let table = IntrinsicTable {
            lowerings: vec![
                IntrinsicLowering::new(present_name, intrinsic),
                IntrinsicLowering::new(blank_name, blank_intrinsic),
            ],
        };
        let required = vec![present.clone(), missing.clone(), blank.clone()];
        let missing_backends = table.missing_backends(&required).collect::<Vec<_>>();

        assert!(
            table.has_backend(&present),
            "Fix: non-empty generated intrinsic spelling must satisfy required backend {seed}."
        );
        assert!(
            !table.has_backend(&missing),
            "Fix: absent generated backend must remain missing {seed}."
        );
        assert!(
            !table.has_backend(&blank),
            "Fix: whitespace-only generated intrinsic spelling must remain missing {seed}."
        );
        assert_eq!(
            missing_backends,
            [missing.as_str(), blank.as_str()],
            "Fix: generated missing-backend order must follow the caller supplied required list {seed}."
        );
    }
}

fn hash_of(value: &BackendId) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
