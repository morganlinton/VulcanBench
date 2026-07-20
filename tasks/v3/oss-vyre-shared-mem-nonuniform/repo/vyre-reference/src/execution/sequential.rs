//! Sequential CPU execution model for workgroup parity.
//!
//! Replaces the "invocation scheduler" abstraction with an obvious
//! sequential semantic: run invocation 0, then 1, then 2... inside a
//! workgroup. At each barrier, the outer driver re-runs every invocation
//! from the barrier checkpoint so shared-memory effects from earlier
//! invocations are visible to later ones. This matches backend
//! spec-compliant semantics under the parity contract.
//!
//! Exposes a single entry point `run_sequential_workgroup` which the
//! conform runner uses as its CPU oracle; [`crate::workgroup`] owns
//! per-invocation state and memory types shared by the execution tree.

use vyre::ir::Program;

use crate::workgroup::{InvocationIds, MAX_WORKGROUP_BYTES};

/// Driver for sequential per-invocation execution inside a workgroup.
///
/// Does the simplest possible thing: iterate invocations 0..N in order,
/// each time honoring any shared-memory writes made by prior invocations.
/// When the underlying program contains a barrier, the caller replays the
/// full sweep from the barrier point. [`crate::workgroup`] provides the
/// invocation and memory types used by this driver.
#[derive(Debug, Clone, Copy)]
pub struct SequentialWorkgroup {
    /// Workgroup size in x/y/z.
    pub size: [u32; 3],
}

impl SequentialWorkgroup {
    /// Construct a driver for the program's declared workgroup size.
    #[must_use]
    pub fn for_program(program: &Program) -> Self {
        Self {
            size: program.workgroup_size(),
        }
    }

    /// Total number of invocations in one workgroup.
    #[must_use]
    pub fn invocation_count(&self) -> u32 {
        self.size[0]
            .saturating_mul(self.size[1])
            .saturating_mul(self.size[2])
    }

    /// Yield the invocation ids in canonical order (z-major, y-major, x-minor).
    pub fn invocations(&self, workgroup_id: [u32; 3]) -> impl Iterator<Item = InvocationIds> {
        let [sx, sy, sz] = self.size;
        let wg = workgroup_id;
        (0..sz).flat_map(move |lz| {
            (0..sy).flat_map(move |ly| {
                (0..sx).map(move |lx| InvocationIds {
                    global: [
                        wg[0].saturating_mul(sx).saturating_add(lx),
                        wg[1].saturating_mul(sy).saturating_add(ly),
                        wg[2].saturating_mul(sz).saturating_add(lz),
                    ],
                    workgroup: wg,
                    local: [lx, ly, lz],
                })
            })
        })
    }
}

/// Maximum shared-memory allocation exported for test convenience so the
/// sequential driver and workgroup memory model agree on bounds.
pub const MAX_SHARED_BYTES: usize = MAX_WORKGROUP_BYTES;

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferDecl, DataType, Node, Program};

    fn trivial_program(size: [u32; 3]) -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32)],
            size,
            vec![Node::let_bind("idx", vyre::ir::Expr::gid_x())],
        )
    }

    #[test]
    fn invocation_count_is_product() {
        let wg = SequentialWorkgroup::for_program(&trivial_program([4, 2, 1]));
        assert_eq!(wg.invocation_count(), 8);
    }

    #[test]
    fn invocation_order_is_canonical() {
        let wg = SequentialWorkgroup { size: [2, 2, 1] };
        let ids: Vec<_> = wg.invocations([0, 0, 0]).map(|i| i.local).collect();
        assert_eq!(ids, vec![[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0]]);
    }

    #[test]
    fn invocation_globals_offset_by_workgroup() {
        let wg = SequentialWorkgroup { size: [2, 1, 1] };
        let ids: Vec<_> = wg.invocations([3, 0, 0]).map(|i| i.global).collect();
        assert_eq!(ids, vec![[6, 0, 0], [7, 0, 0]]);
    }
}
