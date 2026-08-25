use super::*;
use smallvec::SmallVec;

pub(super) fn take_resident_blob(
    pairs: &mut SmallVec<[(&str, ResidentBlob); 8]>,
    name: &str,
    label: &str,
) -> Result<ResidentBlob, String> {
    let index = pairs
        .iter()
        .position(|(candidate, _)| *candidate == name)
        .ok_or_else(|| format!("{label} resident sparse lexer missing output `{name}`"))?;
    Ok(pairs.swap_remove(index).1)
}

pub(super) fn collect_compact_lexer_output_drain(
    program: &Program,
    outputs: &mut Vec<Vec<u8>>,
    label: &str,
    stage: &str,
) -> Result<SparseLexerMegakernelOutput, String> {
    let returned_buffers = returned_buffer_names(program, outputs.len(), label, stage)?;
    collect_compact_lexer_output_named(returned_buffers, outputs.drain(..), label, stage)
}

pub(super) fn collect_compact_lexer_output_named<I, S, O>(
    returned_buffers: I,
    outputs: O,
    label: &str,
    stage: &str,
) -> Result<SparseLexerMegakernelOutput, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    O: IntoIterator<Item = Vec<u8>>,
{
    let mut types = None;
    let mut starts = None;
    let mut lens = None;
    let mut counts = None;
    for (buffer_name, value) in returned_buffers.into_iter().zip(outputs.into_iter()) {
        match buffer_name.as_ref() {
            name if compact_lexer_output_name(name) => match name {
                "out_tok_types" => types = Some(value),
                "out_tok_starts" => starts = Some(value),
                "out_tok_lens" => lens = Some(value),
                "out_counts" => counts = Some(value),
                _ => {}
            },
            _ => {}
        }
    }

    let types = types.ok_or_else(|| format!("{label} sparse lexer {stage} missing types"))?;
    let starts = starts.ok_or_else(|| format!("{label} sparse lexer {stage} missing starts"))?;
    let lens = lens.ok_or_else(|| format!("{label} sparse lexer {stage} missing lens"))?;
    let counts = counts.ok_or_else(|| format!("{label} sparse lexer {stage} missing counts"))?;
    let n_tokens = read_u32_at(&counts, 0)
        .map_err(|e| format!("{label} sparse lexer {stage} token count: {e}"))?;
    Ok(SparseLexerMegakernelOutput {
        types,
        starts,
        lens,
        counts,
        n_tokens,
    })
}

pub(super) fn collect_resident_compact_lexer_output_exact_readback<I, S>(
    backend: &dyn VyreBackend,
    resident_outputs: Vec<ResidentBlob>,
    returned_buffers: I,
    outputs: &mut Vec<Vec<u8>>,
    count_readback: &mut Vec<u8>,
    label: &str,
    stage: &str,
) -> Result<SparseLexerMegakernelOutput, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let names = returned_buffers
        .into_iter()
        .map(|name| name.as_ref().to_string())
        .collect::<SmallVec<[String; 8]>>();
    if names.len() != resident_outputs.len() {
        return Err(format!(
            "{label} sparse lexer {stage} returned {} resident compact buffers, expected {} named outputs. Fix: align resident compact output-set semantics before exact readback.",
            resident_outputs.len(),
            names.len()
        ));
    }
    let mut pairs = names
        .into_iter()
        .zip(resident_outputs)
        .collect::<SmallVec<[(String, ResidentBlob); 8]>>();
    let types = take_owned_resident_blob(&mut pairs, "out_tok_types", label, stage)?;
    let starts = take_owned_resident_blob(&mut pairs, "out_tok_starts", label, stage)?;
    let lens = take_owned_resident_blob(&mut pairs, "out_tok_lens", label, stage)?;
    let counts = take_owned_resident_blob(&mut pairs, "out_counts", label, stage)?;

    backend
        .download_resident_range_into(&counts.resource, 0, 4, count_readback)
        .map_err(|error| format!("{label} sparse lexer {stage} count readback failed: {error}"))?;
    let n_tokens = read_u32_at(count_readback, 0)
        .map_err(|error| format!("{label} sparse lexer {stage} token count: {error}"))?;
    let token_bytes = compact_token_column_byte_len(n_tokens, label, stage)?;

    outputs.clear();
    outputs.resize_with(4, Vec::new);
    let (types_slot, rest) = outputs.split_at_mut(1);
    let (starts_slot, rest) = rest.split_at_mut(1);
    let (lens_slot, counts_slot) = rest.split_at_mut(1);
    let ranges = [
        (&types.resource, 0, token_bytes),
        (&starts.resource, 0, token_bytes),
        (&lens.resource, 0, token_bytes),
    ];
    let mut dense_outputs = [&mut types_slot[0], &mut starts_slot[0], &mut lens_slot[0]];
    backend
        .download_resident_ranges_into(&ranges, &mut dense_outputs)
        .map_err(|error| {
            format!("{label} sparse lexer {stage} dense token readback failed: {error}")
        })?;
    counts_slot[0].clear();
    counts_slot[0].extend_from_slice(count_readback);

    let mut drained = outputs.drain(..);
    Ok(SparseLexerMegakernelOutput {
        types: drained
            .next()
            .ok_or_else(|| format!("{label} sparse lexer {stage} missing exact types"))?,
        starts: drained
            .next()
            .ok_or_else(|| format!("{label} sparse lexer {stage} missing exact starts"))?,
        lens: drained
            .next()
            .ok_or_else(|| format!("{label} sparse lexer {stage} missing exact lens"))?,
        counts: drained
            .next()
            .ok_or_else(|| format!("{label} sparse lexer {stage} missing exact counts"))?,
        n_tokens,
    })
}

fn take_owned_resident_blob(
    pairs: &mut SmallVec<[(String, ResidentBlob); 8]>,
    name: &str,
    label: &str,
    stage: &str,
) -> Result<ResidentBlob, String> {
    let index = pairs
        .iter()
        .position(|(candidate, _)| candidate == name)
        .ok_or_else(|| format!("{label} sparse lexer {stage} missing resident output `{name}`"))?;
    Ok(pairs.swap_remove(index).1)
}

fn compact_token_column_byte_len(n_tokens: u32, label: &str, stage: &str) -> Result<usize, String> {
    usize::try_from(n_tokens)
        .ok()
        .and_then(|tokens| tokens.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "{label} sparse lexer {stage} token byte length overflowed for {n_tokens} token(s). Fix: shard the sparse lexer readback."
            )
        })
}

pub(super) fn resident_output_pairs<'a, const N: usize>(
    names: [&'a str; N],
    outputs: Vec<ResidentBlob>,
    label: &str,
    stage: &str,
) -> Result<SmallVec<[(&'a str, ResidentBlob); 8]>, String> {
    if outputs.len() != N {
        return Err(format!(
            "{label} sparse lexer {stage} returned {} resident buffers, expected exactly {N}. Fix: align resident output-set semantics with the sparse lexer stage contract.",
            outputs.len()
        ));
    }
    Ok(names.into_iter().zip(outputs).collect())
}

pub(super) fn compact_lexer_output_name(name: &str) -> bool {
    matches!(
        name,
        "out_tok_types" | "out_tok_starts" | "out_tok_lens" | "out_counts"
    )
}

pub(super) fn backend_returned_buffer(buffer: &BufferDecl) -> bool {
    buffer.is_output || matches!(buffer.access, BufferAccess::ReadWrite)
}

pub(super) fn returned_buffer_names(
    program: &Program,
    output_count: usize,
    label: &str,
    stage: &str,
) -> Result<Vec<String>, String> {
    let explicit = program
        .buffers()
        .iter()
        .filter(|buffer| buffer.is_output)
        .map(|buffer| buffer.name().to_string())
        .collect::<Vec<_>>();
    if output_count == explicit.len() {
        return Ok(explicit);
    }
    let compact = program
        .buffers()
        .iter()
        .filter(|buffer| compact_lexer_output_name(buffer.name()))
        .map(|buffer| buffer.name().to_string())
        .collect::<Vec<_>>();
    if output_count == compact.len() {
        return Ok(compact);
    }
    let readwrite = program
        .buffers()
        .iter()
        .filter(|buffer| backend_returned_buffer(buffer))
        .map(|buffer| buffer.name().to_string())
        .collect::<Vec<_>>();
    if output_count == readwrite.len() {
        return Ok(readwrite);
    }
    Err(format!(
        "{label} sparse lexer {stage} returned {output_count} buffers, expected either {} explicit outputs, {} compact outputs, or {} read-write/live outputs. Fix: align backend output-set semantics with the sparse lexer collector.",
        explicit.len(),
        compact.len(),
        readwrite.len()
    ))
}

pub(super) fn zero_readback_buffers(mut program: Program, names: &[&str]) -> Program {
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if names.iter().any(|name| *name == buffer.name()) {
            buffer.is_output = false;
            buffer.pipeline_live_out = false;
            buffer.output_byte_range = None;
            buffer.access = BufferAccess::ReadWrite;
        }
    }
    program
}

pub(super) fn mark_output_buffers(mut program: Program, names: &[&str]) -> Program {
    let mut result_marked = false;
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if names.iter().any(|name| *name == buffer.name()) {
            buffer.access = BufferAccess::ReadWrite;
            buffer.pipeline_live_out = true;
            if result_marked {
                buffer.is_output = false;
            } else {
                buffer.is_output = true;
                result_marked = true;
            }
        }
    }
    program
}

#[cfg(test)]
mod tests {
    use super::compact_token_column_byte_len;

    #[test]
    fn compact_token_column_byte_len_allows_zero_live_tokens() {
        assert_eq!(
            compact_token_column_byte_len(0, "test", "resident")
                .expect("Fix: zero live tokens should produce a zero-byte exact readback"),
            0
        );
    }

    #[test]
    fn compact_token_column_byte_len_converts_token_words_to_bytes() {
        assert_eq!(
            compact_token_column_byte_len(17, "test", "resident")
                .expect("Fix: token count should convert to dense u32 column bytes"),
            68
        );
    }
}
