//! Tier 2.5 byte classifier  -  the canonical char-class primitive.
//!
//! Each invocation classifies one source byte by loading a host-supplied
//! 256-entry lookup table from the `table` buffer. The table stays in data
//! rather than code so alternate classifier sets can be swapped in without
//! rebuilding the crate.
//!
//! Tier 3 dialects call this builder and may register wrapper ops
//! with their own ids. This primitive keeps its own Tier 2.5 id so
//! op coverage and composition audits can distinguish the reusable
//! substrate from user-facing library wrappers.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// `\0`
pub const C_EOF: u32 = 0;
/// Space or tab.
pub const C_WS: u32 = 1;
/// `\n` or `\r`
pub const C_NEWLINE: u32 = 2;
/// `A-Z`, `a-z`, `_`
pub const C_ALPHA: u32 = 3;
/// `0-9`
pub const C_DIGIT: u32 = 4;
/// `(`
pub const C_OPEN_PAREN: u32 = 5;
/// `)`
pub const C_CLOSE_PAREN: u32 = 6;
/// `{`
pub const C_OPEN_BRACE: u32 = 7;
/// `}`
pub const C_CLOSE_BRACE: u32 = 8;
/// `;`
pub const C_SEMICOLON: u32 = 9;
/// `,`
pub const C_COMMA: u32 = 10;
/// `.`
pub const C_DOT: u32 = 11;
/// `*`
pub const C_STAR: u32 = 12;
/// `+`
pub const C_PLUS: u32 = 13;
/// `-`
pub const C_MINUS: u32 = 14;
/// `/`
pub const C_SLASH: u32 = 15;
/// `#`
pub const C_HASH: u32 = 16;
/// `'`
pub const C_QUOTE: u32 = 17;
/// `"`
pub const C_DQUOTE: u32 = 18;
/// `=`
pub const C_EQUALS: u32 = 19;
/// `<`
pub const C_LT: u32 = 20;
/// `>`
pub const C_GT: u32 = 21;
/// `!`
pub const C_BANG: u32 = 22;
/// `&`
pub const C_AMP: u32 = 23;
/// `|`
pub const C_PIPE: u32 = 24;
/// `^`
pub const C_CARET: u32 = 25;
/// `~`
pub const C_TILDE: u32 = 26;
/// `%`
pub const C_PERCENT: u32 = 27;
/// `\`
pub const C_BACKSLASH: u32 = 28;
/// `[`
pub const C_OPEN_BRACKET: u32 = 29;
/// `]`
pub const C_CLOSE_BRACKET: u32 = 30;
/// Anything else.
pub const C_OTHER: u32 = 31;

/// Stable op id for the registered Tier 2.5 primitive.
pub const CHAR_CLASS_OP_ID: &str = "vyre-primitives::text::char_class";
/// Byte-lane workgroup used by the table-driven classifier.
pub const CHAR_CLASS_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for classifying `n` byte lanes.
#[must_use]
pub const fn char_class_dispatch_grid(n: u32) -> [u32; 3] {
    let blocks = n.div_ceil(CHAR_CLASS_WORKGROUP_SIZE[0]);
    if blocks == 0 {
        [1, 1, 1]
    } else {
        [blocks, 1, 1]
    }
}

/// Build the default ASCII byte-classification table.
#[must_use]
pub fn build_char_class_table() -> [u32; 256] {
    let mut table = [C_OTHER; 256];

    table[0] = C_EOF;
    table[usize::from(b' ')] = C_WS;
    table[usize::from(b'\t')] = C_WS;
    table[usize::from(b'\n')] = C_NEWLINE;
    table[usize::from(b'\r')] = C_NEWLINE;
    table[usize::from(b'(')] = C_OPEN_PAREN;
    table[usize::from(b')')] = C_CLOSE_PAREN;
    table[usize::from(b'{')] = C_OPEN_BRACE;
    table[usize::from(b'}')] = C_CLOSE_BRACE;
    table[usize::from(b';')] = C_SEMICOLON;
    table[usize::from(b',')] = C_COMMA;
    table[usize::from(b'.')] = C_DOT;
    table[usize::from(b'*')] = C_STAR;
    table[usize::from(b'+')] = C_PLUS;
    table[usize::from(b'-')] = C_MINUS;
    table[usize::from(b'/')] = C_SLASH;
    table[usize::from(b'#')] = C_HASH;
    table[usize::from(b'\'')] = C_QUOTE;
    table[usize::from(b'"')] = C_DQUOTE;
    table[usize::from(b'=')] = C_EQUALS;
    table[usize::from(b'<')] = C_LT;
    table[usize::from(b'>')] = C_GT;
    table[usize::from(b'!')] = C_BANG;
    table[usize::from(b'&')] = C_AMP;
    table[usize::from(b'|')] = C_PIPE;
    table[usize::from(b'^')] = C_CARET;
    table[usize::from(b'~')] = C_TILDE;
    table[usize::from(b'%')] = C_PERCENT;
    table[usize::from(b'\\')] = C_BACKSLASH;
    table[usize::from(b'[')] = C_OPEN_BRACKET;
    table[usize::from(b']')] = C_CLOSE_BRACKET;
    table[usize::from(b'_')] = C_ALPHA;

    for byte in b'0'..=b'9' {
        table[usize::from(byte)] = C_DIGIT;
    }
    for byte in b'A'..=b'Z' {
        table[usize::from(byte)] = C_ALPHA;
    }
    for byte in b'a'..=b'z' {
        table[usize::from(byte)] = C_ALPHA;
    }

    table
}

fn char_class_body(source: &str, classified: &str, n: u32) -> Vec<Node> {
    let load_byte = |index: Expr| {
        Expr::bitand(
            Expr::cast(DataType::U32, Expr::load(source, index)),
            Expr::u32(0xFF),
        )
    };

    vec![Node::Region {
        generator: vyre_foundation::ir::model::expr::Ident::from(CHAR_CLASS_OP_ID),
        source_region: None,
        body: std::sync::Arc::new(vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(n)),
                vec![Node::store(
                    classified,
                    Expr::var("idx"),
                    Expr::load("table", load_byte(Expr::var("idx"))),
                )],
            ),
        ]),
    }]
}

/// Build a Program that writes one character-class code per source byte.
///
/// This compatibility entry point expects one `DataType::U32` element per
/// source byte and reads the low byte of each word. Use [`char_class_u8`] when
/// the source is packed as one byte per element. `table` is loaded from a
/// host-provided buffer named `"table"`.
#[must_use]
pub fn char_class(source: &str, classified: &str, n: u32) -> Program {
    char_class_with_source_type(source, classified, n, DataType::U32)
}

/// Build a Program that writes one character-class code per packed source byte.
///
/// It emits the same class stream as [`char_class`] while cutting source input
/// bandwidth from four bytes per logical byte to one.
#[must_use]
pub fn char_class_u8(source: &str, classified: &str, n: u32) -> Program {
    char_class_with_source_type(source, classified, n, DataType::U8)
}

fn char_class_with_source_type(
    source: &str,
    classified: &str,
    n: u32,
    source_type: DataType,
) -> Program {
    let output_byte_len = usize::try_from(n).unwrap_or(usize::MAX).saturating_mul(4);
    Program::wrapped(
        vec![
            BufferDecl::storage(source, 0, BufferAccess::ReadOnly, source_type).with_count(n),
            BufferDecl::storage("table", 1, BufferAccess::ReadOnly, DataType::U32).with_count(256),
            BufferDecl::output(classified, 2, DataType::U32)
                .with_count(n.max(1))
                .with_output_byte_range(0..output_byte_len),
        ],
        CHAR_CLASS_WORKGROUP_SIZE,
        char_class_body(source, classified, n),
    )
}

/// Reference oracle: classify each source byte through the lookup table.
///
/// Pure function, exposed for fixture generation + harness oracles.
#[must_use]
#[cfg(any(test, feature = "cpu-parity", feature = "text"))]
pub fn reference_char_class(source: &[u8], table: &[u32; 256]) -> Vec<u32> {
    source
        .iter()
        .map(|byte| table[usize::from(*byte)])
        .collect()
}

/// Pack a `[u32]` slice into the LE-byte layout the harness uses.
#[must_use]
pub fn pack_u32(words: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(words)
}

/// Pack a `[u8]` source slice into the per-element u32 layout the GPU
/// kernel expects (each byte in the low 8 bits of a u32 lane).
#[must_use]
pub fn pack_bytes_as_u32(bytes: &[u8]) -> Vec<u8> {
    crate::wire::pack_bytes_as_u32_slice(bytes)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        CHAR_CLASS_OP_ID,
        || char_class("source", "classified", 3),
        Some(|| {
            let table = build_char_class_table();
            vec![vec![
                pack_bytes_as_u32(b"A1 "),
                pack_u32(&table),
                vec![0u8; 3 * 4],
            ]]
        }),
        Some(|| {
            vec![vec![pack_u32(&[C_ALPHA, C_DIGIT, C_WS])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_classifies_ascii_letter_as_alpha() {
        let table = build_char_class_table();
        assert_eq!(table[usize::from(b'A')], C_ALPHA);
        assert_eq!(table[usize::from(b'z')], C_ALPHA);
        assert_eq!(table[usize::from(b'_')], C_ALPHA);
    }

    #[test]
    fn table_classifies_digits() {
        let table = build_char_class_table();
        for byte in b'0'..=b'9' {
            assert_eq!(table[usize::from(byte)], C_DIGIT);
        }
    }

    #[test]
    fn reference_walks_table() {
        let table = build_char_class_table();
        assert_eq!(
            reference_char_class(b"A1 ", &table),
            vec![C_ALPHA, C_DIGIT, C_WS]
        );
    }

    #[test]
    fn reference_covers_every_byte_value() {
        let table = build_char_class_table();
        let source: Vec<u8> = (0u8..=255).collect();
        assert_eq!(reference_char_class(&source, &table), table.to_vec());
    }

    #[test]
    fn program_uses_block_sized_workgroup() {
        let program = char_class("source", "classified", 513);
        assert_eq!(program.workgroup_size(), CHAR_CLASS_WORKGROUP_SIZE);
    }

    #[test]
    fn packed_u8_program_declares_one_source_byte_per_element() {
        let program = char_class_u8("source", "classified", 513);
        let source = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "source")
            .expect("Fix: packed-u8 char_class source buffer must be declared");
        let classified = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "classified")
            .expect("Fix: char_class output buffer must be declared");

        assert_eq!(source.element(), DataType::U8);
        assert_eq!(source.count(), 513);
        assert_eq!(classified.element(), DataType::U32);
        assert_eq!(classified.count(), 513);
        assert_eq!(classified.output_byte_range(), Some(0..513 * 4));
        assert_eq!(program.workgroup_size(), CHAR_CLASS_WORKGROUP_SIZE);
    }

    #[test]
    fn empty_program_declares_empty_output_range() {
        let program = char_class_u8("source", "classified", 0);
        let classified = program
            .buffers()
            .iter()
            .find(|buffer| buffer.name() == "classified")
            .expect("Fix: char_class output buffer must be declared");

        assert_eq!(classified.count(), 1);
        assert_eq!(classified.output_byte_range(), Some(0..0));
    }

    #[test]
    fn dispatch_grid_packs_byte_lanes_into_blocks() {
        assert_eq!(char_class_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(char_class_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(char_class_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(char_class_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(char_class_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn primitive_id_names_the_primitive_tier() {
        assert_eq!(CHAR_CLASS_OP_ID, "vyre-primitives::text::char_class");
    }

    #[test]
    fn char_class_table_builder_uses_widening_indices() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/text/char_class.rs"
        ))
        .expect("Fix: char_class source must be readable");
        let production = src
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: meta-test scans production sources; update fixture path if module moved - production section must exist");
        assert!(
            !production.contains(" as usize"),
            "byte lookup-table indices must use usize::from so the primitive has no narrowing casts"
        );
        assert!(production.contains("usize::from(byte)"));
    }
}
