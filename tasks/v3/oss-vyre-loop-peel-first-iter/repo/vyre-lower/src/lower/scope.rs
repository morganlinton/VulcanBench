//! Variable-scope mechanics for neutral lowering.
//!
//! `vyre_foundation::Node` is name-based while `KernelDescriptor` is
//! result-id based. This module owns the name → result-id transition
//! rules so branch isolation and loop-carried state are explicit.

use vyre_foundation::ir::Ident;

#[derive(Clone, Default)]
pub(super) struct VarScope {
    bindings: im::HashMap<Ident, u32>,
}

pub(super) type ScopeSnapshot = im::HashMap<Ident, u32>;

impl VarScope {
    pub(super) fn bind(&mut self, name: Ident, result: u32) -> Option<u32> {
        self.bindings.insert(name, result)
    }

    pub(super) fn get(&self, name: &Ident) -> Option<u32> {
        self.bindings.get(name).copied()
    }

    pub(super) fn snapshot(&self) -> ScopeSnapshot {
        self.bindings.clone()
    }

    pub(super) fn restore(&mut self, snapshot: ScopeSnapshot) {
        self.bindings = snapshot;
    }

    pub(super) fn restore_loop_exit(
        &mut self,
        incoming: ScopeSnapshot,
        loop_exit: &ScopeSnapshot,
        loop_var: &Ident,
    ) {
        self.bindings = incoming.clone();
        for name in incoming.keys() {
            if name == loop_var {
                continue;
            }
            if let Some(updated) = loop_exit.get(name) {
                self.bindings.insert(name.clone(), *updated);
            }
        }
    }
}
