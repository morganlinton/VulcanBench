#![allow(missing_docs)]

extern crate self as vyre;

mod support;

pub use support::{dialect, ir, ops};

use std::collections::BTreeMap;

use vyre_macros::define_op;

define_op! {
    id = "generated.define_op.scalar.identity",
    dialect = "generated.define_op.scalar",
    category = A,
    inputs = ["u32"],
    outputs = ["u32"],
    laws = [Identity { element: 0 }],
    program = ir::Program { id: 0xA001 },
}

define_op! {
    id = "generated.define_op.scalar.group",
    dialect = "generated.define_op.scalar",
    category = A,
    inputs = ["u32", "u32"],
    outputs = ["u32"],
    laws = [Commutative, Associative, Identity { element: 0 }],
    program = ir::Program { id: 0xA002 },
}

define_op! {
    id = "generated.define_op.scalar.compare",
    dialect = "generated.define_op.scalar",
    category = A,
    inputs = ["i32", "i32"],
    outputs = ["bool"],
    laws = [],
    program = ir::Program { id: 0xA003 },
}

define_op! {
    id = "generated.define_op.memory.load",
    dialect = "generated.define_op.memory",
    category = B,
    inputs = ["ptr", "u32"],
    outputs = ["u32"],
    laws = [],
    program = ir::Program { id: 0xB001 },
}

define_op! {
    id = "generated.define_op.memory.scatter",
    dialect = "generated.define_op.memory",
    category = B,
    inputs = ["ptr", "u32", "u32"],
    outputs = [],
    laws = [],
    program = ir::Program { id: 0xB002 },
}

define_op! {
    id = "generated.define_op.memory.reduce",
    dialect = "generated.define_op.memory",
    category = B,
    inputs = ["ptr", "u32", "u32"],
    outputs = ["u32"],
    laws = [Associative],
    program = ir::Program { id: 0xB003 },
}

define_op! {
    id = "generated.define_op.runtime.dispatch",
    dialect = "generated.define_op.runtime",
    category = C,
    inputs = ["descriptor", "grid", "block"],
    outputs = ["status"],
    laws = [],
    program = ir::Program { id: 0xC001 },
}

define_op! {
    id = "generated.define_op.runtime.barrier",
    dialect = "generated.define_op.runtime",
    category = C,
    inputs = ["scope"],
    outputs = [],
    laws = [Associative],
    program = ir::Program { id: 0xC002 },
}

define_op! {
    id = "generated.define_op.runtime.vote",
    dialect = "generated.define_op.runtime",
    category = C,
    inputs = ["pred", "mask"],
    outputs = ["mask"],
    laws = [Commutative, Associative],
    program = ir::Program { id: 0xC003 },
}

#[derive(Clone, Copy)]
struct ExpectedOp {
    id: &'static str,
    dialect: &'static str,
    category: dialect::Category,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    laws: &'static [ops::AlgebraicLaw],
    program_id: u64,
}

fn generated_cases() -> &'static [ExpectedOp] {
    &[
        ExpectedOp {
            id: "generated.define_op.scalar.identity",
            dialect: "generated.define_op.scalar",
            category: dialect::Category::A,
            inputs: &["u32"],
            outputs: &["u32"],
            laws: &[ops::AlgebraicLaw::Identity { element: 0 }],
            program_id: 0xA001,
        },
        ExpectedOp {
            id: "generated.define_op.scalar.group",
            dialect: "generated.define_op.scalar",
            category: dialect::Category::A,
            inputs: &["u32", "u32"],
            outputs: &["u32"],
            laws: &[
                ops::AlgebraicLaw::Commutative,
                ops::AlgebraicLaw::Associative,
                ops::AlgebraicLaw::Identity { element: 0 },
            ],
            program_id: 0xA002,
        },
        ExpectedOp {
            id: "generated.define_op.scalar.compare",
            dialect: "generated.define_op.scalar",
            category: dialect::Category::A,
            inputs: &["i32", "i32"],
            outputs: &["bool"],
            laws: &[],
            program_id: 0xA003,
        },
        ExpectedOp {
            id: "generated.define_op.memory.load",
            dialect: "generated.define_op.memory",
            category: dialect::Category::B,
            inputs: &["ptr", "u32"],
            outputs: &["u32"],
            laws: &[],
            program_id: 0xB001,
        },
        ExpectedOp {
            id: "generated.define_op.memory.scatter",
            dialect: "generated.define_op.memory",
            category: dialect::Category::B,
            inputs: &["ptr", "u32", "u32"],
            outputs: &[],
            laws: &[],
            program_id: 0xB002,
        },
        ExpectedOp {
            id: "generated.define_op.memory.reduce",
            dialect: "generated.define_op.memory",
            category: dialect::Category::B,
            inputs: &["ptr", "u32", "u32"],
            outputs: &["u32"],
            laws: &[ops::AlgebraicLaw::Associative],
            program_id: 0xB003,
        },
        ExpectedOp {
            id: "generated.define_op.runtime.dispatch",
            dialect: "generated.define_op.runtime",
            category: dialect::Category::C,
            inputs: &["descriptor", "grid", "block"],
            outputs: &["status"],
            laws: &[],
            program_id: 0xC001,
        },
        ExpectedOp {
            id: "generated.define_op.runtime.barrier",
            dialect: "generated.define_op.runtime",
            category: dialect::Category::C,
            inputs: &["scope"],
            outputs: &[],
            laws: &[ops::AlgebraicLaw::Associative],
            program_id: 0xC002,
        },
        ExpectedOp {
            id: "generated.define_op.runtime.vote",
            dialect: "generated.define_op.runtime",
            category: dialect::Category::C,
            inputs: &["pred", "mask"],
            outputs: &["mask"],
            laws: &[
                ops::AlgebraicLaw::Commutative,
                ops::AlgebraicLaw::Associative,
            ],
            program_id: 0xC003,
        },
    ]
}

fn registered_generated_ops() -> BTreeMap<&'static str, dialect::OpDef> {
    inventory::iter::<dialect::OpDefRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .filter(|op| op.id.starts_with("generated.define_op."))
        .map(|op| (op.id, op))
        .collect()
}

#[test]
fn generated_define_op_matrix_registers_every_category_signature_and_law_shape() {
    let registered = registered_generated_ops();
    assert_eq!(
        registered.len(),
        generated_cases().len(),
        "generated define_op! registrations must be unique and complete"
    );

    for expected in generated_cases() {
        let op = registered
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing generated op registration {}", expected.id));
        let inputs = op
            .signature
            .inputs
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        let outputs = op
            .signature
            .outputs
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();

        assert_eq!(op.dialect, expected.dialect, "{}", expected.id);
        assert_eq!(op.category, expected.category, "{}", expected.id);
        assert_eq!(inputs.as_slice(), expected.inputs, "{}", expected.id);
        assert_eq!(outputs.as_slice(), expected.outputs, "{}", expected.id);
        assert!(op.signature.attrs.is_empty(), "{}", expected.id);
        assert!(!op.signature.bytes_extraction, "{}", expected.id);
        assert_eq!(op.laws, expected.laws, "{}", expected.id);
        assert_eq!(
            (op.compose
                .expect("generated op must expose compose factory"))()
            .id,
            expected.program_id,
            "{}",
            expected.id
        );
    }
}

#[test]
fn generated_define_op_inventory_is_stable_across_thousands_of_lookups() {
    let registered = registered_generated_ops();
    let cases = generated_cases();
    let mut assertions = 0usize;

    for seed in 0usize..4096 {
        let expected = cases[seed % cases.len()];
        let op = registered
            .get(expected.id)
            .unwrap_or_else(|| panic!("missing generated op registration {}", expected.id));

        assert_eq!(op.id, expected.id);
        assert_eq!(op.dialect, expected.dialect);
        assert_eq!(op.category, expected.category);
        assert_eq!(op.signature.inputs.len(), expected.inputs.len());
        assert_eq!(op.signature.outputs.len(), expected.outputs.len());
        assert_eq!(op.laws.len(), expected.laws.len());
        assert_eq!(
            (op.compose
                .expect("generated op must expose compose factory"))()
            .id,
            expected.program_id
        );
        assertions += 7;
    }

    assert_eq!(assertions, 4096 * 7);
}
