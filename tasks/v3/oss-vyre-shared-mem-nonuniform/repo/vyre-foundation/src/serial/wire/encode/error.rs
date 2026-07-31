//! Small error type for wire-format hot-path encoders.
//!
//! Every function called per-Node or per-Expr during encode uses this type
//! instead of `String` so that the success path never touches the heap and
//! the rare dynamic error path keeps the returned `Result` compact.

use arrayvec::ArrayString;

/// Error returned by hot-path wire encoders.
///
/// Uses either a borrowed static string (zero allocation) or a boxed bounded
/// buffer when dynamic formatting is required.
#[derive(Debug, Clone)]
pub enum WireEncodeErr {
    /// Borrowed static diagnostic, used by fixed contract failures.
    Static(&'static str),
    /// Bounded diagnostic for errors that include computed values.
    Dynamic(Box<ArrayString<256>>),
}

impl WireEncodeErr {
    /// Create an error from a static string.
    #[inline]
    #[must_use]
    pub const fn static_msg(msg: &'static str) -> Self {
        WireEncodeErr::Static(msg)
    }

    /// Build a dynamic error from prefix + formatted usize + suffix.
    #[inline]
    #[must_use]
    pub fn fmt_usize(prefix: &str, value: usize, suffix: &str) -> Self {
        let mut buf = ArrayString::<256>::new();
        buf.push_str(prefix);
        let mut tmp = itoa::Buffer::new();
        buf.push_str(tmp.format(value));
        buf.push_str(suffix);
        WireEncodeErr::Dynamic(Box::new(buf))
    }

    /// Build a dynamic error from prefix + two formatted usizes + suffix.
    #[inline]
    #[must_use]
    pub fn fmt_usize2(prefix: &str, v1: usize, mid: &str, v2: usize, suffix: &str) -> Self {
        let mut buf = ArrayString::<256>::new();
        buf.push_str(prefix);
        let mut tmp = itoa::Buffer::new();
        buf.push_str(tmp.format(v1));
        buf.push_str(mid);
        buf.push_str(tmp.format(v2));
        buf.push_str(suffix);
        WireEncodeErr::Dynamic(Box::new(buf))
    }

    /// Build a dynamic error from prefix + formatted u64 + suffix.
    #[inline]
    #[must_use]
    pub fn fmt_u64(prefix: &str, value: u64, suffix: &str) -> Self {
        let mut buf = ArrayString::<256>::new();
        buf.push_str(prefix);
        let mut tmp = itoa::Buffer::new();
        buf.push_str(tmp.format(value));
        buf.push_str(suffix);
        WireEncodeErr::Dynamic(Box::new(buf))
    }

    /// Borrow the diagnostic as UTF-8 bytes without allocating.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        <Self as AsRef<[u8]>>::as_ref(self)
    }
}

impl core::fmt::Display for WireEncodeErr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WireEncodeErr::Static(s) => f.write_str(s),
            WireEncodeErr::Dynamic(s) => f.write_str(s.as_str()),
        }
    }
}

impl std::error::Error for WireEncodeErr {}

impl AsRef<str> for WireEncodeErr {
    fn as_ref(&self) -> &str {
        match self {
            WireEncodeErr::Static(s) => s,
            WireEncodeErr::Dynamic(s) => s.as_str(),
        }
    }
}

impl AsRef<[u8]> for WireEncodeErr {
    fn as_ref(&self) -> &[u8] {
        <Self as AsRef<str>>::as_ref(self).as_bytes()
    }
}

impl From<WireEncodeErr> for String {
    fn from(err: WireEncodeErr) -> String {
        String::from(<WireEncodeErr as AsRef<str>>::as_ref(&err))
    }
}

impl From<String> for WireEncodeErr {
    fn from(s: String) -> Self {
        let mut buf = ArrayString::<256>::new();
        let _ = buf.try_push_str(&s);
        WireEncodeErr::Dynamic(Box::new(buf))
    }
}
