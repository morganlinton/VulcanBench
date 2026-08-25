use vyre_libs::parsing::c::lex::tokens::{TOK_PP_IF, TOK_PREPROC};
use vyre_libs::parsing::c::parse::gnu_builtins::gpu_builtin_hash_table_words;
use vyre_libs::parsing::c::preprocess::gpu_directive_metadata::gpu_directive_metadata;
use vyre_libs::parsing::c::preprocess::gpu_if_expression::gpu_if_expression;
use vyre_libs::parsing::c::preprocess::reference_c_preprocessor_directive_metadata;
use vyre_libs::scan::dispatch_io::pack_u32_slice as pack_u32_le;
use vyre_reference::value::Value;

fn pack_macro_values_with_builtin_hashes(values: &[u32]) -> Vec<u8> {
    let mut words = Vec::with_capacity(values.len() + gpu_builtin_hash_table_words().len());
    words.extend_from_slice(&gpu_builtin_hash_table_words());
    words.extend_from_slice(values);
    pack_u32_le(&words)
}

fn unpack_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn build_token_stream(source: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut tok_types = Vec::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut i = 0usize;
    let mut at_line_start = true;
    while i < source.len() {
        if at_line_start {
            let mut j = i;
            while j < source.len() && matches!(source[j], b' ' | b'\t') {
                j += 1;
            }
            if j < source.len() && source[j] == b'#' {
                let row_start = i;
                let mut row_end = j;
                while row_end < source.len() {
                    if source[row_end] == b'\n' {
                        if row_end > row_start && source[row_end - 1] == b'\\' {
                            row_end += 1;
                            continue;
                        }
                        break;
                    }
                    if source[row_end] == b'\r' {
                        if row_end > row_start && source[row_end - 1] == b'\\' {
                            row_end += 1;
                            if row_end < source.len() && source[row_end] == b'\n' {
                                row_end += 1;
                            }
                            continue;
                        }
                        break;
                    }
                    row_end += 1;
                }
                tok_types.push(TOK_PREPROC);
                tok_starts.push(row_start as u32);
                tok_lens.push((row_end - row_start) as u32);
                i = row_end;
                at_line_start = true;
                continue;
            }
        }
        if source[i] == b'\n' || source[i] == b'\r' {
            at_line_start = true;
            i += 1;
            continue;
        }
        tok_types.push(0);
        tok_starts.push(i as u32);
        tok_lens.push(1);
        i += 1;
        at_line_start = false;
    }
    (tok_types, tok_starts, tok_lens)
}

fn pack_defined_macros(names: &[&[u8]]) -> (Vec<u8>, Vec<u32>) {
    let mut packed = Vec::new();
    let mut offsets = Vec::with_capacity(names.len() + 1);
    offsets.push(0u32);
    for name in names {
        packed.extend_from_slice(name);
        offsets.push(packed.len() as u32);
    }
    (packed, offsets)
}

fn pack_defined_macro_values(names: &[&[u8]]) -> Vec<u8> {
    let count = names.len().max(1);
    let values = vec![1u32; count];
    pack_u32_le(&values)
}

pub(crate) fn run_full_pipeline(source: &[u8], defined_macros: &[&[u8]]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    let n = tok_types.len();
    let n_padded = n.max(1);
    let src_padded = (source.len().div_ceil(4) * 4).max(4);

    let mut tt = pack_u32_le(&tok_types);
    tt.resize(n_padded * 4, 0);
    let mut ts = pack_u32_le(&tok_starts);
    ts.resize(n_padded * 4, 0);
    let mut tl = pack_u32_le(&tok_lens);
    tl.resize(n_padded * 4, 0);
    let mut src = source.to_vec();
    src.resize(src_padded, 0);
    let dk_init = vec![0u8; n_padded * 4];
    let dv_init_a = vec![0u8; n_padded * 4];

    let prog_a = gpu_directive_metadata(n as u32, source.len() as u32);
    let outputs_a = vyre_reference::reference_eval(
        &prog_a,
        &[
            Value::from(tt),
            Value::from(ts.clone()),
            Value::from(tl.clone()),
            Value::from(src.clone()),
            Value::from(dk_init),
            Value::from(dv_init_a),
        ],
    )
    .expect("17a kernel eval");
    let mut directive_kinds_bytes = outputs_a[0].to_bytes().to_vec();
    directive_kinds_bytes.resize(n_padded * 4, 0);

    let (macro_names, macro_offsets_words) = pack_defined_macros(defined_macros);
    let mut macro_names_padded = macro_names.clone();
    let macro_names_pad_len = (macro_names.len().div_ceil(4) * 4).max(4);
    macro_names_padded.resize(macro_names_pad_len, 0);
    let macro_offsets_bytes = pack_u32_le(&macro_offsets_words);
    let macro_values_words = unpack_u32(&pack_defined_macro_values(defined_macros));
    let macro_values_bytes = pack_macro_values_with_builtin_hashes(&macro_values_words);

    let dv_init_b = vec![0u8; n_padded * 4];
    let prog_b = gpu_if_expression(n as u32, source.len() as u32);
    let outputs_b = vyre_reference::reference_eval(
        &prog_b,
        &[
            Value::from(ts),
            Value::from(tl),
            Value::from(directive_kinds_bytes.clone()),
            Value::from(src),
            Value::from(macro_names_padded),
            Value::from(macro_offsets_bytes),
            Value::from(macro_values_bytes),
            Value::from(dv_init_b),
        ],
    )
    .expect("17b.4 kernel eval");

    let mut kinds = unpack_u32(&directive_kinds_bytes);
    kinds.truncate(n);
    let mut values = unpack_u32(&outputs_b[0].to_bytes());
    values.truncate(n);
    (kinds, values)
}

fn cpu_kinds_and_values(source: &[u8], defined_macros: &[&[u8]]) -> (Vec<u32>, Vec<u32>) {
    let (tok_types, tok_starts, tok_lens) = build_token_stream(source);
    reference_c_preprocessor_directive_metadata(
        &tok_types,
        &tok_starts,
        &tok_lens,
        source,
        defined_macros,
    )
    .expect("Reference oracle eval")
}

pub(crate) fn run_if_expression_with_macro_value(source: &[u8], name: &[u8], value: u32) -> u32 {
    let mut src = source.to_vec();
    src.resize((source.len().div_ceil(4) * 4).max(4), 0);
    let (mut macro_names, macro_offsets) = pack_defined_macros(&[name]);
    macro_names.resize((macro_names.len().div_ceil(4) * 4).max(4), 0);
    let prog = gpu_if_expression(1, 0);
    let outputs = vyre_reference::reference_eval(
        &prog,
        &[
            Value::from(pack_u32_le(&[0])),
            Value::from(pack_u32_le(&[source.len() as u32])),
            Value::from(pack_u32_le(&[TOK_PP_IF])),
            Value::from(src),
            Value::from(macro_names),
            Value::from(pack_u32_le(&macro_offsets)),
            Value::from(pack_macro_values_with_builtin_hashes(&[value])),
            Value::from(pack_u32_le(&[0])),
        ],
    )
    .expect("gpu_if_expression macro-value contract eval");
    unpack_u32(&outputs[0].to_bytes())[0]
}

fn keep_only_if_elif(kinds: &[u32], values: &[u32]) -> Vec<u32> {
    use vyre_libs::parsing::c::lex::tokens::{TOK_PP_ELIF, TOK_PP_IF};
    kinds
        .iter()
        .zip(values)
        .map(|(k, v)| {
            if *k == TOK_PP_IF || *k == TOK_PP_ELIF {
                *v
            } else {
                0
            }
        })
        .collect()
}

pub(crate) fn assert_gpu_matches_cpu(source: &[u8], defined: &[&[u8]]) {
    let (kinds, gpu_values) = run_full_pipeline(source, defined);
    let (cpu_kinds, cpu_values) = cpu_kinds_and_values(source, defined);
    assert_eq!(
        kinds,
        cpu_kinds,
        "directive_kinds mismatch on {:?}",
        std::str::from_utf8(source)
    );
    assert_eq!(
        gpu_values,
        keep_only_if_elif(&cpu_kinds, &cpu_values),
        "directive_values mismatch on {:?}",
        std::str::from_utf8(source),
    );
}
