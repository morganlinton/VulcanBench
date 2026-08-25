//! Integration test crate for the containing Vyre package.

use super::*;

#[test]
fn scheduler_lookup_tables_use_static_str_keys() {
    // Structural assertion: pass_index must be FxHashMap<&'static str, usize>.
    fn assert_static_str_map(_: &FxHashMap<&'static str, usize>) {}

    let scheduler = PassScheduler::try_default().expect("Fix: built-in passes must be valid");
    assert_static_str_map(&scheduler.pass_index);

    // N=20 passes: build scheduler, topo-sort runs inside with_passes, then
    // exercise the lookup loop via the metrics runner and direct query methods.
    let mut names: Vec<&'static str> = Vec::with_capacity(20);
    for i in 0..20 {
        let name: &'static str = Box::leak(format!("stress_pass_{i}").into_boxed_str());
        names.push(name);
    }
    let passes: Vec<_> = names
        .iter()
        .map(|&name| {
            ProgramPassKind::new(TestPass {
                metadata: PassMetadata::new(name, &[], &[]),
                changes: false,
            })
        })
        .collect();
    let scheduler20 = PassScheduler::with_passes(passes);
    assert_static_str_map(&scheduler20.pass_index);

    // Lookup phase: run_with_metrics iterates execution_order and checks the
    // indexed dirty flags for every pass.
    let report = scheduler20
        .run_with_metrics(trivial_program())
        .expect("Fix: stress scheduler must run");
    assert_eq!(
        report.passes.len(),
        names.len(),
        "all clean test passes should be considered exactly once before convergence"
    );

    // Direct pass_index lookups via public query API.
    for &name in &names {
        assert!(
            !scheduler20.reaches(name, name),
            "a pass must not reach itself"
        );
        assert!(
            scheduler20.pass_index.contains_key(name),
            "pass_index must contain {name}"
        );
    }

    // Free the leaked boxed strings to prevent sanitizer alarms.
    for name in names {
        unsafe {
            let _ = Box::from_raw(name as *const str as *mut str);
        }
    }
}

#[test]
fn scheduler_preserves_program_identity_when_pass_skips() {
    let program = trivial_program();
    let original_entry = Arc::clone(program.entry_arc());

    let scheduler = PassScheduler::with_passes(vec![ProgramPassKind::new(SkipPass)]);
    let result = scheduler
        .run(program)
        .expect("Fix: scheduler must converge when all passes SKIP");

    assert!(
        Arc::ptr_eq(&original_entry, result.entry_arc()),
        "scheduler must preserve entry Arc identity when a pass returns SKIP; \
         reconcile_runnable_top_level must not allocate a fresh Vec or Arc"
    );
}
