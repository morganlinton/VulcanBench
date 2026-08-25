mod cpu_reference_contracts;
mod resident_contracts;
mod source_contracts;
mod via_dispatch_contracts;

pub(super) fn linear_graph() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    // 0 -> 1 -> 2 -> 3
    (vec![0, 1, 2, 3, 3], vec![1, 2, 3], vec![1, 1, 1])
}
