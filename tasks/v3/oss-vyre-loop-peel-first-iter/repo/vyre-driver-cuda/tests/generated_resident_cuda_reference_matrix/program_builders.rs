use super::*;

pub(crate) fn resident_bool_binary_program(case: &BoolBinaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_u32_binary_program(case: &ResidentBinaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_u32_unary_program(case: &ResidentUnaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_i32_binary_program(case: &ResidentBinaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::I32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_i32_unary_program(case: &ResidentUnaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::I32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::I32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_memory_program(case: &ResidentMemoryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, case.ty.clone()).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, case.ty.clone()).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                (case.build_dst)(Expr::gid_x()),
                (case.build_value)(Expr::load("input", (case.build_src)(Expr::gid_x()))),
            )],
        )],
    )
}

pub(crate) fn resident_bool_unary_program(case: &BoolUnaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_bool_select_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("flag", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("lhs", 1, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 2, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::select(
                    Expr::load("flag", Expr::gid_x()),
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_f32_compare_program(case: &F32CompareCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_f32_binary_program(case: &F32BinaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("lhs", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("rhs", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 2, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(
                    Expr::load("lhs", Expr::gid_x()),
                    Expr::load("rhs", Expr::gid_x()),
                ),
            )],
        )],
    )
}

pub(crate) fn resident_f32_unary_program(case: &F32UnaryCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_f32_classify_program(case: &F32ClassifyCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                (case.build)(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_atomic_reduction_program(case: &ResidentAtomicCase) -> Program {
    let idx = Expr::var("idx");
    let bucket = Expr::bitand(idx.clone(), Expr::u32(BUCKET_MASK));
    let value = Expr::load("values", idx.clone());
    Program::wrapped(
        vec![
            BufferDecl::storage("acc", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(LANE_COUNT as u32),
            BufferDecl::read("values", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(LANE_COUNT as u32)),
                vec![Node::let_bind(
                    "old_value",
                    (case.build)("acc", bucket, value),
                )],
            ),
        ],
    )
}

pub(crate) fn resident_cast_program(case: &CastCase) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, case.input_type.clone()).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, case.output_type.clone()).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::cast(case.output_type.clone(), Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

pub(crate) fn resident_fma_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("b", 1, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::read("c", 2, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 3, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::fma(
                    Expr::load("a", Expr::gid_x()),
                    Expr::load("b", Expr::gid_x()),
                    Expr::load("c", Expr::gid_x()),
                ),
            )],
        )],
    )
}

