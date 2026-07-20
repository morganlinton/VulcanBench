//! Validation of buffer load and store operations.
//!
//! Every memory access in vyre IR must target a declared buffer, and
//! stores must target a writable buffer (`ReadWrite` or `Workgroup`).
//! This module catches missing buffer declarations and illegal write
//! permissions before the program reaches the GPU.

use crate::ir_inner::model::program::BufferDecl;
use crate::ir_inner::model::types::{BufferAccess, DataType};
use crate::validate::{err, ValidationError};
use rustc_hash::FxHashMap;

/// Validate that a `Node::Store` targets a writable, declared buffer.
///
/// The function checks two invariants: the buffer name must appear in
/// the program's `buffers` list, and its access mode must allow writes.
/// Violations are appended to `errors` with actionable hints.
///
/// # Examples
///
/// `check_store` is `pub(crate)` and runs inside
/// [`crate::validate::validate::validate`] for every `Node::Store`. See
/// that function's unit tests for runnable coverage of the writable /
/// unknown-buffer / Bytes-element branches.
///
/// # Errors
///
/// Appends a `ValidationError` when the buffer is unknown or not
/// writable.
#[inline]
pub(crate) fn check_store(
    buffer: &str,
    buffers: &FxHashMap<&str, &BufferDecl>,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(buf) = buffers.get(buffer) {
        if buf.access != BufferAccess::ReadWrite
            && buf.access != BufferAccess::WriteOnly
            && buf.access != BufferAccess::Workgroup
        {
            errors.push(err(format!(
                "store to non-writable buffer `{buffer}`. Fix: declare it with BufferAccess::ReadWrite, BufferAccess::WriteOnly, or BufferAccess::Workgroup."
            )));
        }
        // L.1.18: V013 was historically enforced only on `Expr::Atomic`,
        // leaving `Node::Store` targeting a `Bytes` buffer to pass
        // validation silently and then fail lower in target-text emission.
        // Extend V013 here so the error surfaces at validate() time.
        if buf.element == DataType::Bytes && !buf.bytes_extraction {
            errors.push(err(format!(
                "V013: store to buffer `{buffer}` with element type `bytes` is not supported. Fix: use a typed buffer (U32/I32/F32/…) for stores, or declare the buffer with `.with_bytes_extraction(true)` when this is a bytes-producing op such as decode.base64."
            )));
        }
    } else {
        errors.push(err(format!(
            "store to unknown buffer `{buffer}`. Fix: declare it in Program::buffers."
        )));
    }
}

/// Validate that an `Expr::Load` targets a declared buffer.
///
/// Loads are less restricted than stores (read-only buffers are fine),
/// but the buffer name must still be declared in the program. This
/// function appends an error when it is not.
///
/// # Examples
///
/// `check_load` is `pub(crate)` and runs inside
/// [`crate::validate::validate::validate`] for every `Expr::Load`. See
/// that function's unit tests for runnable coverage of the
/// unknown-buffer and Bytes-element branches.
///
/// # Errors
///
/// Appends a `ValidationError` when the buffer is not declared.
#[inline]
pub(crate) fn check_load(
    buffer: &str,
    buffers: &FxHashMap<&str, &BufferDecl>,
    errors: &mut Vec<ValidationError>,
) {
    match buffers.get(buffer) {
        None => {
            errors.push(err(format!(
                "load from unknown buffer `{buffer}`. Fix: declare it in Program::buffers."
            )));
        }
        // L.1.18: V013 coverage extends to `Expr::Load`  -  loading from
        // a `Bytes` buffer gives the caller an opaque multi-byte blob
        // that no scalar arithmetic in the IR knows how to consume.
        // Catch it here rather than letting target-text lowering fail with a
        // generic "unexpected Bytes type" diagnostic.
        Some(buf) if buf.element == DataType::Bytes && !buf.bytes_extraction => {
            errors.push(err(format!(
                "V013: load from buffer `{buffer}` with element type `bytes` is not supported. Fix: declare the buffer with a typed element (U32/I32/F32/…) or with `.with_bytes_extraction(true)` when the consuming op is a dedicated bytes-extraction op."
            )));
        }
        Some(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::BufferDecl;

    fn buf_map(decl: &BufferDecl) -> FxHashMap<&str, &BufferDecl> {
        let mut m = FxHashMap::default();
        m.insert(decl.name(), decl);
        m
    }

    #[test]
    fn store_to_unknown_buffer_errors() {
        let buffers: FxHashMap<&str, &BufferDecl> = FxHashMap::default();
        let mut errors = Vec::new();
        check_store("missing", &buffers, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("unknown buffer"));
    }

    #[test]
    fn store_to_readonly_errors() {
        let decl = BufferDecl::read("buf", 0, DataType::U32).with_count(4);
        let buffers = buf_map(&decl);
        let mut errors = Vec::new();
        check_store("buf", &buffers, &mut errors);
        assert!(errors.iter().any(|e| e.message().contains("non-writable")));
    }

    #[test]
    fn store_to_readwrite_passes() {
        let decl =
            BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4);
        let buffers = buf_map(&decl);
        let mut errors = Vec::new();
        check_store("buf", &buffers, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn load_from_unknown_buffer_errors() {
        let buffers: FxHashMap<&str, &BufferDecl> = FxHashMap::default();
        let mut errors = Vec::new();
        check_load("missing", &buffers, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("unknown buffer"));
    }

    #[test]
    fn load_from_declared_buffer_passes() {
        let decl = BufferDecl::read("buf", 0, DataType::U32).with_count(4);
        let buffers = buf_map(&decl);
        let mut errors = Vec::new();
        check_load("buf", &buffers, &mut errors);
        assert!(errors.is_empty());
    }
}
