//! Test: descriptor dump tests.
use vyre_debug::{dump_descriptor, DescriptorDumpOptions};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

fn minimal_program() -> Program {
    let buffer =
        BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16);
    Program::wrapped(
        vec![buffer],
        [64, 1, 1],
        vec![Node::Store {
            buffer: Ident::from("out"),
            index: Expr::InvocationId { axis: 0 },
            value: Expr::LitU32(7),
        }],
    )
}

#[test]
fn dump_descriptor_renders_minimal_program() {
    let p = minimal_program();
    let desc = vyre_lower::lower(&p).unwrap();
    let dump = dump_descriptor(&desc, &DescriptorDumpOptions::default());
    assert!(dump.text.contains("KernelDescriptor"));
    assert!(dump.text.contains("bindings:"));
    assert!(dump.text.contains("body[]:"));
    assert!(dump.text.contains("7")); // result id of the literal or the literal itself
}

#[test]
fn dump_descriptor_op_counts_match_walk() {
    let p = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16)],
        [64, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(10),
            vec![Node::if_then(
                Expr::eq(Expr::var("i"), Expr::u32(5)),
                vec![Node::Store {
                    buffer: Ident::from("out"),
                    index: Expr::var("i"),
                    value: Expr::LitU32(7),
                }],
            )],
        )],
    );
    let desc = vyre_lower::lower(&p).unwrap();
    let dump = dump_descriptor(&desc, &DescriptorDumpOptions::default());

    // Count ops manually
    let mut total_ops = 0;
    fn walk_body(body: &vyre_lower::KernelBody, count: &mut usize) {
        *count += body.ops.len();
        for child in &body.child_bodies {
            walk_body(child, count);
        }
    }
    walk_body(&desc.body, &mut total_ops);

    let sum: usize = dump.op_counts_by_path.values().sum();
    assert_eq!(sum, total_ops);
}

#[test]
fn dump_descriptor_truncates_when_max_ops_per_body_set() {
    let p = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16)],
        [64, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::u32(2)),
            Node::let_bind("c", Expr::u32(3)),
            Node::let_bind("d", Expr::u32(4)),
            Node::let_bind("e", Expr::u32(5)),
        ],
    );
    let desc = vyre_lower::lower(&p).unwrap();
    let dump = dump_descriptor(
        &desc,
        &DescriptorDumpOptions {
            show_literals: true,
            show_result_ids: true,
            max_ops_per_body: 2,
        },
    );
    assert!(dump.text.contains("<3 more ops>"));
}

#[test]
fn dump_descriptor_show_literals_false_omits_literals_section() {
    let p = minimal_program();
    let desc = vyre_lower::lower(&p).unwrap();
    let dump = dump_descriptor(
        &desc,
        &DescriptorDumpOptions {
            show_literals: false,
            show_result_ids: true,
            max_ops_per_body: usize::MAX,
        },
    );
    assert!(!dump.text.contains("literals:"));
}
