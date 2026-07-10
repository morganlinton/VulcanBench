//! Control flow for callbacks.

macro_rules! try_control {
    ($e:expr) => {
        match $e {
            x => if x.should_break() {
                return x;
            }
        }
    }
}

/// Control flow for callbacks.
///
/// `Break` can carry a value.
#[derive(Copy, Clone, Debug)]
pub enum Control<B> {
    Continue,
    Break(B),
}

impl<B> Control<B> {
    pub fn breaking() -> Control<()> { Control::Break(()) }
    /// Get the value in `Control::Break(_)`, if present.
    pub fn break_value(self) -> Option<B> {
        match self {
            Control::Continue => None,
            Control::Break(b) => Some(b),
        }
    }
}

/// Control flow for callbacks.
///
/// The empty return value `()` is equivalent to continue.
pub trait ControlFlow {
    fn continuing() -> Self;
    #[inline]
    fn should_break(&self) -> bool { false }
}

impl ControlFlow for () {
    fn continuing() { }
}

impl<B> ControlFlow for Control<B> {
    fn continuing() -> Self { Control::Continue }
    fn should_break(&self) -> bool {
        if let Control::Break(_) = *self { true } else { false }
    }
}

impl<E> ControlFlow for Result<(), E> {
    fn continuing() -> Self { Ok(()) }
    fn should_break(&self) -> bool {
        if let Err(_) = *self { true } else { false }
    }
}
