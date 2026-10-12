use super::LoweringError;
use rustc_hash::FxHashSet;
use vyre_foundation::ir::{ExprNode, Ident, NodeExtension};

pub(crate) trait NagaProgramScanAtomicExpr: Send + Sync + 'static {
    fn naga_scan_atomic_expr(
        &self,
        ext: &dyn ExprNode,
        out: &mut FxHashSet<Ident>,
    ) -> Result<(), LoweringError>;
}

pub(crate) trait NagaProgramScanAtomicNode: Send + Sync + 'static {
    fn naga_scan_atomic_node(
        &self,
        ext: &dyn NodeExtension,
        out: &mut FxHashSet<Ident>,
    ) -> Result<(), LoweringError>;
}

pub(crate) struct NagaProgramScanAtomicExprRegistration {
    pub kind: &'static str,
    pub scanner: &'static dyn NagaProgramScanAtomicExpr,
}

pub(crate) struct NagaProgramScanAtomicNodeRegistration {
    pub kind: &'static str,
    pub scanner: &'static dyn NagaProgramScanAtomicNode,
}

inventory::collect!(NagaProgramScanAtomicExprRegistration);
inventory::collect!(NagaProgramScanAtomicNodeRegistration);

pub(crate) fn scan_registered_atomic_expr(
    ext: &dyn ExprNode,
    out: &mut FxHashSet<Ident>,
) -> Result<bool, LoweringError> {
    for registration in inventory::iter::<NagaProgramScanAtomicExprRegistration> {
        if registration.kind == ext.extension_kind() {
            registration.scanner.naga_scan_atomic_expr(ext, out)?;
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn scan_registered_atomic_node(
    ext: &dyn NodeExtension,
    out: &mut FxHashSet<Ident>,
) -> Result<bool, LoweringError> {
    for registration in inventory::iter::<NagaProgramScanAtomicNodeRegistration> {
        if registration.kind == ext.extension_kind() {
            registration.scanner.naga_scan_atomic_node(ext, out)?;
            return Ok(true);
        }
    }
    Ok(false)
}
