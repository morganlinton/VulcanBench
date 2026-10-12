//! `TensorRef`  -  typed buffer-argument wrapper for Cat-A ops.
//!
//! Every Cat-A composition that takes a buffer name as `&str` is a
//! landmine: nothing type-checks `attention(q, k, v, out)` when the
//! caller swaps `q` and `k`. `TensorRef` fixes that by pairing the
//! buffer name with shape + dtype metadata so builders can validate
//! at construction time.
//!
//! The type is intentionally shallow: it carries just enough metadata
//! to catch the most common mistakes (dtype mismatch, shape mismatch,
//! name collision). Full tensor-semantic analysis  -  broadcasting,
//! stride inference, view lifetimes  -  belongs in a future
//! `vyre-libs-tensor` layer, but the name + shape + dtype trio here
//! is the frozen API every consumer pins to.
//!
//! **Future-proofing:** `TensorRef` is `#[non_exhaustive]` and its
//! constructor takes `impl Into<…>` so we can add fields without
//! breaking existing call sites.

use std::sync::Arc;
use vyre::ir::{DataType, Ident};

/// A named, typed, shaped buffer argument passed into a Cat-A op.
///
/// Construct with [`TensorRef::new`] or the convenience helpers
/// (`u32_1d`, `f32_1d`, `u32_2d`, `f32_2d`). Downstream ops consume
/// `TensorRef`s instead of raw `&str` buffer names so type + shape
/// checks happen at `build()` time.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TensorRef {
    /// Name the buffer is registered under. Matches `BufferDecl::name`.
    pub name: Ident,
    /// Element dtype. Enforced against each op's expected dtype set.
    pub dtype: DataType,
    /// Logical shape in elements (not bytes). An empty vec is a scalar;
    /// a 2-element vec is a matrix; etc. Used for shape-mismatch
    /// detection at build-time.
    pub shape: Arc<[u32]>,
}

impl TensorRef {
    /// Construct an explicit `TensorRef`. Callers prefer the shape
    /// helpers below unless their shape is computed.
    #[must_use]
    pub fn new(name: impl Into<Ident>, dtype: DataType, shape: Vec<u32>) -> Self {
        Self {
            name: name.into(),
            dtype,
            shape: Arc::from(shape),
        }
    }

    /// U32 1-D tensor convenience constructor.
    #[must_use]
    pub fn u32_1d(name: impl Into<Ident>, len: u32) -> Self {
        Self::new(name, DataType::U32, vec![len])
    }

    /// F32 1-D tensor convenience constructor.
    #[must_use]
    pub fn f32_1d(name: impl Into<Ident>, len: u32) -> Self {
        Self::new(name, DataType::F32, vec![len])
    }

    /// U32 2-D tensor convenience constructor (rows × cols).
    #[must_use]
    pub fn u32_2d(name: impl Into<Ident>, rows: u32, cols: u32) -> Self {
        Self::new(name, DataType::U32, vec![rows, cols])
    }

    /// F16 1-D tensor convenience constructor.
    #[must_use]
    pub fn f16_1d(name: impl Into<Ident>, len: u32) -> Self {
        Self::new(name, DataType::F16, vec![len])
    }

    /// F16 2-D tensor convenience constructor (rows × cols).
    #[must_use]
    pub fn f16_2d(name: impl Into<Ident>, rows: u32, cols: u32) -> Self {
        Self::new(name, DataType::F16, vec![rows, cols])
    }

    /// F32 2-D tensor convenience constructor (rows × cols).
    #[must_use]
    pub fn f32_2d(name: impl Into<Ident>, rows: u32, cols: u32) -> Self {
        Self::new(name, DataType::F32, vec![rows, cols])
    }

    /// Total element count. Returns `None` on overflow so builders
    /// can surface a structured error rather than silent wraparound.
    #[must_use]
    pub fn element_count(&self) -> Option<u32> {
        self.shape
            .iter()
            .try_fold(1u32, |acc, &dim| acc.checked_mul(dim))
    }

    /// Borrow the buffer name as `&str`  -  the form every IR builder
    /// still accepts. Lets Cat-A ops forward to underlying primitives
    /// while keeping the typed surface on the boundary.
    #[must_use]
    pub fn name_str(&self) -> &str {
        self.name.as_str()
    }
}

/// Error returned when [`TensorRef`] arguments fail a builder's
/// dtype / shape / name-uniqueness check.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum TensorRefError {
    /// Dtype doesn't match what the op expects.
    #[error(
        "TensorRef `{name}` has dtype `{found:?}`; op `{op}` expects `{expected:?}`. Fix: pass a buffer of the correct dtype or cast."
    )]
    DtypeMismatch {
        /// Tensor name that failed.
        name: String,
        /// Caller-provided dtype.
        found: DataType,
        /// Dtype the op requires.
        expected: DataType,
        /// Op id for the failing builder.
        op: &'static str,
    },
    /// Shape doesn't match what the op expects.
    #[error(
        "TensorRef `{name}` has shape {found:?}; op `{op}` expects {expected:?}. Fix: reshape or pick a compatible op variant."
    )]
    ShapeMismatch {
        /// Tensor name that failed.
        name: String,
        /// Caller-provided shape.
        found: Vec<u32>,
        /// Shape the op requires.
        expected: Vec<u32>,
        /// Op id for the failing builder.
        op: &'static str,
    },
    /// Two TensorRef args resolve to the same buffer name.
    #[error(
        "TensorRef name collision in op `{op}`: `{name}` appears on multiple arguments. Fix: use distinct buffer names per argument."
    )]
    NameCollision {
        /// The duplicated buffer name.
        name: String,
        /// Op id for the failing builder.
        op: &'static str,
    },
    /// Total element count overflows u32.
    #[error(
        "TensorRef `{name}` element-count overflows u32 for shape {shape:?}. Fix: reduce dimensions below the u32 boundary."
    )]
    ElementCountOverflow {
        /// Tensor name.
        name: String,
        /// Shape that overflowed.
        shape: Vec<u32>,
    },
}

/// Verify that every name in `refs` is unique. Returns
/// [`TensorRefError::NameCollision`] on the first duplicate.
pub fn check_unique_names(refs: &[&TensorRef], op: &'static str) -> Result<(), TensorRefError> {
    for (idx, t) in refs.iter().enumerate() {
        if refs[..idx]
            .iter()
            .any(|previous| previous.name_str() == t.name_str())
        {
            return Err(TensorRefError::NameCollision {
                name: t.name.as_str().to_string(),
                op,
            });
        }
    }
    Ok(())
}

/// Verify a TensorRef matches the expected dtype; returns
/// [`TensorRefError::DtypeMismatch`] on mismatch.
pub fn check_dtype(
    r: &TensorRef,
    expected: DataType,
    op: &'static str,
) -> Result<(), TensorRefError> {
    if r.dtype != expected {
        return Err(TensorRefError::DtypeMismatch {
            name: r.name.as_str().to_string(),
            found: r.dtype.clone(),
            expected,
            op,
        });
    }
    Ok(())
}

/// Verify a TensorRef matches the expected shape; returns
/// [`TensorRefError::ShapeMismatch`] on mismatch.
pub fn check_shape(
    r: &TensorRef,
    expected: &[u32],
    op: &'static str,
) -> Result<(), TensorRefError> {
    if r.shape.as_ref() != expected {
        return Err(TensorRefError::ShapeMismatch {
            name: r.name.as_str().to_string(),
            found: r.shape.to_vec(),
            expected: expected.to_vec(),
            op,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_1d_builder_produces_expected_fields() {
        let t = TensorRef::u32_1d("x", 64);
        assert_eq!(t.name.as_str(), "x");
        assert_eq!(t.dtype, DataType::U32);
        assert_eq!(t.shape.as_ref(), [64]);
        assert_eq!(t.element_count(), Some(64));
    }

    #[test]
    fn element_count_detects_overflow() {
        let t = TensorRef::new("big", DataType::U32, vec![1u32 << 20, 1u32 << 20]);
        assert_eq!(t.element_count(), None);
    }

    #[test]
    fn check_unique_names_catches_collision() {
        let a = TensorRef::u32_1d("x", 4);
        let b = TensorRef::u32_1d("x", 4);
        let err = check_unique_names(&[&a, &b], "test").unwrap_err();
        assert!(matches!(err, TensorRefError::NameCollision { .. }));
    }

    #[test]
    fn check_dtype_passes_on_match() {
        let t = TensorRef::f32_1d("y", 8);
        assert!(matches!(check_dtype(&t, DataType::F32, "op"), Ok(())));
    }

    #[test]
    fn check_dtype_fails_on_mismatch() {
        let t = TensorRef::u32_1d("y", 8);
        let err = check_dtype(&t, DataType::F32, "op").unwrap_err();
        assert!(matches!(err, TensorRefError::DtypeMismatch { .. }));
    }

    #[test]
    fn check_shape_passes_and_fails() {
        let t = TensorRef::u32_2d("m", 4, 8);
        assert!(check_shape(&t, &[4, 8], "op").is_ok());
        let err = check_shape(&t, &[4, 16], "op").unwrap_err();
        assert!(matches!(err, TensorRefError::ShapeMismatch { .. }));
    }
}
