//! Runtime wire-format serialization for multi-part programs.

pub use decode_parts::decode_parts;
pub use encode_parts::{encode_parts, MAX_SERIALIZED_PART_BYTES};

/// Runtime frame decoder.
pub mod decode_parts;
/// Runtime frame encoder and serializer limits.
pub mod encode_parts;
