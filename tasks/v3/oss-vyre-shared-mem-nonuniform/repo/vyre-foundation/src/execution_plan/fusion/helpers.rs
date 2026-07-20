//! Misc fusion helpers: composition keys + buffer-access lattice upgrade.

use crate::ir::{BufferAccess, BufferDecl, Program};

pub(super) fn fallback_composition_key(prog: &Program) -> String {
    let mut hasher = blake3::Hasher::new();
    for buf in prog.buffers() {
        hasher.update(buf.name().as_bytes());
        hasher.update(&[0]);
    }
    for dim in prog.workgroup_size() {
        hasher.update(&dim.to_le_bytes());
    }
    hasher.update(&(prog.entry().len() as u64).to_le_bytes());
    format!("{}", hasher.finalize().to_hex())
}

/// Upgrade `buffer.access` to the more permissive of the two modes.
pub(super) fn upgrade_buffer_access(buffer: &mut BufferDecl, other: &BufferAccess) {
    let current = buffer.access();
    buffer.access = match (&current, &other) {
        (BufferAccess::ReadWrite, _)
        | (_, BufferAccess::ReadWrite)
        | (BufferAccess::WriteOnly, BufferAccess::ReadOnly | BufferAccess::Uniform)
        | (BufferAccess::ReadOnly | BufferAccess::Uniform, BufferAccess::WriteOnly) => {
            BufferAccess::ReadWrite
        }
        (BufferAccess::WriteOnly, BufferAccess::WriteOnly) => BufferAccess::WriteOnly,
        (BufferAccess::Uniform, _) | (_, BufferAccess::Uniform) => BufferAccess::Uniform,
        (BufferAccess::Workgroup, _) | (_, BufferAccess::Workgroup) => BufferAccess::Workgroup,
        _ => BufferAccess::ReadOnly,
    };
    // Keep kind in sync with the upgraded access.
    buffer.kind = match buffer.access {
        BufferAccess::ReadOnly => crate::ir::MemoryKind::Readonly,
        BufferAccess::Uniform => crate::ir::MemoryKind::Uniform,
        BufferAccess::Workgroup => crate::ir::MemoryKind::Shared,
        _ => crate::ir::MemoryKind::Global,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_plan::fusion::collectors::collect_buffer_targets;
    use crate::ir::{DataType, Expr, Ident, Node};
    use rustc_hash::FxHashSet;

    #[test]
    fn upgrade_write_only_read_only_to_read_write() {
        let mut buffer = BufferDecl::storage("tmp", 0, BufferAccess::WriteOnly, DataType::U32);

        upgrade_buffer_access(&mut buffer, &BufferAccess::ReadOnly);

        assert_eq!(buffer.access(), BufferAccess::ReadWrite);
        assert_eq!(buffer.kind(), crate::ir::MemoryKind::Global);
    }

    fn collect_targets(node: &Node) -> (FxHashSet<Ident>, FxHashSet<Ident>, FxHashSet<Ident>) {
        let mut loads = FxHashSet::default();
        let mut stores = FxHashSet::default();
        let mut atomics = FxHashSet::default();
        collect_buffer_targets(node, &mut loads, &mut stores, &mut atomics);
        (loads, stores, atomics)
    }

    /// An `AsyncStore` reads `source` and writes `destination` (vyre-reference
    /// `eval_async_store` reads source then writes destination). The fusion
    /// cross-arm RAW/WAR barrier pass keys off `collect_buffer_targets`, so
    /// both must be recorded — otherwise a later arm reads a buffer an earlier
    /// arm async-wrote with no barrier between them (a stale-read miscompile).
    #[test]
    fn collect_buffer_targets_records_async_store_source_read_and_destination_write() {
        let node = Node::AsyncStore {
            source: Ident::from("staged"),
            destination: Ident::from("out"),
            offset: Box::new(Expr::u32(0)),
            size: Box::new(Expr::u32(16)),
            tag: Ident::from("t0"),
        };
        let (loads, stores, _atomics) = collect_targets(&node);
        assert!(
            loads.contains(&Ident::from("staged")),
            "AsyncStore source must be recorded as a buffer read; got loads={loads:?}"
        );
        assert!(
            stores.contains(&Ident::from("out")),
            "AsyncStore destination must be recorded as a buffer write; got stores={stores:?}"
        );
    }

    /// Symmetric to the store case: an `AsyncLoad` reads `source` and writes
    /// `destination`.
    #[test]
    fn collect_buffer_targets_records_async_load_source_read_and_destination_write() {
        let node = Node::AsyncLoad {
            source: Ident::from("global_in"),
            destination: Ident::from("shared_tile"),
            offset: Box::new(Expr::u32(0)),
            size: Box::new(Expr::u32(16)),
            tag: Ident::from("t1"),
        };
        let (loads, stores, _atomics) = collect_targets(&node);
        assert!(
            loads.contains(&Ident::from("global_in")),
            "AsyncLoad source must be recorded as a buffer read; got loads={loads:?}"
        );
        assert!(
            stores.contains(&Ident::from("shared_tile")),
            "AsyncLoad destination must be recorded as a buffer write; got stores={stores:?}"
        );
    }

    /// `IndirectDispatch` reads its `count_buffer` to derive launch geometry;
    /// the hazard detector must see that read so a write of the count buffer in
    /// an earlier arm forces a barrier before the dispatch consumes it.
    #[test]
    fn collect_buffer_targets_records_indirect_dispatch_count_buffer_read() {
        let node = Node::IndirectDispatch {
            count_buffer: Ident::from("counts"),
            count_offset: 0,
        };
        let (loads, _stores, _atomics) = collect_targets(&node);
        assert!(
            loads.contains(&Ident::from("counts")),
            "IndirectDispatch count_buffer must be recorded as a buffer read; got loads={loads:?}"
        );
    }
}
