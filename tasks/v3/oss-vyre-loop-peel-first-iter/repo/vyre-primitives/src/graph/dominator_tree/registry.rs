use super::program::{dominator_tree_program, OP_ID};

inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || dominator_tree_program(4, 4, 4, "idom"),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[0, 1, 2, 3, 3]),
                crate::wire::pack_u32_slice(&[1, 2, 3, 0]),
                crate::wire::pack_u32_slice(&[0, 0, 1, 2, 3]),
                crate::wire::pack_u32_slice(&[0, 1, 2, 0]),
                crate::wire::pack_u32_slice(&[0; 4]),
                crate::wire::pack_u32_slice(&[0; 4]),
            ]]
        }),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[0, 0, 1, 2]),
                crate::wire::pack_u32_slice(&[0, 1, 2, 3]),
            ]]
        }),
    )
}
