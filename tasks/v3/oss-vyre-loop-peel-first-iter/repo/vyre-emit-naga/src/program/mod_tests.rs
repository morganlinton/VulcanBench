// Tests for `mod.rs`. Split out per audit item #85 to keep the
// parent file focused on production code.
// (allow(missing_docs) belongs on the enclosing tests module, not this include!()-d chunk.)

use super::atomic_scanner::scan_atomic_targets;
use super::{extension_ops, LoweringError};
use std::sync::Arc;
use vyre_foundation::ir::{BufferDecl, DataType, Ident};
use vyre_foundation::ir::{Expr, ExprNode, Node, Program};

struct OpaqueAtomicExpr;

impl std::fmt::Debug for OpaqueAtomicExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpaqueAtomicExpr")
    }
}

impl ExprNode for OpaqueAtomicExpr {
    fn extension_kind(&self) -> &'static str {
        "test::scan::opaque-atomic"
    }
    fn debug_identity(&self) -> &str {
        "test::scan::opaque-atomic"
    }
    fn result_type(&self) -> Option<DataType> {
        Some(DataType::U32)
    }
    fn cse_safe(&self) -> bool {
        true
    }
    fn stable_fingerprint(&self) -> [u8; 32] {
        [0x42; 32]
    }
    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct OpaqueAtomicExprScanner;

impl extension_ops::NagaProgramScanAtomicExpr for OpaqueAtomicExprScanner {
    fn naga_scan_atomic_expr(
        &self,
        ext: &dyn ExprNode,
        out: &mut rustc_hash::FxHashSet<Ident>,
    ) -> Result<(), LoweringError> {
        ext.as_any()
                .downcast_ref::<OpaqueAtomicExpr>()
                .ok_or_else(|| {
                    LoweringError::invalid(
                        "opaque atomic scanner received the wrong expression payload. Fix: register scanner kinds with matching payload types.",
                    )
                })?;
        out.insert(Ident::from("opaque_target"));
        Ok(())
    }
}

static OPAQUE_ATOMIC_EXPR_SCANNER: OpaqueAtomicExprScanner = OpaqueAtomicExprScanner;

inventory::submit! {
    extension_ops::NagaProgramScanAtomicExprRegistration {
        kind: "test::scan::opaque-atomic",
        scanner: &OPAQUE_ATOMIC_EXPR_SCANNER,
    }
}

#[derive(Debug)]
struct OpaqueUnknownExpr;
impl ExprNode for OpaqueUnknownExpr {
    fn extension_kind(&self) -> &'static str {
        "test::scan::opaque-unknown"
    }
    fn debug_identity(&self) -> &str {
        "test::scan::opaque-unknown"
    }
    fn result_type(&self) -> Option<DataType> {
        Some(DataType::U32)
    }
    fn cse_safe(&self) -> bool {
        true
    }
    fn stable_fingerprint(&self) -> [u8; 32] {
        [0x99; 32]
    }
    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn atomic_scan_collects_targets_from_opaque_expr_extensions() {
    let mut scanner = rustc_hash::FxHashSet::default();
    let expr = Expr::Opaque(Arc::new(OpaqueAtomicExpr));
    let node = Node::let_bind("x", expr);
    scan_atomic_targets(&node, &mut scanner)
        .expect("Fix: atomic scanner should honor extension scan traits.");
    assert!(scanner.contains(&Ident::from("opaque_target")));
}

#[test]
fn atomic_scan_rejects_unknown_opaque_expr_extensions() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 1, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::Opaque(Arc::new(OpaqueUnknownExpr)),
        )],
    );
    let mut scanner = rustc_hash::FxHashSet::default();
    let err = scan_atomic_targets(&program.entry()[0], &mut scanner)
        .expect_err("Fix: unsupported opaque atomics should fail with actionable error.");
    let message = err.to_string();
    assert!(message.contains("unsupported opaque expression"));
    assert!(message.contains("Fix:"));
}
