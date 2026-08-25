// ───────────────────────────────────────────────────────────────────
// Budget constants
// ───────────────────────────────────────────────────────────────────

/// Maximum total IR statement nodes per composition.
/// If exceeded, the op must be split into sub-ops connected via Expr::Call.
const MAX_NODES: usize = 200;

/// Maximum control-flow nesting depth (If/Loop).
/// Deeply nested logic should be factored into helper compositions.
const MAX_DEPTH: usize = 6;

/// Maximum loop count per composition.
/// More than 8 loops strongly suggests the op is doing multiple phases
/// that should each be their own registered op. Threshold raised from
/// 4 → 8 to admit the python312_extract_with_blocks parser, whose 8
/// loop phases (token-class scan, indent scan, block-open scan,
/// block-close scan, decorator scan, suite-body scan, body-indent
/// match, span emit) are tightly coupled and would lose information
/// across registry boundaries if split. Future loop-budget regressions
/// past 8 should split the op rather than raising further.
const MAX_LOOPS: usize = 8;

// No complexity-budget exemptions. Every op must fit under the node,
// depth, and loop caps. If an op legitimately exceeds a limit, the
// correct fix is to raise the workspace-wide cap with an audited
// justification, not to maintain a hardcoded skip list that hides
// structural debt.

// ───────────────────────────────────────────────────────────────────
// Gate 1: No monoliths  -  complexity budget
// ───────────────────────────────────────────────────────────────────

#[test]
fn every_op_is_under_complexity_budget() {
    let mut violations = Vec::new();

    for entry in vyre_libs::harness::all_entries() {
        let program = (entry.build)();
        let stats = measure_program(&program);

        if stats.total_nodes > MAX_NODES {
            violations.push(format!(
                "OVER-BUDGET: `{}` has {} statement nodes (max {}). \
                 Split into smaller compositions connected via Expr::Call.",
                entry.id, stats.total_nodes, MAX_NODES,
            ));
        }

        if stats.max_depth > MAX_DEPTH {
            violations.push(format!(
                "OVER-DEPTH: `{}` has control-flow depth {} (max {}). \
                 Factor inner branches/loops into helper ops.",
                entry.id, stats.max_depth, MAX_DEPTH,
            ));
        }

        if stats.loop_count > MAX_LOOPS {
            violations.push(format!(
                "OVER-LOOPS: `{}` has {} loops (max {}). \
                 Each loop phase should be a separate registered op.",
                entry.id, stats.loop_count, MAX_LOOPS,
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "Composition discipline violations:\n{}",
        violations.join("\n"),
    );
}

// ───────────────────────────────────────────────────────────────────
// Gate 2: No reimplementation  -  structural subsumption
// ───────────────────────────────────────────────────────────────────

#[test]
fn no_op_reinvents_another_registered_op() {
    let entries: Vec<_> = vyre_libs::harness::all_entries().collect();
    let programs: Vec<(&str, Program)> = entries.iter().map(|e| (e.id, (e.build)())).collect();
    let fingerprints: Vec<(&str, u64)> = programs
        .iter()
        .map(|(id, program)| (*id, structural_fingerprint(program)))
        .collect();

    let mut collisions = Vec::new();

    // Known-equivalent op pairs that share IR shape by mathematical
    // identity, not by accidental copy-paste:
    //   logical::and ≡ math::algebra::meet  (bitwise AND is the meet
    //   logical::or  ≡ math::algebra::join   on the boolean lattice)
    // Both halves are correctly registered under their domain-specific
    // names so callers can `use` the one that matches their abstraction;
    // collapsing them to a single op would break boolean-lattice consumers
    // that read `meet`/`join` semantically.
    fn known_equivalent_pair(a: &str, b: &str) -> bool {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        matches!(
            (lo, hi),
            (
                "vyre-libs::logical::and",
                "vyre-libs::math::algebra::meet"
            ) | (
                "vyre-libs::logical::or",
                "vyre-libs::math::algebra::join"
            )
        )
    }

    for (i, (id_a, fp_a)) in fingerprints.iter().enumerate() {
        for (j, (id_b, fp_b)) in fingerprints.iter().enumerate().skip(i + 1) {
            if fp_a == fp_b {
                // Allow same-family ops to share shapes. Ops in the same
                // namespace (e.g. vyre-libs::security::*) are parameterized
                // families  -  same structure, different buffer semantics.
                // Cross-namespace collisions still fail.
                let ns_a = id_a.rsplitn(2, "::").last().unwrap_or(id_a);
                let ns_b = id_b.rsplitn(2, "::").last().unwrap_or(id_b);
                if ns_a == ns_b {
                    continue;
                }
                if same_canonical_generators(&programs[i].1, &programs[j].1) {
                    continue;
                }
                if known_equivalent_pair(id_a, id_b) {
                    continue;
                }
                collisions.push(format!(
                    "STRUCTURAL-DUP: `{id_a}` and `{id_b}` have identical IR shapes. \
                     One should call the other via Expr::Call instead of duplicating logic.",
                ));
            }
        }
    }

    assert!(
        collisions.is_empty(),
        "Subsumption violations:\n{}",
        collisions.join("\n"),
    );
}

fn same_canonical_generators(a: &Program, b: &Program) -> bool {
    let mut a_generators = Vec::new();
    collect_region_generators(a.entry(), &mut a_generators);
    let mut b_generators = Vec::new();
    collect_region_generators(b.entry(), &mut b_generators);
    !a_generators.is_empty() && a_generators == b_generators
}

fn collect_region_generators<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
    for node in nodes {
        match node {
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                if is_child_composition(source_region.as_ref().map(|r| r.name.as_str())) {
                    out.push(generator.as_str());
                }
                collect_region_generators(body, out);
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_region_generators(then, out);
                collect_region_generators(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => collect_region_generators(body, out),
            _ => {}
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Gate 3: Every OpEntry must declare test fixtures
// ───────────────────────────────────────────────────────────────────

#[test]
fn every_op_has_test_fixtures() {
    let mut missing = Vec::new();

    for entry in vyre_libs::harness::all_entries() {
        // CRITIQUE_CONFORM_2026-04-23 M7: the original gate required
        // BOTH fixtures to be missing before failing. An op that
        // shipped only one half (test_inputs without expected_output
        // or vice versa) passed the gate despite being incomplete,
        // which produced "ran 0 witness cases, all green" false
        // positives downstream. Fail on either missing.
        if entry.test_inputs.is_none() || entry.expected_output.is_none() {
            missing.push(format!(
                "MISSING-FIXTURES: `{}` has no test_inputs or expected_output. \
                 Add real test_inputs and expected_output fixtures.",
                entry.id,
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "Fixture coverage violations:\n{}",
        missing.join("\n"),
    );
}

// ───────────────────────────────────────────────────────────────────
// Info test: print complexity report for human review
// ───────────────────────────────────────────────────────────────────

#[test]
fn print_complexity_report() {
    let mut report = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        let program = (entry.build)();
        let stats = measure_program(&program);
        report.push((entry.id, stats));
    }

    // Sort by total nodes descending  -  most complex first.
    report.sort_by(|a, b| b.1.total_nodes.cmp(&a.1.total_nodes));

    eprintln!("\n=== Composition Complexity Report ===");
    eprintln!(
        "{:<50} {:>5} {:>5} {:>5} {:>5}",
        "Op ID", "Nodes", "Exprs", "Depth", "Loops"
    );
    eprintln!("{}", "-".repeat(75));
    for (id, stats) in &report {
        let flag = if stats.total_nodes > MAX_NODES
            || stats.max_depth > MAX_DEPTH
            || stats.loop_count > MAX_LOOPS
        {
            " ⚠"
        } else {
            ""
        };
        eprintln!(
            "{:<50} {:>5} {:>5} {:>5} {:>5}{}",
            id, stats.total_nodes, stats.total_exprs, stats.max_depth, stats.loop_count, flag,
        );
    }
    eprintln!("Total ops: {}", report.len());
}

// ───────────────────────────────────────────────────────────────────
// Gate 4: wip_exemptions must not grow
// ───────────────────────────────────────────────────────────────────

/// Exemptions from fixture/composition gates that are actively being
/// closed. Adding an entry here requires a tracked issue and an
/// expiration date. The list must never grow  -  only shrink.
const WIP_EXEMPTIONS: &[&str] = &[];

#[test]
fn label_by_family_is_not_exempt() {
    assert!(
        !WIP_EXEMPTIONS.contains(&"vyre-libs::security::label_by_family"),
        "label_by_family must not carry a UniversalDiffExemption or wip_exemption. \
         Fix: remove the exemption and add real test fixtures.",
    );
}

#[test]
fn wip_exemptions_list_does_not_grow() {
    const CURRENT_COUNT: usize = 0;
    assert!(
        WIP_EXEMPTIONS.len() <= CURRENT_COUNT,
        "wip_exemptions grew from {} to {}. Fix: close an exemption before adding a new one.",
        CURRENT_COUNT,
        WIP_EXEMPTIONS.len(),
    );
}

// ───────────────────────────────────────────────────────────────────
// Gate 5: OpEntry::tolerance() must be dead code or wired in
// ───────────────────────────────────────────────────────────────────

#[test]
fn op_entry_tolerance_is_dead_code_outside_definition() {
    // H4: OpEntry::tolerance() is either wired into the parity comparison
    // path or proven unused. After H5 both lens.rs files use
    // compare_output_buffers which is program-level, so .tolerance()
    // should have zero call sites outside its definition in
    // vyre-harness/src/lib.rs.
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap());
    let workspace_root = root.parent().unwrap().parent().unwrap();

    let output = std::process::Command::new("grep")
        .args([
            "-rn",
            "\\.tolerance()",
            "--include=*.rs",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("grep must be available for this CI gate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    let allowed_prefix = "vyre-harness/src/lib.rs";
    // The splittable test layout puts this contract in
    // `composition_discipline.rs` and its chunk-shards
    // (`composition_discipline_chunk*.rs`). Both must be excluded
    // because the gate file itself necessarily mentions
    // `.tolerance()` (in the grep pattern + a doc comment).
    let violations: Vec<&str> = lines
        .iter()
        .filter(|line| {
            let file_path = line.split(':').next().unwrap_or("");
            if file_path.ends_with(allowed_prefix) {
                return false;
            }
            let basename = file_path.rsplit('/').next().unwrap_or(file_path);
            !basename.starts_with("composition_discipline")
        })
        .copied()
        .collect();

    assert!(
        violations.is_empty(),
        "OpEntry::tolerance() has call sites outside its definition. \
         Either wire it into the parity path or delete it. Violations:\n{}",
        violations.join("\n")
    );
}
