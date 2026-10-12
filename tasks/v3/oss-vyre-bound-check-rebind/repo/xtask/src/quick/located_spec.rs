use std::path::PathBuf;

pub(crate) struct LocatedSpec {
    pub(crate) spec: crate::quick::quick_op::QuickOp,
    pub(crate) source_file: PathBuf,
}
