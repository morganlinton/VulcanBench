use std::sync::Arc;

use crate::extension::OpaqueExprResolver;
use crate::ir_inner::model::expr::{Expr, ExprNode};
use crate::ir_inner::model::types::DataType;

const KIND_I64: &str = "vyre.literal.i64";
const KIND_U64: &str = "vyre.literal.u64";
const KIND_F64: &str = "vyre.literal.f64";

#[derive(Debug)]
struct WideLiteralExpr {
    kind: &'static str,
    debug_identity: String,
    result_type: DataType,
    payload: [u8; 8],
    fingerprint: [u8; 32],
}

impl WideLiteralExpr {
    fn new(
        kind: &'static str,
        debug_identity: String,
        result_type: DataType,
        payload: [u8; 8],
    ) -> Self {
        let fingerprint = *blake3::hash(&[kind.as_bytes(), &payload].concat()).as_bytes();
        Self {
            kind,
            debug_identity,
            result_type,
            payload,
            fingerprint,
        }
    }
}

impl ExprNode for WideLiteralExpr {
    fn extension_kind(&self) -> &'static str {
        self.kind
    }

    fn debug_identity(&self) -> &str {
        &self.debug_identity
    }

    fn result_type(&self) -> Option<DataType> {
        Some(self.result_type.clone())
    }

    fn cse_safe(&self) -> bool {
        true
    }

    fn stable_fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn wire_payload(&self) -> Vec<u8> {
        self.payload.to_vec()
    }
}

fn decode_i64(payload: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    let bytes: [u8; 8] = payload.try_into().map_err(|_| {
        "invalid i64 literal payload width. Fix: encode exactly 8 bytes for opaque i64 literals."
            .to_string()
    })?;
    Ok(Arc::new(WideLiteralExpr::new(
        KIND_I64,
        format!("i64({})", i64::from_le_bytes(bytes)),
        DataType::I64,
        bytes,
    )))
}

fn decode_u64(payload: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    let bytes: [u8; 8] = payload.try_into().map_err(|_| {
        "invalid u64 literal payload width. Fix: encode exactly 8 bytes for opaque u64 literals."
            .to_string()
    })?;
    Ok(Arc::new(WideLiteralExpr::new(
        KIND_U64,
        format!("u64({})", u64::from_le_bytes(bytes)),
        DataType::U64,
        bytes,
    )))
}

fn decode_f64(payload: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    let bytes: [u8; 8] = payload.try_into().map_err(|_| {
        "invalid f64 literal payload width. Fix: encode exactly 8 bytes for opaque f64 literals."
            .to_string()
    })?;
    Ok(Arc::new(WideLiteralExpr::new(
        KIND_F64,
        format!("f64({})", f64::from_le_bytes(bytes)),
        DataType::F64,
        bytes,
    )))
}

inventory::submit! {
    OpaqueExprResolver {
        kind: KIND_I64,
        deserialize: decode_i64,
    }
}

inventory::submit! {
    OpaqueExprResolver {
        kind: KIND_U64,
        deserialize: decode_u64,
    }
}

inventory::submit! {
    OpaqueExprResolver {
        kind: KIND_F64,
        deserialize: decode_f64,
    }
}

impl Expr {
    /// Construct a 64-bit signed integer literal extension.
    #[must_use]
    #[inline]
    pub fn i64(value: i64) -> Expr {
        Expr::opaque_arc(Arc::new(WideLiteralExpr::new(
            KIND_I64,
            format!("i64({value})"),
            DataType::I64,
            value.to_le_bytes(),
        )))
    }

    /// Construct a 64-bit unsigned integer literal extension.
    #[must_use]
    #[inline]
    pub fn u64(value: u64) -> Expr {
        Expr::opaque_arc(Arc::new(WideLiteralExpr::new(
            KIND_U64,
            format!("u64({value})"),
            DataType::U64,
            value.to_le_bytes(),
        )))
    }

    /// Construct a 64-bit floating-point literal extension.
    #[must_use]
    #[inline]
    pub fn f64(value: f64) -> Expr {
        Expr::opaque_arc(Arc::new(WideLiteralExpr::new(
            KIND_F64,
            format!("f64({value})"),
            DataType::F64,
            value.to_le_bytes(),
        )))
    }
}
