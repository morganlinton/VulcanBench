use super::*;

pub(super) fn safe_load_src_expr(
    source_layout: super::super::gpu_source_bytes::SourceByteLayout,
    addr: Expr,
    source_byte_len: Expr,
) -> Expr {
    super::super::gpu_source_bytes::safe_load_source_layout_byte_expr(
        "source",
        source_layout,
        addr,
        source_byte_len,
    )
}
