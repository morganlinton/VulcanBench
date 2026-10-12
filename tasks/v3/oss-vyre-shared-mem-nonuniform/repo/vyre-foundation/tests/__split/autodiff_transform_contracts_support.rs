use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

pub(crate) fn square_via_local_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::output("out", 1, DataType::F32).with_count(4),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind(
                "y",
                Expr::mul(
                    Expr::load("x", Expr::InvocationId { axis: 0 }),
                    Expr::load("x", Expr::InvocationId { axis: 0 }),
                ),
            ),
            Node::store("out", Expr::InvocationId { axis: 0 }, Expr::var("y")),
        ],
    )
}

pub(crate) fn flatten_nodes(nodes: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    for node in nodes {
        out.push(node);
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                out.extend(flatten_nodes(then));
                out.extend(flatten_nodes(otherwise));
            }
            Node::Loop { body, .. } | Node::Block(body) => out.extend(flatten_nodes(body)),
            Node::Region { body, .. } => out.extend(flatten_nodes(body)),
            _ => {}
        }
    }
    out
}

pub(crate) fn generated_intermediate_buffer_program(seed: u32) -> Program {
    let gid = Expr::InvocationId { axis: 0 };
    let x = Expr::load("x", gid.clone());
    let w = Expr::load("w", gid.clone());
    let tmp_value = match seed % 5 {
        0 => Expr::mul(x.clone(), x.clone()),
        1 => Expr::add(Expr::mul(x.clone(), w.clone()), Expr::f32(1.0)),
        2 => Expr::fma(x.clone(), w.clone(), Expr::f32((seed % 13) as f32 * 0.0625)),
        3 => Expr::div(
            Expr::add(x.clone(), Expr::f32(2.0)),
            Expr::add(w.clone(), Expr::f32(3.0)),
        ),
        _ => Expr::select(
            Expr::bool(seed & 1 == 0),
            Expr::add(x.clone(), w.clone()),
            Expr::mul(x.clone(), w.clone()),
        ),
    };
    let tmp = Expr::load("tmp", gid.clone());
    let out_value = match (seed / 5) % 4 {
        0 => Expr::mul(tmp, x),
        1 => Expr::add(tmp, w),
        2 => Expr::fma(tmp, x, Expr::f32(0.5)),
        _ => Expr::mul(
            Expr::add(tmp, Expr::f32(1.0)),
            Expr::add(x, Expr::f32(0.25)),
        ),
    };

    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::storage("w", 1, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::storage("tmp", 2, BufferAccess::ReadWrite, DataType::F32).with_count(64),
            BufferDecl::output("out", 3, DataType::F32).with_count(64),
        ],
        [64, 1, 1],
        vec![
            Node::store("tmp", gid.clone(), tmp_value),
            Node::store("out", gid, out_value),
        ],
    )
}

pub(crate) fn generated_f32_identity_cast_program(seed: u32) -> Program {
    let gid = Expr::InvocationId { axis: 0 };
    let x = Expr::cast(DataType::F32, Expr::load("x", gid.clone()));
    let w = Expr::cast(DataType::F32, Expr::load("w", gid.clone()));
    let expr = match seed % 6 {
        0 => x,
        1 => Expr::add(x, w),
        2 => Expr::mul(x, w),
        3 => Expr::cast(DataType::F32, Expr::add(x, w)),
        4 => Expr::fma(
            x.clone(),
            Expr::cast(DataType::F32, w),
            Expr::f32((seed % 31) as f32 * 0.03125),
        ),
        _ => Expr::select(
            Expr::bool(seed & 1 == 0),
            Expr::cast(DataType::F32, Expr::mul(x.clone(), w.clone())),
            Expr::cast(DataType::F32, Expr::add(x, w)),
        ),
    };

    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::storage("w", 1, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::output("out", 2, DataType::F32).with_count(64),
        ],
        [64, 1, 1],
        vec![Node::store("out", Expr::InvocationId { axis: 0 }, expr)],
    )
}

pub(crate) fn generated_nondifferentiable_cast_shape(seed: u32) -> (DataType, DataType) {
    let source = match seed % 4 {
        0 => DataType::U32,
        1 => DataType::I32,
        2 => DataType::Bool,
        _ => DataType::F32,
    };
    let target = match (seed / 4) % 4 {
        0 => DataType::F32,
        1 => DataType::U32,
        2 => DataType::I32,
        _ => DataType::Bool,
    };
    let target = if source == DataType::F32 && target == DataType::F32 {
        DataType::U32
    } else {
        target
    };
    (source, target)
}

pub(crate) fn generated_cast_program(source: DataType, target: DataType) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, source).with_count(1),
            BufferDecl::output("out", 1, target.clone()).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::cast(target, Expr::load("x", Expr::u32(0))),
        )],
    )
}

pub(crate) fn generated_differentiable_program(seed: u32) -> Program {
    let gid = Expr::InvocationId { axis: 0 };
    let x = Expr::load("x", gid.clone());
    let w = Expr::load("w", gid.clone());
    let expr = generated_differentiable_expr(seed, x, w);

    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::storage("w", 1, BufferAccess::ReadOnly, DataType::F32).with_count(64),
            BufferDecl::output("out", 2, DataType::F32).with_count(64),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("y", expr),
            Node::store("out", Expr::InvocationId { axis: 0 }, Expr::var("y")),
        ],
    )
}

fn generated_differentiable_expr(seed: u32, x: Expr, w: Expr) -> Expr {
    match seed % 8 {
        0 => Expr::add(x, w),
        1 => Expr::sub(x, w),
        2 => Expr::mul(x, w),
        3 => Expr::div(Expr::add(x, Expr::f32(1.0)), Expr::add(w, Expr::f32(2.0))),
        4 => Expr::fma(x, w, Expr::f32((seed % 17) as f32 * 0.125)),
        5 => Expr::select(
            Expr::bool(seed & 0x2 == 0),
            Expr::mul(x.clone(), w.clone()),
            Expr::add(x, w),
        ),
        6 => Expr::mul(Expr::add(x.clone(), w.clone()), Expr::sub(x, w)),
        _ => Expr::add(
            Expr::fma(x.clone(), w.clone(), Expr::f32(0.5)),
            Expr::select(Expr::bool(seed & 0x4 == 0), x, w),
        ),
    }
}
