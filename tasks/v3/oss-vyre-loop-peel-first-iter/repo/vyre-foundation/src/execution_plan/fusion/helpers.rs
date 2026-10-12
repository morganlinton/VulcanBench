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
    use crate::ir::DataType;

    #[test]
    fn upgrade_write_only_read_only_to_read_write() {
        let mut buffer = BufferDecl::storage("tmp", 0, BufferAccess::WriteOnly, DataType::U32);

        upgrade_buffer_access(&mut buffer, &BufferAccess::ReadOnly);

        assert_eq!(buffer.access(), BufferAccess::ReadWrite);
        assert_eq!(buffer.kind(), crate::ir::MemoryKind::Global);
    }
}
