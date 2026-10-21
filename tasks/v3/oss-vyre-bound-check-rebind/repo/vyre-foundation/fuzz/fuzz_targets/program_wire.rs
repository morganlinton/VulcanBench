#![no_main]

use libfuzzer_sys::fuzz_target;
use vyre_foundation::ir::Program;

fuzz_target!(|data: &[u8]| {
    // Invariant 1: never panic on arbitrary input.
    let program = match Program::from_wire(data) {
        Ok(p) => p,
        Err(e) => {
            let msg = e.to_string();
            // Invariant 2: every from_wire error carries a Fix: hint.
            assert!(
                msg.contains("Fix:"),
                "from_wire error must carry a Fix: hint, got: {msg}"
            );
            return;
        }
    };

    // Invariant 3: round-trip equality.
    // from_wire ∘ to_wire ∘ from_wire == from_wire
    let round = program
        .to_wire()
        .expect("Fix: to_wire must succeed for a Program that just decoded; restore this invariant before continuing.");
    let reparsed = Program::from_wire(&round)
        .expect("Fix: reparsing canonical to_wire bytes must succeed; restore this invariant before continuing.");
    assert!(
        program.structural_eq(&reparsed),
        "round-trip structural equality failed"
    );
});
