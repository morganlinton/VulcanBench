//! Category C zero-copy I/O intrinsics.
//!
//! The `io` dialect declares ops that move bytes between persistent
//! storage and GPU memory without a CPU staging copy. These are
//! Category C  -  they have no portable lowering. A concrete backend
//! opts in by registering a `BackendRegistration` that supplies
//! `primary_text` / `primary_binary` / `secondary_text` / `native_module` builders.
//!
//! The ops are opt-in: a backend registers concrete io_uring /
//! GPUDirect Storage lowerings before it can execute them. A program
//! that uses these ops fails capability checks with a clear message
//! unless such a backend is linked.
//!
//! The ops:
//!
//! * `io.dma_from_nvme(fd, offset, length)`  -  stream bytes directly
//!   from an NVMe block device into GPU memory.
//! * `io.write_back_to_nvme(handle, fd, offset)`  -  stream GPU bytes
//!   back to an NVMe block device.
//! * `mem.zerocopy_map(fd)`  -  map a file descriptor so that the GPU
//!   can read it as its own address space (GDS).
//! * `mem.unmap(handle)`  -  release a `mem.zerocopy_map` reservation.
//!
//! Even without lowerings, the ops are compositional in vyre IR:
//! frontends can write Programs against them today, and the Program
//! validates. Execution succeeds only when a backend that supports
//! the `io` dialect is registered.

use crate::OpDefRegistration;
use crate::{Category, OpDef, Signature, TypedParam};

const OP_DMA_FROM_NVME: &str = "io.dma_from_nvme";
const OP_WRITE_BACK_TO_NVME: &str = "io.write_back_to_nvme";
const OP_ZEROCOPY_MAP: &str = "mem.zerocopy_map";
const OP_UNMAP: &str = "mem.unmap";

const SIG_DMA_FROM_NVME: Signature = Signature {
    inputs: &[
        TypedParam {
            name: "fd",
            ty: "i32",
        },
        TypedParam {
            name: "offset",
            ty: "u64",
        },
        TypedParam {
            name: "length",
            ty: "u64",
        },
    ],
    outputs: &[TypedParam {
        name: "handle",
        ty: "GpuBufferHandle",
    }],
    attrs: &[],
    bytes_extraction: false,
};

const SIG_WRITE_BACK_TO_NVME: Signature = Signature {
    inputs: &[
        TypedParam {
            name: "handle",
            ty: "GpuBufferHandle",
        },
        TypedParam {
            name: "fd",
            ty: "i32",
        },
        TypedParam {
            name: "offset",
            ty: "u64",
        },
    ],
    outputs: &[],
    attrs: &[],
    bytes_extraction: false,
};

const SIG_ZEROCOPY_MAP: Signature = Signature {
    inputs: &[TypedParam {
        name: "fd",
        ty: "i32",
    }],
    outputs: &[TypedParam {
        name: "handle",
        ty: "GpuBufferHandle",
    }],
    attrs: &[],
    bytes_extraction: false,
};

const SIG_UNMAP: Signature = Signature {
    inputs: &[TypedParam {
        name: "handle",
        ty: "GpuBufferHandle",
    }],
    outputs: &[],
    attrs: &[],
    bytes_extraction: false,
};

inventory::submit! {
    OpDefRegistration::new(|| OpDef {
        id: OP_DMA_FROM_NVME,
        dialect: "io",
        category: Category::Intrinsic,
        signature: SIG_DMA_FROM_NVME,
        lowerings: crate::LoweringTable::empty(),
        laws: &[],
        compose: None,
    })
}

inventory::submit! {
    OpDefRegistration::new(|| OpDef {
        id: OP_WRITE_BACK_TO_NVME,
        dialect: "io",
        category: Category::Intrinsic,
        signature: SIG_WRITE_BACK_TO_NVME,
        lowerings: crate::LoweringTable::empty(),
        laws: &[],
        compose: None,
    })
}

inventory::submit! {
    OpDefRegistration::new(|| OpDef {
        id: OP_ZEROCOPY_MAP,
        dialect: "io",
        category: Category::Intrinsic,
        signature: SIG_ZEROCOPY_MAP,
        lowerings: crate::LoweringTable::empty(),
        laws: &[],
        compose: None,
    })
}

inventory::submit! {
    OpDefRegistration::new(|| OpDef {
        id: OP_UNMAP,
        dialect: "io",
        category: Category::Intrinsic,
        signature: SIG_UNMAP,
        lowerings: crate::LoweringTable::empty(),
        laws: &[],
        compose: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{DialectRegistry, Target};

    #[test]
    fn every_io_op_registers() -> Result<(), String> {
        let _lock = crate::registry::registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_inventory());
        let reg = DialectRegistry::global();
        for op in [
            OP_DMA_FROM_NVME,
            OP_WRITE_BACK_TO_NVME,
            OP_ZEROCOPY_MAP,
            OP_UNMAP,
        ] {
            let id = reg.intern_op(op);
            let def = reg
                .lookup(id)
                .ok_or_else(|| {
                    format!(
                        "Fix: op `{op}` must register via inventory::submit!(OpDefRegistration{{...}}); restore the registration in this dialect."
                    )
                })?;
            assert_eq!(def.id, op);
            assert_eq!(def.category, Category::Intrinsic);
        }
        Ok(())
    }

    #[test]
    fn io_ops_have_no_gpu_lowering() {
        let _lock = crate::registry::registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_inventory());
        let reg = DialectRegistry::global();
        for op in [
            OP_DMA_FROM_NVME,
            OP_WRITE_BACK_TO_NVME,
            OP_ZEROCOPY_MAP,
            OP_UNMAP,
        ] {
            let id = reg.intern_op(op);
            // No backend opts into io ops yet; target lowerings
            // lowerings are all None. The capability-negotiation
            // layer surfaces a `BackendError::Unsupported` in this
            // case (see B-B5 backend trait split for the checked
            // path).
            assert!(
                reg.get_lowering(id, Target::PrimaryText).is_none(),
                "{op} must not carry a primary-text lowering until a backend opts in"
            );
        }
    }

    #[test]
    fn io_ops_use_structured_intrinsic_sentinel_not_custom_cpu_paths() {
        let _lock = crate::registry::registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_inventory());
        let reg = DialectRegistry::global();
        for op in [
            OP_DMA_FROM_NVME,
            OP_WRITE_BACK_TO_NVME,
            OP_ZEROCOPY_MAP,
            OP_UNMAP,
        ] {
            let id = reg.intern_op(op);
            let def = reg.lookup(id).unwrap();
            assert!(
                vyre_foundation::cpu_op::is_cpu_reference_sentinel(def.lowerings.cpu_ref),
                "{op} must not install a custom CPU path; Category C io ops require concrete backend lowering"
            );
        }
    }

    #[test]
    fn io_dialect_is_distinct_from_stdlib() {
        let _lock = crate::registry::registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_inventory());
        let reg = DialectRegistry::global();
        let id = reg.intern_op(OP_DMA_FROM_NVME);
        let def = reg.lookup(id).unwrap();
        assert_eq!(def.dialect, "io");
    }
}
