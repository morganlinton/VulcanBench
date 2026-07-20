pub use super::depth::{DEFAULT_MAX_CALL_DEPTH, DEFAULT_MAX_NESTING_DEPTH, DEFAULT_MAX_NODE_COUNT};
use super::ValidationError;
use std::borrow::Cow;

#[inline]
pub(crate) fn err(message: impl Into<Cow<'static, str>>) -> ValidationError {
    ValidationError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn err_from_string() {
        let e = err(format!("something {}", "wrong"));
        assert!(e.message().contains("something wrong"));
    }

    #[test]
    fn err_from_static_str() {
        let e = err("static message");
        assert_eq!(e.message(), "static message");
    }

    const _: () = assert!(DEFAULT_MAX_CALL_DEPTH > 0);
    const _: () = assert!(DEFAULT_MAX_NESTING_DEPTH > 0);
    const _: () = assert!(DEFAULT_MAX_NODE_COUNT > 0);
}
