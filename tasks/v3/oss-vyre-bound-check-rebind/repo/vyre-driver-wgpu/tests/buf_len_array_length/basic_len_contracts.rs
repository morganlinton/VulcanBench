use super::*;

fn buf_len_writer_program() -> Program {
    let body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

fn byte_buf_len_writer_program() -> Program {
    let body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

fn byte_load_writer_program(index: u32) -> Program {
    let body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::cast(DataType::U32, Expr::load("input", Expr::u32(index))),
        )],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

fn four_byte_pack_writer_program(start: u32) -> Program {
    let mut body = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind(
                "b0",
                Expr::cast(DataType::U32, Expr::load("input", Expr::u32(start))),
            ),
            Node::let_bind(
                "b1",
                Expr::cast(
                    DataType::U32,
                    Expr::load("input", Expr::u32(start.saturating_add(1))),
                ),
            ),
            Node::let_bind(
                "b2",
                Expr::cast(
                    DataType::U32,
                    Expr::load("input", Expr::u32(start.saturating_add(2))),
                ),
            ),
            Node::let_bind(
                "b3",
                Expr::cast(
                    DataType::U32,
                    Expr::load("input", Expr::u32(start.saturating_add(3))),
                ),
            ),
            Node::store(
                "out",
                Expr::u32(0),
                Expr::bitor(
                    Expr::var("b0"),
                    Expr::bitor(
                        Expr::shl(Expr::var("b1"), Expr::u32(8)),
                        Expr::bitor(
                            Expr::shl(Expr::var("b2"), Expr::u32(16)),
                            Expr::shl(Expr::var("b3"), Expr::u32(24)),
                        ),
                    ),
                ),
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        std::mem::take(&mut body),
    )
}

#[test]
fn buf_len_returns_one_element_for_four_byte_input() {
    // The fixture binds a 4-byte input → 1 u32 element. ArrayLength
    // must report 1; the IR Store writes that to out[0]. Before the
    // Q3 fix, this returned 0 on wgpu/Vulkan.
    let program = buf_len_writer_program();
    let observed = dispatch_and_read_first_word(&program, vec![0xAB, 0, 0, 0]);
    assert_eq!(
        observed, 1,
        "Q3: arrayLength on a 4-byte (1×u32) read-only storage buffer must return 1, got {observed}. \
         If this is 0, the wgpu/Vulkan path is computing the binding range wrong for small storage buffers  -  \
         see docs/optimization/ROADMAP.md Q3."
    );
}

#[test]
fn buf_len_returns_three_elements_for_twelve_byte_input() {
    // Three u32 elements. Same reasoning as the single-element case
    // but covers a non-minimal size to rule out a `max(1)` saturation
    // or similar implementation accident.
    let program = buf_len_writer_program();
    let observed =
        dispatch_and_read_first_word(&program, vec![0x01, 0, 0, 0, 0x02, 0, 0, 0, 0x03, 0, 0, 0]);
    assert_eq!(
        observed, 3,
        "Q3: arrayLength on a 12-byte (3×u32) read-only storage buffer must return 3, got {observed}."
    );
}

#[test]
fn buf_len_returns_eight_elements_for_thirty_two_byte_input() {
    // 32 bytes  -  past the 16-byte/32-byte minimum-binding-size
    // thresholds some Vulkan stacks impose. If ArrayLength is broken
    // only below a threshold, this case should pass while the smaller
    // ones fail; documenting the boundary helps Q3's root-cause search.
    let program = buf_len_writer_program();
    let bytes: Vec<u8> = (0..32).map(|i| i as u8).collect();
    let observed = dispatch_and_read_first_word(&program, bytes);
    assert_eq!(
        observed, 8,
        "Q3: arrayLength on a 32-byte (8×u32) read-only storage buffer must return 8, got {observed}."
    );
}

#[test]
fn byte_buf_len_reports_padded_byte_capacity_for_dynamic_u8_input() {
    let program = byte_buf_len_writer_program();
    let observed = dispatch_and_read_first_word(&program, vec![b'a', b'b', b'c', b'd', b'e']);
    assert_eq!(
        observed, 8,
        "U8 storage is packed into WGSL u32 words, so dynamic buf_len must expose byte capacity \
         (arrayLength * 4) to byte-addressed IR helpers; got {observed}."
    );
}

#[test]
fn byte_load_extracts_lane_from_dynamic_u8_input() {
    let program = byte_load_writer_program(11);
    let observed = dispatch_and_read_first_word(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        observed,
        u32::from(b'/'),
        "U8 load at byte index 11 must extract lane 3 from the packed WGPU u32 word; got {observed}."
    );
}

#[test]
fn byte_loads_pack_adjacent_lanes_from_dynamic_u8_input() {
    let program = four_byte_pack_writer_program(8);
    let observed = dispatch_and_read_first_word(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        observed.to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "four adjacent U8 loads must preserve byte-addressed lanes before byte compaction."
    );
}

