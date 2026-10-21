use crate::paths;
use std::path::PathBuf;

pub(crate) fn load_spec_by_id(op_id: &str) -> Option<crate::quick::located_spec::LocatedSpec> {
    let (spec, file): (crate::quick::quick_op::QuickOp, PathBuf) = match op_id {
        "primitive.bitwise.xor" => (
            crate::quick::quick_op::QuickOp {
                id: "primitive.bitwise.xor",
                arity: 2,
                laws: &[
                    crate::quick::quick_law::QuickLaw::Commutative,
                    crate::quick::quick_law::QuickLaw::Associative,
                    crate::quick::quick_law::QuickLaw::Identity(0),
                    crate::quick::quick_law::QuickLaw::SelfInverse(0),
                ],
                eval: crate::quick::eval_xor::eval_xor,
            },
            paths::workspace_root().join("vyre-core/src/ops/primitive/bitwise/xor.rs"),
        ),
        _ => return None,
    };

    Some(crate::quick::located_spec::LocatedSpec {
        spec,
        source_file: file,
    })
}
