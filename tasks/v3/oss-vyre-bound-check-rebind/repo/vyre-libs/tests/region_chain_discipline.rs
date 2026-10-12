//! Region-chain discipline  -  every registered Cat-A op must produce a
//! [`Program`] whose top-level entry is exactly one [`Node::Region`]
//! with a stable `generator` ident.
//!
//! Enforces the architectural invariant documented in
//! `docs/lego-block-rule.md` and `audits/vision-adherence-deep-audit-2026-04-21.md`
//! BLOCKER-5: the optimizer, source-mapping, and CSE passes all rely on
//! finding a single Region at the entry of every op so they can treat
//! it as atomic. Bare statement bodies bypass that contract silently.
//!
//! If this test fails, DO NOT weaken the assertion. Add a
//! [`crate::region::wrap_anonymous`] call to the offending op's
//! `fn(...) -> Program` builder.

use vyre_foundation::ir::Node;
use vyre_libs::harness::all_entries;

#[test]
fn every_cat_a_program_entry_is_a_single_generator_region() {
    let mut violations: Vec<String> = Vec::new();

    for entry in all_entries() {
        let program = (entry.build)();
        let body = program.entry();

        if body.len() != 1 {
            violations.push(format!(
                "{}: Program::entry() has {} top-level nodes, expected 1 Region",
                entry.id,
                body.len()
            ));
            continue;
        }

        match &body[0] {
            Node::Region { generator, .. } => {
                let generator_str: &str = generator.as_ref();
                if generator_str.trim().is_empty() {
                    violations.push(format!(
                        "{}: Region present but generator ident is empty",
                        entry.id
                    ));
                }
            }
            other => {
                violations.push(format!(
                    "{}: Program::entry()[0] is not Node::Region (got {:?})",
                    entry.id,
                    std::mem::discriminant(other),
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Fix: {} op(s) violate the region-chain discipline. Wrap their \
         bodies in crate::region::wrap_anonymous(OP_ID, ...) before \
         handing them to Program::new:\n  - {}",
        violations.len(),
        violations.join("\n  - "),
    );
}
