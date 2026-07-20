//! Shared byte helpers for canonical primitive evaluators.

use std::{error::Error, fmt};

use crate::workgroup::Memory;
use vyre_primitives::CombineOp;

/// Error returned by canonical primitive reference evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    /// Build an actionable evaluation error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        debug_assert!(message.contains("Fix:"));
        Self { message }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for EvalError {}

/// CPU reference evaluator for one canonical primitive.
pub trait ReferenceEvaluator {
    /// Evaluate the primitive over byte-backed memory payloads.
    ///
    /// # Errors
    ///
    /// Returns [`EvalError`] when the input arity or payload format violates
    /// the primitive contract.
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, EvalError>;
}

pub(crate) fn one_input(inputs: &[Memory], id: &str) -> Result<Vec<u8>, EvalError> {
    if inputs.len() != 1 {
        return Err(EvalError::new(format!(
            "primitive `{id}` expected 1 input memory, got {}. Fix: pass exactly one byte payload.",
            inputs.len()
        )));
    }
    Ok(inputs[0].bytes())
}

pub(crate) fn two_inputs(inputs: &[Memory], id: &str) -> Result<(Vec<u8>, Vec<u8>), EvalError> {
    if inputs.len() != 2 {
        return Err(EvalError::new(format!(
            "primitive `{id}` expected 2 input memories, got {}. Fix: pass left and right byte payloads.",
            inputs.len()
        )));
    }
    Ok((inputs[0].bytes(), inputs[1].bytes()))
}

pub(crate) fn read_u32(bytes: impl AsRef<[u8]>, id: &str) -> Result<u32, EvalError> {
    let bytes = bytes.as_ref();
    if bytes.len() != 4 {
        return Err(EvalError::new(format!(
            "primitive `{id}` expected a 4-byte u32 payload, got {} bytes. Fix: encode scalar inputs as little-endian u32.",
            bytes.len()
        )));
    }
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(crate) fn u32_words(bytes: impl AsRef<[u8]>, id: &str) -> Result<Vec<u32>, EvalError> {
    let bytes = bytes.as_ref();
    if bytes.len() % 4 != 0 {
        return Err(EvalError::new(format!(
            "primitive `{id}` expected u32-aligned bytes, got {} bytes. Fix: encode every element as little-endian u32.",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

pub(crate) fn write_u32s(values: impl IntoIterator<Item = u32>) -> Memory {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Memory::from_bytes(bytes)
}

pub(crate) fn scalar(value: u32) -> Memory {
    Memory::from_bytes(value.to_le_bytes().to_vec())
}

pub(crate) fn unary_u32_scalar(
    inputs: &[Memory],
    id: &str,
    op: impl FnOnce(u32) -> u32,
) -> Result<Memory, EvalError> {
    let input = one_input(inputs, id)?;
    Ok(scalar(op(read_u32(input, id)?)))
}

pub(crate) fn binary_u32_scalar(
    inputs: &[Memory],
    id: &str,
    op: impl FnOnce(u32, u32) -> u32,
) -> Result<Memory, EvalError> {
    let (left, right) = two_inputs(inputs, id)?;
    Ok(scalar(op(read_u32(left, id)?, read_u32(right, id)?)))
}

pub(crate) fn binary_u32_predicate(
    inputs: &[Memory],
    id: &str,
    op: impl FnOnce(u32, u32) -> bool,
) -> Result<Memory, EvalError> {
    binary_u32_scalar(inputs, id, |left, right| u32::from(op(left, right)))
}

pub(crate) fn combine(op: CombineOp, left: u32, right: u32) -> Result<u32, EvalError> {
    Ok(match op {
        CombineOp::Add => left.wrapping_add(right),
        CombineOp::Mul => left.wrapping_mul(right),
        CombineOp::BitAnd => left & right,
        CombineOp::BitOr => left | right,
        CombineOp::BitXor => left ^ right,
        CombineOp::Min => left.min(right),
        CombineOp::Max => left.max(right),
        _ => {
            return Err(EvalError::new(format!(
                "primitive combiner does not support CombineOp variant {op:?}. Fix: register a reference evaluator for the new combiner before dispatch."
            )));
        }
    })
}

pub(crate) fn checked_index(index: u32, len: usize, id: &str) -> Result<usize, EvalError> {
    let index = usize::try_from(index).map_err(|_| {
        EvalError::new(format!(
            "primitive `{id}` index does not fit usize. Fix: keep index regions within platform addressable bounds."
        ))
    })?;
    if index >= len {
        Err(EvalError::new(format!(
            "primitive `{id}` index {index} is outside source length {len}. Fix: validate index regions before dispatch."
        )))
    } else {
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_known_variants_do_not_panic() {
        let cases = [
            (CombineOp::Add, 7, 5, 12),
            (CombineOp::Mul, 7, 5, 35),
            (CombineOp::BitAnd, 0b1100, 0b1010, 0b1000),
            (CombineOp::BitOr, 0b1100, 0b1010, 0b1110),
            (CombineOp::BitXor, 0b1100, 0b1010, 0b0110),
            (CombineOp::Min, 7, 5, 5),
            (CombineOp::Max, 7, 5, 7),
        ];

        for (op, left, right, expected) in cases {
            assert_eq!(combine(op, left, right), Ok(expected), "{op:?}");
        }
    }

    #[test]
    fn scalar_helpers_preserve_contract_checks() {
        let left = Memory::from_bytes(7u32.to_le_bytes().to_vec());
        let right = Memory::from_bytes(5u32.to_le_bytes().to_vec());
        let malformed = Memory::from_bytes(vec![1, 2, 3]);

        assert_eq!(
            binary_u32_scalar(
                &[left.clone(), right.clone()],
                "test_add",
                u32::wrapping_add
            )
            .expect("Fix: valid binary scalar inputs must evaluate")
            .bytes(),
            12u32.to_le_bytes().to_vec()
        );
        assert_eq!(
            binary_u32_predicate(&[left.clone(), right.clone()], "test_gt", |a, b| a > b)
                .expect("Fix: valid binary predicate inputs must evaluate")
                .bytes(),
            1u32.to_le_bytes().to_vec()
        );
        assert_eq!(
            unary_u32_scalar(std::slice::from_ref(&left), "test_not", |value| !value)
                .expect("Fix: valid unary scalar input must evaluate")
                .bytes(),
            (!7u32).to_le_bytes().to_vec()
        );

        assert!(
            binary_u32_scalar(std::slice::from_ref(&left), "test_add", u32::wrapping_add).is_err()
        );
        assert!(unary_u32_scalar(&[malformed], "test_not", |value| !value).is_err());
    }
}
