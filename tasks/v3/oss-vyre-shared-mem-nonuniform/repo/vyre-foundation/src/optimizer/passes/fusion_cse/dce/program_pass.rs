//! Dead-code elimination  -  registered ProgramPass.
//!
//! The engine itself lives at `super::engine`; this module hooks it
//! into the scheduler's fixpoint loop and invalidation tracking.

use super::engine;
use crate::ir::Program;
use crate::optimizer::{fingerprint_program, vyre_pass, PassResult};

#[vyre_pass(
    name = "dce",
    requires = [],
    invalidates = ["region_inline"],
    analyze = "always",
    phase = "fusion_cse",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
/// Built-in DCE pass.
pub struct DcePass;

impl DcePass {
    /// Run DCE over the program entry.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let before = fingerprint_program(&program);
        let optimized = engine::dce(program);
        PassResult {
            changed: fingerprint_program(&optimized) != before,
            program: optimized,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Node};

    #[test]
    fn dce_analyze_always_runs() {
        let empty = Program::wrapped(vec![], [1, 1, 1], vec![]);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&DcePass, &empty),
            crate::optimizer::PassAnalysis::RUN
        );
    }

    #[test]
    fn dce_transform_removes_dead_let() {
        let dead_let = Node::let_bind("dead", Expr::u32(42));
        let active_let = Node::let_bind("alive", Expr::u32(1));
        let store = Node::store("out", Expr::u32(0), Expr::var("alive"));

        let p = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![dead_let, active_let, store],
        );
        let result = DcePass::transform(p);

        assert!(
            result.changed,
            "DCE failed to detect change when removing dead let"
        );
        assert!(
            !result.program.entry().iter().any(|n| {
                if let Node::Let { name, .. } = n {
                    name == "dead"
                } else {
                    false
                }
            }),
            "DCE should have removed the dead Let node"
        );
    }
}
