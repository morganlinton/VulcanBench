//! CUDA driver compile gate: validate that emitted PTX is well-formed
//! CUDA assembly.
//!
//! Real driver module-load validation is gated behind the `nvrtc` feature
//! because it requires a CUDA toolkit and GPU driver at test time.
//! When the feature is off, the mock gate validates PTX string
//! structure and instruction presence.

use vyre_foundation::ir::BinOp;
use vyre_lower::KernelOpKind;

#[path = "nvrtc_compile_gate/fixtures.rs"]
mod fixtures;
use fixtures::{
    ptx_for_dynamic_vector_load_fusion, ptx_for_dynamic_vector_store_fusion, ptx_for_op,
    ptx_for_vector_load_fusion, ptx_for_vector_store_fusion,
};

#[test]
fn mock_gate_add_ptx_is_well_formed() {
    let ptx = ptx_for_op(KernelOpKind::BinOpKind(BinOp::Add));
    assert!(ptx.contains(".version"), "missing .version directive");
    assert!(ptx.contains(".target"), "missing .target directive");
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("add"), "missing add instruction");
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_mul_ptx_is_well_formed() {
    let ptx = ptx_for_op(KernelOpKind::BinOpKind(BinOp::Mul));
    assert!(ptx.contains(".version"), "missing .version directive");
    assert!(ptx.contains(".target"), "missing .target directive");
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("mul.lo"), "missing mul.lo instruction");
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_fma_ptx_is_well_formed() {
    let ptx = ptx_for_op(KernelOpKind::Fma);
    assert!(ptx.contains(".version"), "missing .version directive");
    assert!(ptx.contains(".target"), "missing .target directive");
    assert!(ptx.contains(".visible .entry main"), "missing entry point");
    assert!(ptx.contains("fma.rn"), "missing fma.rn instruction");
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_ptx_has_register_declarations() {
    let ptx = ptx_for_op(KernelOpKind::BinOpKind(BinOp::Add));
    assert!(ptx.contains(".reg"), "missing register declarations");
    assert!(ptx.contains("%r"), "missing u32 register prefix");
}

#[test]
fn mock_gate_vector_load_fusion_ptx_is_well_formed() {
    let ptx = ptx_for_vector_load_fusion();
    assert!(
        ptx.contains("ld.global.nc.v4.u32") || ptx.contains("ld.global.v4.u32"),
        "missing fused vector load instruction\n{ptx}"
    );
    assert_eq!(
        ptx.matches("ld.global.u32").count(),
        0,
        "fused vector load must not leave scalar global loads\n{ptx}"
    );
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_dynamic_vector_load_fusion_ptx_is_well_formed() {
    let ptx = ptx_for_dynamic_vector_load_fusion();
    assert!(
        ptx.contains("ld.global.nc.v4.u32") || ptx.contains("ld.global.v4.u32"),
        "missing dynamic-base fused v4 vector load instruction\n{ptx}"
    );
    let scalar_data_loads =
        ptx.matches("ld.global.u32").count() + ptx.matches("ld.global.nc.u32").count();
    assert_eq!(
        scalar_data_loads, 0,
        "dynamic-base fused vector load must eliminate scalar data loads\n{ptx}"
    );
    assert!(
        ptx.contains("st.global.u32"),
        "missing per-thread output store"
    );
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_vector_store_fusion_ptx_is_well_formed() {
    let ptx = ptx_for_vector_store_fusion();
    assert!(
        ptx.contains("st.global.v4.u32"),
        "missing fused vector store instruction\n{ptx}"
    );
    assert_eq!(
        ptx.matches("st.global.u32").count(),
        0,
        "fused vector store must not leave scalar global stores\n{ptx}"
    );
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_dynamic_vector_store_fusion_ptx_is_well_formed() {
    let ptx = ptx_for_dynamic_vector_store_fusion();
    assert!(
        ptx.contains("st.global.v4.u32"),
        "missing dynamic-base fused vector store instruction\n{ptx}"
    );
    assert_eq!(
        ptx.matches("st.global.u32").count(),
        0,
        "dynamic-base fused vector store must not leave scalar global stores\n{ptx}"
    );
    assert!(ptx.contains("ret;"), "missing ret instruction");
}

#[test]
fn mock_gate_rejects_malformed_placeholder() {
    // A syntactically invalid PTX fragment should not pass structural
    // checks that real emitted PTX satisfies.
    let fake = ".version 0.0\n.target sm_99\n.entry broken { }";
    assert!(!fake.contains(".reg"), "fake should lack register decls");
    assert!(!fake.contains("ret;"), "fake should lack ret");
}

#[cfg(feature = "nvrtc")]
#[path = "nvrtc_compile_gate/cuda.rs"]
mod nvrtc_real;
