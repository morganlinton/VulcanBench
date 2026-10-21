use super::*;

fn collect_let_names_preorder<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => out.push(name.as_str()),
            Node::If {
                then, otherwise, ..
            } => {
                collect_let_names_preorder(then, out);
                collect_let_names_preorder(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_let_names_preorder(body, out);
            }
            Node::Region { body, .. } => collect_let_names_preorder(body, out),
            _ => {}
        }
    }
}

#[test]
fn default_offsets_cover_all_slots() {
    let offsets = default_priority_offsets(256);
    let array_offsets = default_priority_offsets_array(256);
    assert_eq!(offsets.len(), PRIORITY_LEVELS as usize + 1);
    assert_eq!(*offsets.last().unwrap(), 256);
    assert_eq!(offsets.as_slice(), array_offsets.as_slice());
    // Every partition has at least base_per_pri slots
    for i in 0..PRIORITY_LEVELS as usize {
        assert!(
            offsets[i + 1] > offsets[i],
            "empty partition at priority {i}"
        );
    }
}

#[test]
fn offsets_with_small_count() {
    let offsets = default_priority_offsets(5);
    // 5 / 5 = 1 per partition
    assert_eq!(offsets, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn priority_offsets_do_not_overlap_epoch() {
    assert!(
        PRIORITY_OFFSETS_BASE > control::EPOCH,
        "priority offsets must not overwrite the batch-fence epoch word"
    );
}

#[test]
fn write_default_offsets_populates_control_buffer() {
    let mut control = crate::megakernel::Megakernel::try_encode_control(false, 1, 0).unwrap();
    write_default_priority_offsets(&mut control, 10).unwrap();
    let read = |word: u32| {
        let start = word as usize * 4;
        u32::from_le_bytes(control[start..start + 4].try_into().unwrap())
    };
    assert_eq!(read(PRIORITY_OFFSETS_BASE), 0);
    assert_eq!(read(PRIORITY_OFFSETS_BASE + PRIORITY_LEVELS), 10);
    assert_eq!(read(control::EPOCH), 0);
}

#[test]
fn priority_scan_produces_valid_ir() {
    let nodes = priority_scan_body(256);
    assert!(
            nodes.len() >= 6,
            "priority scan must include claim outputs, starvation accounting, scan loop, and accounting writeback"
        );
}

#[test]
fn policy_offset_start_never_emits_zero_denominator_rem() {
    let expr = policy_offset_start(Expr::var("start"), Expr::var("end"), Expr::var("lane"));
    let debug = format!("{expr:?}");
    assert!(
        debug.contains("Max") && debug.contains("LitU32(1)"),
        "Fix: empty priority partitions must not lower lane_id % 0; expression was {debug}"
    );
}

#[test]
fn priority_scan_authorizes_tenant_before_claim_cas() {
    let nodes = priority_scan_body(256);
    let mut names = Vec::new();
    collect_let_names_preorder(&nodes, &mut names);
    let tenant_mask = names
        .iter()
        .position(|name| *name == "probe_tenant_mask")
        .expect("Fix: priority scheduler must load the tenant mask before claim CAS");
    let claim_cas = names
        .iter()
        .position(|name| *name == "probe_prev")
        .expect("Fix: priority scheduler must still claim eligible work");
    assert!(
            tenant_mask < claim_cas,
            "Fix: priority scan must not convert unauthorized tenant work into CLAIMED slots; tenant_mask appears at {tenant_mask}, CAS result at {claim_cas}."
        );
}

#[test]
fn strided_probe_count_bounds_total_partition_work() {
    assert_eq!(priority_partition_probe_count(0, 256), 0);
    assert_eq!(priority_partition_probe_count(1, 256), 1);
    assert_eq!(priority_partition_probe_count(256, 256), 1);
    assert_eq!(priority_partition_probe_count(257, 256), 2);

    let offsets = default_priority_offsets(1024);
    let total_worker_probes: u32 = offsets
        .windows(2)
        .map(|window| priority_partition_probe_budget(window[1] - window[0], 256))
        .sum();
    assert!(
        total_worker_probes <= 1024 + 256 * PRIORITY_LEVELS,
        "strided scan must stay linear in ring slots, got {total_worker_probes} probes"
    );
}

#[test]
fn default_priority_scan_masks_duplicate_partition_lanes() {
    let offsets = default_priority_offsets(1024);
    let total_worker_probes: u32 = offsets
        .windows(2)
        .map(|window| priority_partition_probe_budget(window[1] - window[0], 1024))
        .sum();
    assert_eq!(
        total_worker_probes, 1024,
        "default priority scan should touch each slot status at most once per pass"
    );
}
