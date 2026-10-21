//! `path_reconstruct`  -  Tier-3 shim over
//! [`vyre_primitives::graph::path_reconstruct`].

use vyre::ir::Program;
use vyre_primitives::graph::path_reconstruct::path_reconstruct as primitive_path_reconstruct;

const OP_ID: &str = "vyre-libs::security::path_reconstruct";

/// Signature retained for ABI compatibility.
#[must_use]
pub fn path_reconstruct(
    parent: &str,
    target: &str,
    path_out: &str,
    path_len: &str,
    max_depth: u32,
) -> Program {
    // path_reconstruct does not consume a node count; max_depth is
    // the structural sizing parameter. Use it as the non-degenerate
    // sentinel  -  a zero-depth reconstruction has no useful output.
    crate::security::assert_security_inputs(
        OP_ID,
        max_depth,
        &[
            ("parent", parent),
            ("target", target),
            ("path_out", path_out),
            ("path_len", path_len),
        ],
    );
    crate::region::tag_program(
        OP_ID,
        primitive_path_reconstruct(parent, target, path_out, path_len, max_depth),
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || path_reconstruct("parent", "target", "path_out", "path_len", 4),
        test_inputs: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 1, 2]),
                to_bytes(&[3]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0]),
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[3, 2, 1, 0]), to_bytes(&[4])]]
        }),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::graph::path_reconstruct::cpu_ref;

    #[test]
    fn path_reconstruct_program_emits_buffers() {
        let p = path_reconstruct("parent", "target", "path_out", "path_len", 4);
        let names: Vec<&str> = p.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"parent"));
        assert!(names.contains(&"target"));
        assert!(names.contains(&"path_out"));
        assert!(names.contains(&"path_len"));
    }

    #[test]
    fn path_reconstruct_respects_max_depth() {
        let p = path_reconstruct("parent", "target", "path_out", "path_len", 8);
        let path_out_buf = p
            .buffers()
            .iter()
            .find(|b| b.name() == "path_out")
            .expect("Fix: path_out buffer");
        assert_eq!(path_out_buf.count, 8);
    }

    #[test]
    fn path_reconstruct_cpu_ref_happy_path() {
        let parent = [0, 0, 1, 2];
        let mut scratch = Vec::new();
        let len = cpu_ref(&parent, 3, 4, &mut scratch);
        assert_eq!(len, 4);
        assert_eq!(scratch, vec![3, 2, 1, 0]);
    }

    #[test]
    fn path_reconstruct_cpu_ref_oob_target_returns_self() {
        // Target >= parent.len() => get returns None => current stays unchanged,
        // loop breaks immediately, path = [target].
        let parent = [0, 0, 1, 2];
        let mut scratch = Vec::new();
        let len = cpu_ref(&parent, 10, 4, &mut scratch);
        assert_eq!(len, 1);
        assert_eq!(scratch[0], 10);
    }

    #[test]
    fn path_reconstruct_cpu_ref_cycle_terminates_at_max_depth() {
        // 2-cycle: 1->2, 2->1. Starting at 1, the walk alternates 1,2,1,2...
        // It must stop at max_depth.
        let parent = [0, 2, 1, 3];
        let mut scratch = Vec::new();
        let len = cpu_ref(&parent, 1, 4, &mut scratch);
        assert_eq!(len, 4);
        assert_eq!(scratch, vec![1, 2, 1, 2]);
    }

    #[test]
    fn path_reconstruct_gpu_matches_cpu_reference_on_cycle() {
        let parent = [0u32, 2, 1, 3];
        let target = 1u32;
        let max_depth = 4u32;
        let p = path_reconstruct("parent", "target", "path_out", "path_len", max_depth);
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![
            to_bytes(&parent),
            to_bytes(&[target]),
            to_bytes(&[0, 0, 0, 0]),
            to_bytes(&[0]),
        ];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let gpu_path_bytes = outputs[0].to_bytes();
        let gpu_len = u32::from_le_bytes(outputs[1].to_bytes()[0..4].try_into().unwrap());

        let mut cpu_scratch = Vec::new();
        let cpu_len = cpu_ref(&parent, target, max_depth, &mut cpu_scratch);

        assert_eq!(
            gpu_len, cpu_len,
            "GPU path length must match CPU reference on cycle"
        );
        let gpu_path: Vec<u32> = gpu_path_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(
            &gpu_path[..cpu_len as usize],
            &cpu_scratch[..cpu_len as usize],
            "GPU path must match CPU reference up to max_depth on cyclic parent array"
        );
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn path_reconstruct_empty_buffer_name_should_panic() {
        let _ = path_reconstruct("", "target", "path_out", "path_len", 4);
    }
}
