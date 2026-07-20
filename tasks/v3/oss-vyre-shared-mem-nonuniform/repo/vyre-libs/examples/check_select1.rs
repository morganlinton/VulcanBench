//! Check select1_query CPU vs optimized CPU.
use vyre_reference::value::Value;

fn main() {
    let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0u32];
    let queries = [1u32, 2, 3, 4, 5];
    let to_bytes = vyre_primitives::wire::pack_u32_slice;
    let inputs = [to_bytes(&bits), to_bytes(&queries), vec![0u8; 5 * 4]];

    let program = vyre_primitives::bitset::select::select1_query("bits", "queries", "out", 4, 5);
    let optimized = vyre_foundation::optimizer::pre_lowering::optimize(program.clone());

    let cpu_orig = vyre_reference::reference_eval(
        &program,
        &inputs.iter().cloned().map(Value::from).collect::<Vec<_>>(),
    )
    .expect("Fix: original must execute");
    let cpu_opt = vyre_reference::reference_eval(
        &optimized,
        &inputs.iter().cloned().map(Value::from).collect::<Vec<_>>(),
    )
    .expect("Fix: optimized must execute");

    println!(
        "Original CPU output: {:?}",
        cpu_orig.iter().map(|v| v.to_bytes()).collect::<Vec<_>>()
    );
    println!(
        "Optimized CPU output: {:?}",
        cpu_opt.iter().map(|v| v.to_bytes()).collect::<Vec<_>>()
    );

    let orig_bytes: Vec<Vec<u8>> = cpu_orig.iter().map(|v| v.to_bytes()).collect();
    let opt_bytes: Vec<Vec<u8>> = cpu_opt.iter().map(|v| v.to_bytes()).collect();
    assert_eq!(
        orig_bytes, opt_bytes,
        "Original and optimized CPU outputs must match"
    );
    println!("CPU outputs match!");
}
