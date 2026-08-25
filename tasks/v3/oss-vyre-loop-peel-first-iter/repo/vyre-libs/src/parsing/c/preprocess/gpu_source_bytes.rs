use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceByteLayout {
    PackedU32,
    RawU8,
}

/// Runtime byte bound for a packed `source` buffer.
pub(crate) fn packed_source_byte_len_expr() -> Expr {
    packed_buffer_byte_len_expr("source")
}

/// Runtime byte bound for a source-like byte buffer.
pub(crate) fn source_byte_len_expr(buffer: &'static str, layout: SourceByteLayout) -> Expr {
    match layout {
        SourceByteLayout::PackedU32 => packed_buffer_byte_len_expr(buffer),
        SourceByteLayout::RawU8 => Expr::buf_len(buffer),
    }
}

/// Runtime byte bound for any packed U32 byte buffer.
pub(crate) fn packed_buffer_byte_len_expr(buffer: &'static str) -> Expr {
    Expr::mul(Expr::buf_len(buffer), Expr::u32(4))
}

pub(crate) fn source_buffer_element(layout: SourceByteLayout) -> DataType {
    match layout {
        SourceByteLayout::PackedU32 => DataType::U32,
        SourceByteLayout::RawU8 => DataType::U8,
    }
}

/// Load one byte from a canonical packed U32 byte buffer.
pub(crate) fn load_packed_byte_expr(buffer: &'static str, addr: Expr) -> Expr {
    crate::scan::builders::load_packed_byte_expr(buffer, addr)
}

/// Load one byte from a source-like byte buffer.
pub(crate) fn load_source_layout_byte_expr(
    buffer: &'static str,
    layout: SourceByteLayout,
    addr: Expr,
) -> Expr {
    match layout {
        SourceByteLayout::PackedU32 => load_packed_byte_expr(buffer, addr),
        SourceByteLayout::RawU8 => Expr::bitand(
            Expr::cast(DataType::U32, Expr::load(buffer, addr)),
            Expr::u32(0xFF),
        ),
    }
}

/// Load one byte from a packed U32 byte buffer, returning zero outside
/// the supplied byte bound.
pub(crate) fn safe_load_packed_byte_expr(
    buffer: &'static str,
    addr: Expr,
    byte_bound: Expr,
) -> Expr {
    Expr::select(
        Expr::lt(addr.clone(), byte_bound),
        load_packed_byte_expr(buffer, addr),
        Expr::u32(0),
    )
}

/// Load one byte from a source-like byte buffer, returning zero outside
/// the supplied byte bound.
pub(crate) fn safe_load_source_layout_byte_expr(
    buffer: &'static str,
    layout: SourceByteLayout,
    addr: Expr,
    byte_bound: Expr,
) -> Expr {
    Expr::select(
        Expr::lt(addr.clone(), byte_bound),
        load_source_layout_byte_expr(buffer, layout, addr),
        Expr::u32(0),
    )
}

/// Load one byte from the canonical packed `source` buffer, returning
/// zero outside the runtime byte bound.
pub(crate) fn safe_load_source_byte_expr(addr: Expr, source_byte_len: Expr) -> Expr {
    safe_load_packed_byte_expr("source", addr, source_byte_len)
}

/// Common ABI buffers for standalone literal scanners.
pub(crate) fn literal_scan_common_buffers(
    source_binding: u32,
    start_pos_binding: u32,
    value_out_binding: u32,
    bytes_consumed_out_binding: u32,
) -> Vec<BufferDecl> {
    vec![
        BufferDecl::storage(
            "source",
            source_binding,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(0),
        BufferDecl::storage(
            "start_pos",
            start_pos_binding,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(1),
        BufferDecl::storage(
            "value_out",
            value_out_binding,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
        BufferDecl::storage(
            "bytes_consumed_out",
            bytes_consumed_out_binding,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
    ]
}

/// Single-element status output buffer for literal scanners.
pub(crate) fn literal_scan_status_output(name: &'static str, binding: u32) -> BufferDecl {
    BufferDecl::storage(name, binding, BufferAccess::ReadWrite, DataType::U32).with_count(1)
}

/// Canonical standalone literal scanner wrapper.
pub(crate) fn literal_scan_program(
    buffers: Vec<BufferDecl>,
    body: Vec<Node>,
    op_id: &'static str,
) -> Program {
    Program::wrapped(buffers, [256, 1, 1], body).with_entry_op_id(op_id)
}
