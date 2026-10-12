use vyre::ir::Program;

pub(super) struct LexProgramPlan {
    pub(super) program: Program,
    pub(super) sparse_output: bool,
    pub(super) keyword_promoted: bool,
}
