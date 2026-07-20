#[test]
fn ifdef_value_returns_zero_when_macro_is_undefined() {
    let source: &[u8] = b"#ifdef MISSING\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_IFDEF];
    let macro_names: &[u8] = b"FOOBAR";
    let macro_offsets = vec![0u32, 3, 6];

    let values = run_ifdef_value_real_gpu(
        &tok_starts,
        &tok_lens,
        &directive_kinds,
        source,
        macro_names,
        &macro_offsets,
    );
    assert_eq!(values.first().copied(), Some(0), "MISSING not defined → ifdef value 0");
}

#[test]
fn ifndef_value_inverts() {
    let source: &[u8] = b"#ifndef FOO\n";
    let tok_starts = vec![0u32];
    let tok_lens = vec![source.len() as u32];
    let directive_kinds = vec![TOK_PP_IFNDEF];
    let macro_names: &[u8] = b"FOO";
    let macro_offsets = vec![0u32, 3];

    let values = run_ifdef_value_real_gpu(
        &tok_starts,
        &tok_lens,
        &directive_kinds,
        source,
        macro_names,
        &macro_offsets,
    );
    assert_eq!(
        values.first().copied(),
        Some(0),
        "FOO is defined → ifndef inverts to 0"
    );
}

#[test]
fn include_parse_middle_row_real_gpu() {
    let source: &[u8] =
        b"#define X 1\n#include \"foo.h\"\n#undef Y\n";
    let tok_starts = vec![0u32, 12, 29];
    let tok_lens = vec![12u32, 17, 9];
    let directive_kinds = vec![TOK_PP_DEFINE, TOK_PP_INCLUDE, TOK_PP_UNDEF];

    let (path_s, path_l, is_system) =
        run_include_parse_real_gpu(&tok_starts, &tok_lens, &directive_kinds, source);
    assert_eq!(path_s[1], 22, "middle-row include start: {path_s:?}");
    assert_eq!(path_l[1], 5, "middle-row include len: {path_l:?}");
    assert_eq!(is_system[1], 0, "middle-row include system flag: {is_system:?}");
}

#[test]
fn fused_define_include_undef_real_gpu() {
    // Single dispatch through the 3-way fused (define + include +
    // undef) program against a row that exercises ALL THREE
    // directive kinds at different token positions. Validates
    // fuse_programs lowers cleanly to wgpu/naga and that the merged
    // buffer order matches what gpu_extract_directive_payloads
    // assumes.
    let source: &[u8] =
        b"#define X 1\n#include \"foo.h\"\n#undef Y\n";
    // Row offsets into source:
    //   #define X 1  -> 0..12
    //   #include "foo.h" -> 12..29
    //   #undef Y -> 29..38
    let tok_starts = vec![0u32, 12, 29];
    let tok_lens = vec![12u32, 17, 9];
    let directive_kinds = vec![TOK_PP_DEFINE, TOK_PP_INCLUDE, TOK_PP_UNDEF];

    let dp = gpu_define_parse(3, source.len() as u32);
    let ip = gpu_include_parse(3, source.len() as u32);
    let up = gpu_undef_parse(3, source.len() as u32);
    let fused = fuse_programs(&[dp, ip, up]).expect("fuse");

    let padded_src_len = source.len().div_ceil(4) * 4;
    let mut src = source.to_vec();
    src.resize(padded_src_len.max(4), 0);
    let zero = vec![0u8; 3 * 4];
    let inputs = vec![
        pack_u32_le(&tok_starts),
        pack_u32_le(&tok_lens),
        pack_u32_le(&directive_kinds),
        src,
        zero.clone(), // name_start_out (define)
        zero.clone(), // name_len_out (define)
        zero.clone(), // args_start_out (define)
        zero.clone(), // args_len_out (define)
        zero.clone(), // body_start_out (define)
        zero.clone(), // body_len_out (define)
        zero.clone(), // is_function_like_out (define)
        zero.clone(), // path_start_out (include)
        zero.clone(), // path_len_out (include)
        zero.clone(), // is_system_out (include)
        zero.clone(), // undef_name_start_out (undef)
        zero,         // undef_name_len_out (undef)
    ];
    let ref_inputs: Vec<vyre_reference::value::Value> =
        inputs.iter().cloned().map(vyre_reference::value::Value::from).collect();
    let ref_outs = vyre_reference::reference_eval(&fused, &ref_inputs)
        .expect("reference fused parse");
    let ref_all_outs: Vec<Vec<u32>> =
        ref_outs.iter().map(|out| unpack_u32(&out.to_bytes())).collect();
    assert_eq!(
        ref_all_outs[7][1],
        22,
        "reference fused include path_start must point at foo.h: ref_all_outs={ref_all_outs:?}"
    );
    assert_eq!(
        ref_all_outs[8][1],
        5,
        "reference fused include path_len must be 5: ref_all_outs={ref_all_outs:?}"
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let outs = backend
        .dispatch(&fused, &inputs, &DispatchConfig::default())
        .expect("dispatch fused parse");

    let name_s = unpack_u32(&outs[0]);
    let name_l = unpack_u32(&outs[1]);
    let path_s = unpack_u32(&outs[7]);
    let path_l = unpack_u32(&outs[8]);
    let undef_name_s = unpack_u32(&outs[10]);
    let undef_name_l = unpack_u32(&outs[11]);
    let all_outs: Vec<Vec<u32>> = outs.iter().map(|out| unpack_u32(out)).collect();

    // Define row: name = "X" at column 8 (after `#define `).
    let ns = name_s[0] as usize;
    let nl = name_l[0] as usize;
    assert_eq!(&source[ns..ns + nl], b"X", "define name span");
    // Include row: path = "foo.h" between the quotes.
    let ps = path_s[1] as usize;
    let pl = path_l[1] as usize;
    assert_eq!(
        &source[ps..ps + pl],
        b"foo.h",
        "include path span: path_s={path_s:?} path_l={path_l:?} all_outs={all_outs:?}"
    );
    // Undef row: name = "Y".
    let us = undef_name_s[2] as usize;
    let ul = undef_name_l[2] as usize;
    assert_eq!(&source[us..us + ul], b"Y", "undef name span");
}
