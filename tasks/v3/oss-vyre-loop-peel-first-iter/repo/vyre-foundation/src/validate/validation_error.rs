//! Validation error type for vyre IR programs.

use core::fmt;
use std::borrow::Cow;
use std::sync::Arc;

/// A validation error in a vyre Program.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// What is wrong.
    pub message: Cow<'static, str>,
}

impl ValidationError {
    /// Build an unsupported-operation diagnostic for backend capability checks.
    #[must_use]
    pub fn unsupported_op(backend: &'static str, op_id: &Arc<str>, node_index: usize) -> Self {
        Self {
            message: Cow::Owned(format!(
                "backend `{backend}` does not support operation `{op_id}` at node {node_index}. Fix: choose a backend whose capability set includes this operation, lower the program through a supported backend pipeline, or register an implementation for `{op_id}`."
            )),
        }
    }

    /// Error message.
    #[must_use]
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vyre IR validation: {}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_op_contains_fix_hint() {
        let err = ValidationError::unsupported_op("backend-a", &Arc::from("math::fma"), 3);
        assert!(err.message().contains("backend-a"));
        assert!(err.message().contains("math::fma"));
        assert!(err.message().contains("3"));
        assert!(err.message().contains("Fix:"));
    }

    #[test]
    fn message_accessor() {
        let err = ValidationError {
            message: Cow::Borrowed("test"),
        };
        assert_eq!(err.message(), "test");
    }

    #[test]
    fn display_formatting() {
        let err = ValidationError {
            message: Cow::Borrowed("bad IR"),
        };
        assert_eq!(err.to_string(), "vyre IR validation: bad IR");
    }

    #[test]
    fn clone_and_eq() {
        let a = ValidationError {
            message: Cow::Borrowed("same"),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
