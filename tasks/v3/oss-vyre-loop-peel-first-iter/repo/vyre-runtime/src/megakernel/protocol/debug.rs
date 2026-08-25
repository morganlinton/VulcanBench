/// Atomic write cursor: the next word to be written by PRINTF.
pub const CURSOR_WORD: u32 = 0;
/// First record word.
pub const RECORDS_BASE: u32 = 1;
/// Number of u32 words per PRINTF record.
pub const RECORD_WORDS: u32 = 4;
/// Record capacity compiled into the default megakernel program.
pub const RECORD_CAPACITY: u32 = 64;
/// Total u32 words compiled into the default debug-log buffer.
pub const BUFFER_WORDS: u32 = RECORDS_BASE + RECORD_CAPACITY * RECORD_WORDS;
