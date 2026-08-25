use crate::parsing::c::preprocess::gpu_pipeline::segments::append_active_segment;

pub(super) fn emit_active_token_range(
    source: &[u8],
    active_segment: &mut Vec<u8>,
    active_segment_start: &mut Option<usize>,
    last_emit_end: &mut usize,
    tok_start: usize,
    tok_end: usize,
) -> Result<(), String> {
    if tok_start > *last_emit_end {
        append_active_segment(
            active_segment,
            active_segment_start,
            source,
            *last_emit_end,
            tok_start,
            "inter-token emission",
        )?;
    }
    if tok_end <= *last_emit_end {
        return Ok(());
    }
    let emit_start = tok_start.max(*last_emit_end);
    append_active_segment(
        active_segment,
        active_segment_start,
        source,
        emit_start,
        tok_end,
        "token emission",
    )?;
    *last_emit_end = tok_end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_token_rows_emit_source_bytes_once() {
        let source = b"3.14;";
        let mut active_segment = Vec::new();
        let mut active_segment_start = None;
        let mut last_emit_end = 0;

        emit_active_token_range(
            source,
            &mut active_segment,
            &mut active_segment_start,
            &mut last_emit_end,
            0,
            4,
        )
        .expect("Fix: first token span is valid");
        emit_active_token_range(
            source,
            &mut active_segment,
            &mut active_segment_start,
            &mut last_emit_end,
            1,
            4,
        )
        .expect("Fix: overlapping token span is valid");
        emit_active_token_range(
            source,
            &mut active_segment,
            &mut active_segment_start,
            &mut last_emit_end,
            4,
            5,
        )
        .expect("Fix: following token span is valid");

        assert_eq!(active_segment_start, Some(0));
        assert_eq!(active_segment, source);
        assert_eq!(last_emit_end, source.len());
    }
}
