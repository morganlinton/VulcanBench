//! Tier 2.5 line-index  -  write a per-byte line number into `lines[i]`.
//!
//! Every parser dialect that reports diagnostics needs line numbers.
//! This op is a GPU-native flag/scan pipeline. The first pass marks bytes
//! where the line number increments from the previous byte, and the reduce
//! substrate scans those marks directly into the line number for every byte
//! position.
//!
//! Carriage-return handling: `\r` alone (Mac classic), `\r\n` (Windows),
//! and bare `\n` (Unix) are all normalized. Newline bytes belong to the
//! line they terminate; the following byte starts the next line when one
//! exists. This matches `str::lines()` semantics for byte-counting purposes.
//!
//! Ranged use: `column_for_byte(idx)` is `idx - line_start_offset`.
//! This primitive deliberately publishes per-byte line numbers only;
//! dialects that need column offsets derive them from their own
//! line-start representation.

use std::sync::Arc;

use crate::reduce::multi_block_prefix_scan::{multi_block_prefix_scan_sum_u32, BLOCK_LANES};
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable op id for the registered Tier 3 wrapper.
pub const OP_ID: &str = "vyre-primitives::text::line_index";
const FLAG_OP_ID: &str = "vyre-primitives::text::line_index::line_start_flags";

/// Build a Program that writes `lines[i] = line_number_of(source[i])`.
///
/// Newline bytes belong to the line they terminate, so the generated
/// pipeline scans per-byte "line starts here" increment flags: byte 0 is
/// always line 0, a byte after `\n` starts the next line, and a byte after
/// lone `\r` starts the next line unless the current byte is the `\n` half
/// of `\r\n`.
///
/// This compatibility entry point expects one `DataType::U32` element per
/// source byte and reads the low byte of each word. Use [`line_index_u8`]
/// when the source is packed as one byte per element.
#[must_use]
pub fn line_index(source: &str, lines: &str, n: u32) -> Program {
    match try_line_index(source, lines, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, lines, DataType::U32, error),
    }
}

/// Build a line-index Program over a packed `DataType::U8` source buffer.
///
/// It emits the same per-byte line numbers as [`line_index`] while reducing
/// source input bandwidth from four bytes per logical byte to one.
#[must_use]
pub fn line_index_u8(source: &str, lines: &str, n: u32) -> Program {
    match try_line_index_u8(source, lines, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, lines, DataType::U32, error),
    }
}

fn try_line_index(source: &str, lines: &str, n: u32) -> Result<Program, String> {
    try_line_index_with_source_type(source, lines, n, DataType::U32)
}

fn try_line_index_u8(source: &str, lines: &str, n: u32) -> Result<Program, String> {
    try_line_index_with_source_type(source, lines, n, DataType::U8)
}

fn try_line_index_with_source_type(
    source: &str,
    lines: &str,
    n: u32,
    source_type: DataType,
) -> Result<Program, String> {
    if n == 0 {
        return Ok(empty_line_index_program(source, lines, source_type));
    }

    let flags = format!("__{lines}_line_start_flags");

    let flag_pass = line_start_flags_program(source, &flags, n, source_type)?;
    let scan_pass = multi_block_prefix_scan_sum_u32(&flags, lines, n);
    if scan_pass.stats().trap() {
        return Err(format!(
            "line_index n={n} could not build its prefix-scan pass. Fix: shard the source before line indexing or repair reduce::multi_block_prefix_scan sizing."
        ));
    }

    vyre_foundation::execution_plan::fusion::fuse_programs(&[flag_pass, scan_pass])
        .map(|program| demote_intermediate_outputs(program, lines))
        .map_err(|error| {
            format!(
                "line_index fusion failed for n={n}: {error}. Fix: repair flag/scan fusion instead of falling back to a serial lane-0 loop."
            )
        })
}

fn empty_line_index_program(source: &str, lines: &str, source_type: DataType) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(source, 0, BufferAccess::ReadOnly, source_type).with_count(0),
            BufferDecl::output(lines, 1, DataType::U32)
                .with_count(0)
                .with_output_byte_range(0..0),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(Vec::new()),
        }],
    )
}

fn output_byte_range(words: u32, context: &str) -> Result<usize, String> {
    usize::try_from(words)
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "{context} words={words} overflows output byte range. Fix: shard the source before GPU line indexing."
            )
        })
}

fn line_start_flags_program(
    source: &str,
    flags: &str,
    n: u32,
    source_type: DataType,
) -> Result<Program, String> {
    let t = Expr::InvocationId { axis: 0 };
    let prev_idx = Expr::add(t.clone(), Expr::u32(u32::MAX));
    let output_bytes = output_byte_range(n, "line_index line-start-flags output")?;
    let load_byte = |index: Expr| {
        Expr::bitand(
            Expr::cast(DataType::U32, Expr::load(source, index)),
            Expr::u32(0xFF),
        )
    };

    let lane_body = vec![
        Node::let_bind("byte", load_byte(t.clone())),
        Node::let_bind("prev_byte", Expr::u32(0)),
        Node::if_then(
            Expr::lt(Expr::u32(0), t.clone()),
            vec![Node::assign("prev_byte", load_byte(prev_idx))],
        ),
        Node::let_bind("flag", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::lt(Expr::u32(0), t.clone()),
                Expr::eq(Expr::var("prev_byte"), Expr::u32(0x0A)),
            ),
            vec![Node::assign("flag", Expr::u32(1))],
        ),
        Node::if_then(
            Expr::and(
                Expr::lt(Expr::u32(0), t.clone()),
                Expr::and(
                    Expr::eq(Expr::var("prev_byte"), Expr::u32(0x0D)),
                    Expr::ne(Expr::var("byte"), Expr::u32(0x0A)),
                ),
            ),
            vec![Node::assign("flag", Expr::u32(1))],
        ),
        Node::store(flags, t.clone(), Expr::var("flag")),
    ];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(source, 0, BufferAccess::ReadOnly, source_type).with_count(n),
            BufferDecl::storage(flags, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n)
                .with_pipeline_live_out(true)
                .with_output_byte_range(0..output_bytes),
        ],
        [BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FLAG_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(Expr::lt(t, Expr::u32(n)), lane_body)]),
        }],
    ))
}

fn demote_intermediate_outputs(program: Program, final_output: &str) -> Program {
    let buffers = program
        .buffers()
        .iter()
        .map(|buffer| {
            let mut buffer = buffer.clone();
            if buffer.name() != final_output && buffer.is_output() {
                buffer.is_output = false;
                buffer.pipeline_live_out = true;
            }
            buffer
        })
        .collect();
    program.with_rewritten_buffers(buffers)
}

/// Reference oracle: same line-counting semantics as the GPU kernel.
#[must_use]
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub fn reference_line_index(source: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(source.len());
    let mut line: u32 = 0;
    let mut prev_was_cr = false;
    for &byte in source {
        // Lone `\r` (not followed by `\n`) means the current byte
        // belongs to the next line  -  increment BEFORE recording this
        // byte's line number.
        if prev_was_cr && byte != b'\n' {
            line += 1;
        }
        out.push(line);
        if byte == b'\n' {
            line += 1;
            prev_was_cr = false;
        } else {
            prev_was_cr = byte == b'\r';
        }
    }
    out
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || line_index("source", "lines", 5),
        Some(|| {
            vec![vec![
                vec![0x61, 0x00, 0x00, 0x00, 0x62, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x63, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00],
            ]]
        }),
        Some(|| {
            vec![vec![
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
            ]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_no_newlines() {
        assert_eq!(reference_line_index(b"Hello"), vec![0; 5]);
    }

    #[test]
    fn reference_unix_lf() {
        // "ab\ncd" → lines [0, 0, 0, 1, 1]
        assert_eq!(reference_line_index(b"ab\ncd"), vec![0, 0, 0, 1, 1]);
    }

    #[test]
    fn reference_windows_crlf() {
        // "ab\r\ncd" → lines [0, 0, 0, 0, 1, 1]
        assert_eq!(reference_line_index(b"ab\r\ncd"), vec![0, 0, 0, 0, 1, 1]);
    }

    #[test]
    fn reference_mac_classic_cr() {
        // "ab\rcd" → lines [0, 0, 0, 1, 1]
        assert_eq!(reference_line_index(b"ab\rcd"), vec![0, 0, 0, 1, 1]);
    }

    #[test]
    fn reference_multiple_newlines() {
        // "a\n\nb" → lines [0, 0, 1, 2]
        assert_eq!(reference_line_index(b"a\n\nb"), vec![0, 0, 1, 2]);
    }

    #[test]
    fn reference_trailing_lone_cr_does_not_increment_after_eof() {
        // "ab\r" → lines [0, 0, 0]; we don't see a follow-up byte.
        assert_eq!(reference_line_index(b"ab\r"), vec![0, 0, 0]);
    }

    #[test]
    fn builder_uses_parallel_scan_pipeline() {
        let program = line_index("source", "lines", BLOCK_LANES + 17);
        assert_eq!(program.workgroup_size(), [BLOCK_LANES, 1, 1]);
        assert!(program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "__lines_line_start_flags"
                && buffer.is_pipeline_live_out()));
        assert!(!program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "__lines_line_break_prefix"));
        assert_eq!(
            program
                .buffers()
                .iter()
                .filter(|buffer| buffer.is_output())
                .count(),
            1
        );
    }
}
