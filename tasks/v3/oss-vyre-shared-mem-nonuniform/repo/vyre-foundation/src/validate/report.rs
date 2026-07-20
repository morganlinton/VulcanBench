use std::borrow::Cow;

use super::ValidationError;

/// Full result of a validation run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Hard validation failures that reject the Program.
    pub errors: Vec<ValidationError>,
    /// Non-fatal diagnostics emitted during validation.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// Return true when the report contains no hard validation failures.
    #[must_use]
    #[inline]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Non-fatal validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationWarning {
    /// Human-readable warning message with an actionable fix.
    pub message: Cow<'static, str>,
}

impl ValidationWarning {
    /// Warning message.
    #[must_use]
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[inline]
pub(crate) fn warn(message: impl Into<Cow<'static, str>>) -> ValidationWarning {
    ValidationWarning {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_is_ok() {
        let report = ValidationReport::default();
        assert_eq!(report.errors.len(), 0);
    }

    #[test]
    fn report_with_error_is_not_ok() {
        let mut report = ValidationReport::default();
        report.errors.push(ValidationError {
            message: Cow::Borrowed("test error"),
        });
        assert!(!report.is_ok());
    }

    #[test]
    fn warn_builds_warning() {
        let w = warn("narrowing cast");
        assert_eq!(w.message(), "narrowing cast");
    }

    #[test]
    fn warning_clone_and_eq() {
        let a = warn("test");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn report_clone_and_eq() {
        let a = ValidationReport::default();
        let b = a.clone();
        assert_eq!(a, b);
    }
}
