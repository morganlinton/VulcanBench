//! Test crate.

#[test]
fn dump_matmul_wgsl() {
    let p = vyre_libs::math::matmul("a", "b", "out", 4, 4, 4);
    let lowered = vyre_foundation::optimizer::pre_lowering::optimize(p.clone());
    let wgsl = vyre_driver_wgpu::emit::lower(&lowered).expect("lower");
    println!("===WGSL===\n{wgsl}\n===END===");
}
