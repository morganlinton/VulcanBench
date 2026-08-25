#[test]
fn opaque_malformed_payload_decoder_survives() {
    let malformed = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Opaque(Arc::new(TestNodeExt {
            payload: malformed,
        }))],
    );
    let wire = program.to_wire().unwrap();
    let result = std::panic::catch_unwind(|| Program::from_wire(&wire));
    assert_decode_fails(result, "malformed opaque payload");
}

// ---------------------------------------------------------------------------
// 5. Text serialization adversarial tests
// ---------------------------------------------------------------------------

#[test]
fn text_format_special_characters_in_buffer_names() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out_🚀_ñ\\t\r\n", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let text = program.to_text().expect("must encode with special chars in buffer name");
    let decoded = Program::from_text(&text).expect("must decode with special chars in buffer name");
    assert_eq!(decoded, program);
}

#[test]
fn text_format_rejects_unclosed_braces_body() {
    let input = format!(
        "{}\nwire_bytes 10\n{{\n  ;\n}}\n",
        vyre_foundation::serial::text::TEXT_FORMAT_HEADER
    );
    let err = Program::from_text(&input).unwrap_err();
    let msg = err.message();
    assert!(
        msg.contains("Fix:") || msg.contains("hex") || msg.contains("InvalidHexCharacter"),
        "expected hex-related error, got: {msg}"
    );
}

#[test]
fn text_format_rejects_missing_semicolons_body() {
    let input = format!(
        "{}\nwire_bytes 2\naa bb;\n",
        vyre_foundation::serial::text::TEXT_FORMAT_HEADER
    );
    let err = Program::from_text(&input).unwrap_err();
    let msg = err.message();
    assert!(
        msg.contains("Fix:")
            || msg.contains("hex")
            || msg.contains("length")
            || msg.contains("InvalidHexCharacter"),
        "expected structured error, got: {msg}"
    );
}

#[test]
fn text_format_rejects_odd_hex_line_length() {
    let input = format!(
        "{}\nwire_bytes 1\nabc\n",
        vyre_foundation::serial::text::TEXT_FORMAT_HEADER
    );
    let err = Program::from_text(&input).unwrap_err();
    let msg = err.message();
    assert!(
        msg.contains("Fix:") || msg.contains("odd") || msg.contains("OddHexLineLength"),
        "expected odd-length error, got: {msg}"
    );
}
