use super::*;

#[test]
fn table_line_splice_offset_map_is_monotonic() {
    let cases: &[&[u8]] = &[
        b"no splicing",
        b"line\\\ncontinuation",
        b"line\\\r\ncontinuation",
        b"a\\\nb\\\r\nc",
        b"\\\n",
        b"",
    ];
    for source in cases {
        let spliced = c_translation_phase_line_splice(source);
        assert_eq!(
            spliced.original_offsets.len(),
            spliced.bytes.len() + 1,
            "offset map length must be bytes + 1"
        );
        // Offsets must be non-decreasing.
        for w in spliced.original_offsets.windows(2) {
            assert!(w[0] <= w[1], "original_offsets must be monotonic");
        }
        // Final offset must equal source length.
        assert_eq!(
            spliced.original_offsets.last().copied().unwrap_or(0),
            source.len(),
            "final offset must equal source length"
        );
    }
}
