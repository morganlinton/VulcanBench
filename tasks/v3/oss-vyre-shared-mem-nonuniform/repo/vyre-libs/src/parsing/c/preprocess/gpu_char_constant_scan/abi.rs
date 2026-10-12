/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_char_constant_scan";

/// Canonical binding indices.
pub const BINDING_SOURCE: u32 = 0;
/// Canonical binding for the input start position.
pub const BINDING_START_POS: u32 = 1;
/// Canonical binding for the output literal value.
pub const BINDING_VALUE_OUT: u32 = 2;
/// Canonical binding for the output bytes-consumed count.
pub const BINDING_BYTES_CONSUMED_OUT: u32 = 3;
/// Canonical binding for the output ok flag.
pub const BINDING_OK_OUT: u32 = 4;

/// Maximum byte iterations inside the `'…'`. Covers up to four-byte
/// multi-char constants plus the longest single-byte escape.
pub const MAX_CONTENT_BYTES: u32 = 8;
