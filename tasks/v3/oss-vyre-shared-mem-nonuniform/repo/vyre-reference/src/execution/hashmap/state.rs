//! Invocation state, local scopes, and workgroup scheduling.
//!
//! This module owns the mutable per-lane interpreter state. It delegates node
//! stepping to `step` and synchronization checks to `sync`; it does not evaluate
//! expressions or resolve buffers directly.

use super::{
    memory::HashmapMemory,
    step::step_round_robin,
    sync::{live_waiting_count, release_barrier_if_ready, verify_uniform_control_flow},
};
use crate::{
    value::Value,
    workgroup::{Frame, InvocationIds},
};
use im::HashMap;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use vyre::ir::{Expr, Node, Program};
use vyre::{Error, OpDef};

#[doc = " Local variable environment backed by persistent maps for O(1) subgroup snapshots."]
pub(crate) struct HashmapLocals {
    pub(crate) locals: HashMap<Arc<str>, Value>,
    pub(crate) immutable: HashMap<Arc<str>, bool>,
    pub(crate) scopes: Vec<Vec<Arc<str>>>,
}

impl HashmapLocals {
    pub(crate) fn new() -> Self {
        Self {
            locals: HashMap::new(),
            immutable: HashMap::new(),
            scopes: vec![Vec::new()],
        }
    }
    pub(crate) fn local(&self, name: &str) -> Option<Value> {
        self.locals.get(name).cloned()
    }

    #[cfg(feature = "subgroup-ops")]
    pub(crate) fn snapshot(&self) -> HashmapLocalSnapshot {
        HashmapLocalSnapshot {
            locals: self.locals.clone(),
        }
    }
    pub(crate) fn bind(&mut self, name: &str, value: Value) -> Result<Arc<str>, Error> {
        if self.locals.contains_key(name) {
            return Err(Error::interp(format!(
                "duplicate local binding `{name}`. Fix: choose a unique local name; shadowing is not allowed."
            )));
        }
        let name: Arc<str> = Arc::from(name);
        self.locals.insert(Arc::clone(&name), value);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Arc::clone(&name));
        }
        Ok(name)
    }
    pub(crate) fn assign(&mut self, name: &str, value: Value) -> Result<(), Error> {
        let key = self
            .locals
            .get_key_value(name)
            .map(|(key, _)| Arc::clone(key))
            .ok_or_else(|| {
                Error::interp(format!(
                    "assignment to undeclared variable `{name}`. Fix: add a Let before assigning it."
                ))
            })?;
        if self.immutable.get(name).copied().unwrap_or(false) {
            return Err(Error::interp(format!(
                "assignment to loop variable `{name}`. Fix: loop variables are immutable."
            )));
        }
        self.locals.insert(key, value);
        Ok(())
    }
    pub(crate) fn bind_loop_var(&mut self, name: &str, value: Value) -> Result<(), Error> {
        let name = self.bind(name, value)?;
        self.immutable.insert(name, true);
        Ok(())
    }
    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }
    pub(crate) fn pop_scope(&mut self) {
        if let Some(names) = self.scopes.pop() {
            for name in names {
                self.locals.remove(&name);
                self.immutable.remove(&name);
            }
        }
    }
}

#[cfg(feature = "subgroup-ops")]
#[doc = " Persistent local value snapshot for subgroup collective evaluation."]
#[derive(Clone)]
pub(crate) struct HashmapLocalSnapshot {
    pub(crate) locals: HashMap<Arc<str>, Value>,
}

#[cfg(feature = "subgroup-ops")]
impl HashmapLocalSnapshot {
    #[cfg(test)]
    pub(crate) fn local(&self, name: &str) -> Option<Value> {
        self.locals.get(name).cloned()
    }
}

pub(crate) struct HashmapInvocation<'a> {
    pub(crate) ids: InvocationIds,
    #[cfg_attr(not(feature = "subgroup-ops"), allow(dead_code))]
    pub(crate) linear_local_index: u32,
    pub(crate) locals: HashmapLocals,
    pub(crate) returned: bool,
    pub(crate) waiting_at_barrier: bool,
    pub(crate) uniform_checks: Vec<(usize, bool)>,
    pub(crate) frames: Vec<Frame<'a>>,
    pub(crate) pending_async: FxHashMap<Arc<str>, HashmapAsyncTransfer>,
    pub(crate) op_cache: FxHashMap<*const Expr, HashmapResolvedCall>,
}

pub(crate) enum HashmapAsyncTransfer {
    Copy {
        destination: String,
        start: usize,
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HashmapResolvedCall {
    pub(crate) def: &'static OpDef,
}

impl<'a> HashmapInvocation<'a> {
    pub(crate) fn new(ids: InvocationIds, linear_local_index: u32, entry: &'a [Node]) -> Self {
        Self {
            ids,
            linear_local_index,
            locals: HashmapLocals::new(),
            returned: false,
            waiting_at_barrier: false,
            uniform_checks: Vec::new(),
            pending_async: FxHashMap::default(),
            op_cache: FxHashMap::default(),
            frames: vec![Frame::Nodes {
                nodes: entry,
                index: 0,
                scoped: false,
            }],
        }
    }
    pub(crate) fn done(&self) -> bool {
        self.returned || self.frames.is_empty()
    }

    pub(crate) fn begin_async(
        &mut self,
        tag: &str,
        transfer: HashmapAsyncTransfer,
    ) -> Result<(), Error> {
        if self.pending_async.contains_key(tag) {
            return Err(Error::interp(format!(
                "async transfer tag `{tag}` was started more than once before a matching wait. Fix: reuse the tag only after AsyncWait completes."
            )));
        }
        self.pending_async.insert(Arc::from(tag), transfer);
        Ok(())
    }

    pub(crate) fn finish_async(&mut self, tag: &str) -> Result<HashmapAsyncTransfer, Error> {
        self.pending_async.remove(tag).ok_or_else(|| Error::interp(format!(
            "async wait for tag `{tag}` has no matching async transfer. Fix: emit AsyncLoad or AsyncStore before AsyncWait."
        )))
    }
}

#[cfg(feature = "subgroup-ops")]
#[derive(Clone)]
pub(crate) struct HashmapInvocationSnapshot {
    pub(crate) ids: InvocationIds,
    pub(crate) linear_local_index: u32,
    pub(crate) locals: HashmapLocalSnapshot,
}

pub(crate) fn create_invocations<'a>(
    program: &Program,
    workgroup: [u32; 3],
    entry: &'a [Node],
) -> Result<Vec<HashmapInvocation<'a>>, Error> {
    let [sx, sy, sz] = program.workgroup_size();
    let invocation_count = sx
        .checked_mul(sy)
        .and_then(|count| count.checked_mul(sz))
        .ok_or_else(|| {
            Error::interp(
                "workgroup invocation count overflows u32. Fix: reduce workgroup dimensions before reference execution.",
            )
        })?;
    let mut invocations = Vec::with_capacity(usize::try_from(invocation_count).map_err(|_| {
        Error::interp(
            "workgroup invocation count exceeds host usize. Fix: reduce workgroup dimensions before reference execution.",
        )
    })?);
    let global_dim = |wgid: u32, size: u32, local: u32| {
        wgid . checked_mul (size) . and_then (| base | base . checked_add (local)) . ok_or_else (| | { Error :: interp ("workgroup * dispatch dimensions overflow u32 global id. Fix: reduce workgroup id or workgroup size so each global_invocation_id component fits in u32." ,) })
    };
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                let local = [x, y, z];
                let global = [
                    global_dim(workgroup[0], sx, x)?,
                    global_dim(workgroup[1], sy, y)?,
                    global_dim(workgroup[2], sz, z)?,
                ];
                invocations.push(HashmapInvocation::new(
                    InvocationIds {
                        global,
                        workgroup,
                        local,
                    },
                    invocations.len() as u32,
                    entry,
                ));
            }
        }
    }
    Ok(invocations)
}

pub(crate) fn run_invocations(
    memory: &mut HashmapMemory,
    invocations: &mut [HashmapInvocation<'_>],
    #[cfg(feature = "subgroup-ops")] uses_subgroup_ops: bool,
) -> Result<(), Error> {
    while invocations.iter().any(|inv| !inv.done()) {
        let made_progress = step_round_robin(
            memory,
            invocations,
            #[cfg(feature = "subgroup-ops")]
            uses_subgroup_ops,
        )?;
        verify_uniform_control_flow(invocations)?;
        if release_barrier_if_ready(invocations) {
            continue;
        }
        if !made_progress && live_waiting_count(invocations) > 0 {
            return Err(Error::interp(
                "program violates uniform-control-flow rule: not every live invocation reached the same barrier. Fix: move Barrier to uniform control flow.",
            ));
        }
    }
    if let Some((invocation, tag)) = invocations.iter().find_map(|invocation| {
        invocation
            .pending_async
            .keys()
            .next()
            .map(|tag| (invocation, tag))
    }) {
        return Err(Error::interp(format!(
            "invocation {:?} completed with async transfer tag `{tag}` still pending. Fix: add AsyncWait for every AsyncLoad/AsyncStore tag before Return or end-of-program.",
            invocation.ids
        )));
    }
    Ok(())
}
