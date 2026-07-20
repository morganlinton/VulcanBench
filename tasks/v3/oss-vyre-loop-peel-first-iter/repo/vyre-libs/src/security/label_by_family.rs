//! `label_by_family`  -  Tier-3 shim over
//! [`vyre_primitives::label::resolve_family`].

use vyre::ir::Program;
use vyre_primitives::label::resolve_family::resolve_family;

const OP_ID: &str = "vyre-libs::security::label_by_family";

/// Resolve every node whose tag bitmap intersects `family_mask`.
#[must_use]
pub fn label_by_family(
    node_tags: &str,
    nodeset_out: &str,
    node_count: u32,
    family_mask: u32,
) -> Program {
    crate::security::assert_security_inputs(
        OP_ID,
        node_count,
        &[("node_tags", node_tags), ("nodeset_out", nodeset_out)],
    );
    crate::region::tag_program(
        OP_ID,
        resolve_family(node_tags, nodeset_out, node_count, family_mask),
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || label_by_family("node_tags", "out", 4, 0b0010),
        test_inputs: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0x01, 0x02, 0x06, 0x04]),
                to_bytes(&[0]),
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b0110])]]
        }),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn label_by_family_program_emits_buffers() {
        let p = label_by_family("node_tags", "out", 4, 0b0010);
        let names: Vec<&str> = p.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"node_tags"));
        assert!(names.contains(&"out"));
    }

    #[test]
    fn label_by_family_respects_family_mask() {
        // Node tags: [0x01, 0x02, 0x06, 0x04]
        // Mask 0b0010 matches nodes with bit 1 set: 0x02 (node 1) and 0x06 (node 2).
        let p = label_by_family("node_tags", "out", 4, 0b0010);
        let out_buf = p
            .buffers()
            .iter()
            .find(|b| b.name() == "out")
            .expect("Fix: out buffer");
        // bitset_words(4) = 1
        assert_eq!(out_buf.count, 1);
    }

    #[test]
    fn label_by_family_empty_tags_returns_empty_nodeset() {
        let p = label_by_family("node_tags", "out", 4, 0b0010);
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![to_bytes(&[0, 0, 0, 0]), to_bytes(&[0])];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let out_word = u32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert_eq!(out_word, 0, "empty node_tags must produce empty nodeset");
    }

    #[test]
    fn label_by_family_zero_mask_matches_nothing() {
        let p = label_by_family("node_tags", "out", 4, 0);
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![to_bytes(&[0xFF, 0xFF, 0xFF, 0xFF]), to_bytes(&[0])];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let out_word = u32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert_eq!(out_word, 0, "family_mask = 0 must match nothing");
    }

    #[test]
    fn label_by_family_universal_mask_matches_all_nonzero() {
        let p = label_by_family("node_tags", "out", 4, 0xFFFFFFFF);
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![to_bytes(&[0x01, 0x02, 0x04, 0x08]), to_bytes(&[0])];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let out_word = u32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert_eq!(
            out_word, 0b1111,
            "family_mask = 0xFFFFFFFF must match all non-zero tags"
        );
    }

    #[test]
    fn label_by_family_max_node_count_does_not_panic() {
        let p = label_by_family("node_tags", "out", 32, 0b0001);
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![to_bytes(&[0xFFFFFFFF; 32]), to_bytes(&[0; 1])];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let out_word = u32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert_eq!(
            out_word, 0xFFFFFFFF,
            "max node_count with all tags set must return full bitset"
        );
    }

    #[test]
    #[should_panic(expected = "node_count must be positive")]
    fn label_by_family_zero_node_count_should_panic() {
        let _ = label_by_family("node_tags", "out", 0, 0xFF);
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn label_by_family_empty_buffer_name_should_panic() {
        let _ = label_by_family("", "out", 4, 0xFF);
    }

    proptest! {
        #[test]
        fn label_by_family_proptest_random_mask_and_tags(
            tags in prop::collection::vec(0u32..0x10000, 1..64),
            mask in 0u32..0x10000,
        ) {
            let node_count = tags.len() as u32;
            let words = node_count.div_ceil(32);
            let expected = vyre_primitives::label::resolve_family::cpu_ref(&tags, mask);
            let p = label_by_family("node_tags", "out", node_count, mask);
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            let inputs = vec![
                to_bytes(&tags),
                to_bytes(&vec![0u32; words as usize]),
            ];
            let values: Vec<vyre_reference::value::Value> =
                inputs.into_iter().map(vyre_reference::value::Value::from).collect();
            let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
            let gpu_words: Vec<u32> =
                vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
            prop_assert_eq!(gpu_words, expected);
        }
    }
}
