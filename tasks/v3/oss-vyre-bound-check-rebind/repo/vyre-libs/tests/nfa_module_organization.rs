//! Organization and generated layout contracts for the scan NFA surface.

#[test]
fn nfa_scan_module_is_split_by_responsibility() {
    let root = include_str!("../src/scan/nfa.rs");
    let alloc = include_str!("../src/scan/nfa/alloc.rs");
    let plan = include_str!("../src/scan/nfa/plan.rs");
    let tables = include_str!("../src/scan/nfa/tables.rs");
    let shards = include_str!("../src/scan/nfa/shards.rs");

    // The contract is responsibility separation, not a line count: the root
    // must keep plan, table packing, and sharding in sibling modules and
    // re-export their public surface explicitly. File size is a guideline
    // surfaced by the large-file advisory, not a law enforced here.
    assert!(
        root.contains("mod plan;")
            && root.contains("mod alloc;")
            && root.contains("mod shards;")
            && root.contains("mod tables;")
            && root.contains("pub use plan::{compile, try_compile, NfaCompileError, NfaPlan};")
            && root.contains("pub use shards::plan_shards;")
            && root.contains("pub use tables::{"),
        "Fix: scan::nfa must keep plan, table packing, and sharding in sibling modules."
    );
    for (name, source) in [
        ("nfa/alloc.rs", alloc),
        ("nfa/plan.rs", plan),
        ("nfa/tables.rs", tables),
        ("nfa/shards.rs", shards),
    ] {
        assert!(
            source.starts_with("//!"),
            "Fix: {name} needs concrete module-level docs."
        );
    }
}

#[cfg(feature = "matching-nfa")]
#[test]
fn generated_nfa_plan_and_table_layout_matrix_is_stable_after_split() {
    use vyre_libs::scan::nfa::{
        build_transition_table, build_transition_table_lane_major, plan_shards, try_compile,
    };
    use vyre_primitives::nfa::subgroup_nfa::{LANES_PER_SUBGROUP, MAX_STATES_PER_SUBGROUP};

    let pattern_sets: &[&[&str]] = &[
        &[],
        &[""],
        &["a"],
        &["abc", "de", "f"],
        &["alpha", "beta", "gamma", "delta"],
        &["\0", "\u{7f}", "\u{80}", "\u{ff}"],
    ];
    let mut checked_sets = 0usize;

    for patterns in pattern_sets {
        let plan = try_compile(patterns).expect("Fix: generated NFA pattern set should compile.");
        let expected_states = 1 + patterns
            .iter()
            .map(|pattern| pattern.len() as u32)
            .sum::<u32>();
        assert_eq!(plan.num_states, expected_states);
        assert_eq!(plan.accept_states.len(), patterns.len());
        assert_eq!(plan.accept_state_ids.len(), patterns.len());
        assert_eq!(plan.accept_start_anchored, vec![false; patterns.len()]);
        assert_eq!(plan.accept_end_anchored, vec![false; patterns.len()]);

        let flat = build_transition_table(patterns);
        let lane_major = build_transition_table_lane_major(patterns);
        let padded_states =
            LANES_PER_SUBGROUP * (plan.num_states as usize).div_ceil(LANES_PER_SUBGROUP);
        assert_eq!(
            flat.len(),
            plan.num_states as usize * 256 * LANES_PER_SUBGROUP
        );
        assert_eq!(lane_major.len(), padded_states * 256 * LANES_PER_SUBGROUP);

        for src in 0..plan.num_states as usize {
            for byte in [0usize, 1, b'a' as usize, b'z' as usize, 0x7f, 0x80, 0xff] {
                for lane in 0..LANES_PER_SUBGROUP {
                    let flat_idx =
                        src * 256 * LANES_PER_SUBGROUP + byte * LANES_PER_SUBGROUP + lane;
                    let lane_major_idx = lane * padded_states * 256 + byte * padded_states + src;
                    assert_eq!(
                        flat[flat_idx], lane_major[lane_major_idx],
                        "Fix: split NFA table packers diverged at src={src}, byte={byte}, lane={lane}."
                    );
                }
            }
        }
        checked_sets += 1;
    }

    let huge = vec!["a".repeat(100); 20];
    let refs = huge.iter().map(String::as_str).collect::<Vec<_>>();
    for shard in plan_shards(&refs) {
        let states = 1 + shard.iter().map(|pattern| pattern.len()).sum::<usize>();
        assert!(
            states <= MAX_STATES_PER_SUBGROUP,
            "Fix: generated NFA shard has {states} states, above subgroup limit."
        );
    }
    assert_eq!(checked_sets, pattern_sets.len());
}
