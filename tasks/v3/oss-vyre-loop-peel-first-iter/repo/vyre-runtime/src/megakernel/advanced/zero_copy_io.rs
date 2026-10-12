//! GPU-initiated IO fragment via `AsyncLoad`.
//!
//! Emits the IR node that asks the runtime scheduler to map source and
//! destination capability tables onto the concrete ingest path. Linux runtimes
//! wire that request to registered mapped reads or the native GPUDirect NVMe
//! driver; this module owns only the device-side request fragment.

use vyre_foundation::ir::{Expr, Node};

use crate::megakernel::io::{
    IO_DESTINATION_CAPABILITY_TABLE, IO_QUEUE_DMA_TAG, IO_SOURCE_CAPABILITY_TABLE,
};

/// Binding names for a GPU-initiated direct file pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectIoBindings {
    /// Source handle table or device namespace.
    pub source: &'static str,
    /// Destination GPU cache buffer.
    pub destination: &'static str,
    /// Variable naming the first byte to pull.
    pub file_start: &'static str,
    /// Variable naming one-past-last byte to pull.
    pub file_end: &'static str,
    /// Trace tag attached to the async transfer.
    pub tag: &'static str,
}

impl Default for DirectIoBindings {
    fn default() -> Self {
        Self {
            source: IO_SOURCE_CAPABILITY_TABLE,
            destination: IO_DESTINATION_CAPABILITY_TABLE,
            file_start: "file_start",
            file_end: "file_end",
            tag: IO_QUEUE_DMA_TAG,
        }
    }
}

/// Emit a GPU-initiated direct file pull using default megakernel names.
///
/// This translates to the backend's async-load path; callers must pair it
/// with the runtime IO queue that maps `source` to device/offset handles.
#[must_use]
pub fn pull_file_async_direct() -> Node {
    pull_file_async_direct_with(&DirectIoBindings::default())
}

/// Emit a GPU-initiated direct file pull using custom binding names.
#[must_use]
pub fn pull_file_async_direct_with(bindings: &DirectIoBindings) -> Node {
    Node::async_load_ext(
        bindings.source,
        bindings.destination,
        Expr::var(bindings.file_start),
        Expr::sub(Expr::var(bindings.file_end), Expr::var(bindings.file_start)),
        bindings.tag,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_io_uses_extended_async_load_fields() {
        let node = pull_file_async_direct();
        let Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } = node
        else {
            panic!("direct IO must emit AsyncLoad");
        };
        assert_eq!(source.as_str(), IO_SOURCE_CAPABILITY_TABLE);
        assert_eq!(destination.as_str(), IO_DESTINATION_CAPABILITY_TABLE);
        assert_eq!(tag.as_str(), IO_QUEUE_DMA_TAG);
        assert!(matches!(*offset, Expr::Var(_)));
        assert!(matches!(*size, Expr::BinOp { .. }));
    }
}
