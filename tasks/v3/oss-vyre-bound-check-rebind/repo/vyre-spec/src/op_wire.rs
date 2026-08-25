macro_rules! impl_builtin_wire_tag {
    ($name:ident, $opaque:ident, { $($variant:ident => $tag:expr),+ $(,)? }) => {
        impl $name {
            /// Frozen builtin wire tag for this operation.
            ///
            /// Returns `None` for extension-declared opaque operators because their
            /// wire representation is the high-bit extension id, not a core tag.
            #[must_use]
            pub const fn builtin_wire_tag(&self) -> Option<u8> {
                match self {
                    $(Self::$variant => Some($tag),)+
                    Self::$opaque(_) => None,
                }
            }
        }
    };
}
