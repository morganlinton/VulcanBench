//! Workgroup simulation  -  the parity engine's model of invocation coordination.
//!
//! GPU backends must reproduce the exact barrier synchronization, shared-memory
//! layout, and invocation-ID arithmetic that this module defines. The conform gate
//! compares GPU dispatch output against this deterministic CPU simulation; any
//! divergence in control flow uniformity or workgroup memory semantics is a bug.

use std::convert::Infallible;
use std::ops::ControlFlow::{self, Continue};
use std::sync::Arc;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
#[cfg(test)]
use vyre::ir::BufferAccess;
use vyre::ir::{Expr, Node, Program};
use vyre::visit::{visit_node_preorder, visit_preorder, ExprVisitor, NodeVisitor};
use vyre::OpDef;
use vyre_foundation::ir::model::expr::GeneratorRef;

use vyre::Error;

use crate::{oob::Buffer, value::Value};

/// Maximum per-workgroup shared memory the reference interpreter will allocate.
pub const MAX_WORKGROUP_BYTES: usize = 64 * 1024 * 1024;

/// Small-N buffer lookup keyed by interned `Arc<str>` names.
///
/// Typical reference interpreter programs have ≤ 8 declared buffers. A
/// linear scan over 8 entries is branch-predicted and hits L1 cache; hashing
/// each access (as `HashMap<String, Buffer>` did) burned a SipHash-1-3 on
/// every load/store in the inner interpreter loop. This struct preserves
/// the public `get` / `get_mut` / `insert` shape consumers depend on while
/// eliminating the per-lookup hash + heap traffic.
#[derive(Debug, Default, Clone)]
pub struct BufferMap {
    entries: SmallVec<[(Arc<str>, Buffer); 8]>,
}

impl BufferMap {
    /// Construct an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: SmallVec::new(),
        }
    }

    /// Look up a buffer by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Buffer> {
        self.entries
            .iter()
            .find(|(key, _)| key.as_ref() == name)
            .map(|(_, buffer)| buffer)
    }

    /// Look up a mutable buffer by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Buffer> {
        self.entries
            .iter_mut()
            .find(|(key, _)| key.as_ref() == name)
            .map(|(_, buffer)| buffer)
    }

    /// Insert or overwrite a buffer. Returns the previous value when the
    /// key already existed.
    pub fn insert(&mut self, name: impl Into<Arc<str>>, buffer: Buffer) -> Option<Buffer> {
        let name = name.into();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|(key, _)| key.as_ref() == name.as_ref())
        {
            return Some(std::mem::replace(&mut entry.1, buffer));
        }
        self.entries.push((name, buffer));
        None
    }

    /// Iterate `(name, buffer)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Buffer)> {
        self.entries
            .iter()
            .map(|(name, buffer)| (name.as_ref(), buffer))
    }

    /// Move-iterate `(name, buffer)` pairs.
    pub fn into_iter_pairs(self) -> impl Iterator<Item = (Arc<str>, Buffer)> {
        self.entries.into_iter()
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Identity of one compute invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvocationIds {
    /// Global invocation id.
    pub global: [u32; 3],
    /// Workgroup id.
    pub workgroup: [u32; 3],
    /// Local invocation id.
    pub local: [u32; 3],
}

impl InvocationIds {
    /// Zero-valued invocation ids for examples and unit tests.
    pub const ZERO: Self = Self {
        global: [0, 0, 0],
        workgroup: [0, 0, 0],
        local: [0, 0, 0],
    };
}

/// Shared execution memory for storage and current workgroup buffers.
#[derive(Debug, Default, Clone)]
pub struct Memory {
    pub(crate) storage: BufferMap,
    pub(crate) workgroup: BufferMap,
}

impl Memory {
    /// Create empty memory for test fixtures.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Add a storage buffer.
    #[must_use]
    pub fn with_storage(mut self, name: impl Into<Arc<str>>, buffer: Buffer) -> Self {
        self.storage.insert(name, buffer);
        self
    }

    /// Add a workgroup buffer.
    #[must_use]
    pub fn with_workgroup(mut self, name: impl Into<Arc<str>>, buffer: Buffer) -> Self {
        self.workgroup.insert(name, buffer);
        self
    }

    /// Build a single byte payload memory used by canonical primitive evaluators.
    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let mut storage = BufferMap::new();
        storage.insert("__value", Buffer::new(bytes, vyre::ir::DataType::Bytes));
        Self {
            storage,
            workgroup: BufferMap::new(),
        }
    }

    /// Return the byte payload for canonical primitive evaluators.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.storage.get("__value").map_or_else(Vec::new, |buffer| {
            buffer
                .bytes
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        })
    }

    /// Consume this memory and return the byte payload for canonical primitives.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.storage
            .into_iter_pairs()
            .find_map(|(name, buffer)| {
                (name.as_ref() == "__value").then(|| {
                    std::sync::Arc::try_unwrap(buffer.bytes)
                        .map(|rw| rw.into_inner().unwrap_or_else(|error| error.into_inner()))
                        .unwrap_or_else(|a| {
                            a.read().unwrap_or_else(|error| error.into_inner()).clone()
                        })
                })
            })
            .unwrap_or_default()
    }
}

/// Shared slot layout for all locals in one program.
#[derive(Debug, Default)]
pub struct LocalSlots {
    names: rustc_hash::FxHashMap<Arc<str>, usize>,
    slot_names: Vec<Arc<str>>,
}

impl LocalSlots {
    /// Build a slot layout from every binding site in a program.
    #[must_use]
    pub fn for_program(program: &Program) -> Self {
        Self::for_nodes(program.entry())
    }

    /// Build a slot layout from a node slice.
    #[must_use]
    pub fn for_nodes(nodes: &[Node]) -> Self {
        let mut slots = Self::default();
        for node in nodes {
            match visit_node_preorder(&mut slots, node) {
                Continue(()) => {}
                ControlFlow::Break(never) => match never {},
            }
        }
        slots
    }

    fn slot(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    fn len(&self) -> usize {
        self.slot_names.len()
    }

    fn intern(&mut self, name: &str) {
        if self.names.contains_key(name) {
            return;
        }
        let slot = self.slot_names.len();
        let name: Arc<str> = Arc::from(name);
        self.slot_names.push(Arc::clone(&name));
        self.names.insert(name, slot);
    }
}

impl ExprVisitor for LocalSlots {
    type Break = Infallible;
}

impl NodeVisitor for LocalSlots {
    type Break = Infallible;

    fn visit_let(
        &mut self,
        _: &Node,
        name: &vyre::ir::Ident,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        self.intern(name);
        visit_preorder(self, value)
    }

    fn visit_assign(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, value)
    }

    fn visit_store(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        index: &Expr,
        value: &Expr,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, index)?;
        visit_preorder(self, value)
    }

    fn visit_if(
        &mut self,
        _: &Node,
        cond: &Expr,
        _: &[Node],
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, cond)
    }

    fn visit_loop(
        &mut self,
        _: &Node,
        var: &vyre::ir::Ident,
        from: &Expr,
        to: &Expr,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        self.intern(var);
        visit_preorder(self, from)?;
        visit_preorder(self, to)
    }

    fn visit_indirect_dispatch(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        _: u64,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_async_load(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        _: &vyre::ir::Ident,
        offset: &Expr,
        size: &Expr,
        _: &vyre::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, offset)?;
        visit_preorder(self, size)
    }

    fn visit_async_store(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        _: &vyre::ir::Ident,
        offset: &Expr,
        size: &Expr,
        _: &vyre::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, offset)?;
        visit_preorder(self, size)
    }

    fn visit_async_wait(&mut self, _: &Node, _: &vyre::ir::Ident) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_trap(
        &mut self,
        _: &Node,
        address: &Expr,
        _: &vyre::ir::Ident,
    ) -> ControlFlow<Self::Break> {
        visit_preorder(self, address)
    }

    fn visit_resume(&mut self, _: &Node, _: &vyre::ir::Ident) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_return(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_barrier(&mut self, _: &Node) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_block(&mut self, _: &Node, _: &[Node]) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_region(
        &mut self,
        _: &Node,
        _: &vyre::ir::Ident,
        _: &Option<GeneratorRef>,
        _: &[Node],
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }

    fn visit_opaque_node(
        &mut self,
        _: &Node,
        _: &dyn vyre::ir::NodeExtension,
    ) -> ControlFlow<Self::Break> {
        Continue(())
    }
}

/// One paused or running invocation.
pub struct Invocation<'a> {
    /// Builtin ids for this invocation.
    pub ids: InvocationIds,
    slots: Arc<LocalSlots>,
    locals: Vec<Option<Value>>,
    immutable: Vec<bool>,
    scopes: Vec<Vec<usize>>,
    frames: Vec<Frame<'a>>,
    /// True after `return`.
    pub returned: bool,
    /// True when paused at a barrier.
    pub waiting_at_barrier: bool,
    /// Uniform-if observations for branches that contain a barrier.
    pub uniform_checks: Vec<(usize, bool)>,
    /// Async transfers started by `AsyncLoad`/`AsyncStore` and pending
    /// observation by `AsyncWait`.
    pub(crate) pending_async: FxHashMap<Arc<str>, AsyncTransfer>,
    pub(crate) op_cache: FxHashMap<*const Expr, ResolvedCall>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedCall {
    pub(crate) def: &'static OpDef,
}

/// Interpreter continuation stack.
#[non_exhaustive]
pub enum Frame<'a> {
    /// Sequence of nodes.
    Nodes {
        /// Nodes being executed.
        nodes: &'a [Node],
        /// Next node index.
        index: usize,
        /// Whether completion pops a lexical scope.
        scoped: bool,
    },
    /// Bounded `u32` loop.
    Loop {
        /// Loop variable name.
        var: &'a str,
        /// Next induction value.
        next: u32,
        /// Exclusive upper bound.
        to: u32,
        /// Loop body.
        body: &'a [Node],
    },
}

impl<'a> Invocation<'a> {
    /// Create an invocation at the start of the entry point.
    pub fn new(ids: InvocationIds, entry: &'a [Node]) -> Self {
        Self::with_slots(ids, entry, Arc::new(LocalSlots::for_nodes(entry)))
    }

    pub(crate) fn with_slots(
        ids: InvocationIds,
        entry: &'a [Node],
        slots: Arc<LocalSlots>,
    ) -> Self {
        let slot_count = slots.len();
        Self {
            ids,
            slots,

            locals: vec![None; slot_count],
            immutable: vec![false; slot_count],
            scopes: vec![Vec::new()],
            frames: vec![Frame::Nodes {
                nodes: entry,
                index: 0,
                scoped: false,
            }],
            returned: false,
            waiting_at_barrier: false,
            uniform_checks: Vec::new(),
            pending_async: FxHashMap::default(),
            op_cache: FxHashMap::default(),
        }
    }

    /// Return true when no further execution can occur.
    pub fn done(&self) -> bool {
        self.returned || self.frames.is_empty()
    }

    /// Push a lexical scope.
    ///
    ///
    /// ```rust,no_run
    /// use vyre_reference::workgroup::{Invocation, InvocationIds};
    /// let mut invocation = Invocation::new(InvocationIds::ZERO, &[]);
    /// invocation.push_scope();
    /// ```
    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    /// Pop a lexical scope and remove bindings declared in it.
    ///
    ///
    /// ```rust,no_run
    /// use vyre_reference::workgroup::{Invocation, InvocationIds};
    /// let mut invocation = Invocation::new(InvocationIds::ZERO, &[]);
    /// invocation.pop_scope();
    /// ```
    pub fn pop_scope(&mut self) {
        if let Some(names) = self.scopes.pop() {
            for slot in names {
                self.locals[slot] = None;
                self.immutable[slot] = false;
            }
        }
    }

    pub(crate) fn begin_async(&mut self, tag: &str, transfer: AsyncTransfer) -> Result<(), Error> {
        let tag: Arc<str> = Arc::from(tag);
        if self.pending_async.insert(tag.clone(), transfer).is_some() {
            return Err(Error::interp(format!(
                "async tag `{}` was started more than once before a matching wait. \
                 Fix: reuse the tag only after AsyncWait completes.",
                tag
            )));
        }
        Ok(())
    }

    pub(crate) fn finish_async(&mut self, tag: &str) -> Result<AsyncTransfer, Error> {
        self.pending_async.remove(tag).ok_or_else(|| Error::interp(format!(
            "async wait for tag `{tag}` has no matching async load. Fix: emit AsyncLoad before AsyncWait."
        )))
    }

    /// Look up an active local by name.
    pub fn local(&self, name: &str) -> Option<&Value> {
        self.slots
            .slot(name)
            .and_then(|slot| self.locals.get(slot))
            .and_then(Option::as_ref)
    }

    /// Bind a mutable local.
    ///
    ///
    /// ```rust,no_run
    /// use vyre_reference::{value::Value, workgroup::{Invocation, InvocationIds}};
    /// fn main() -> Result<(), vyre_foundation::Error> {
    ///     let mut invocation = Invocation::new(InvocationIds::ZERO, &[]);
    ///     invocation.bind("example", Value::U32(1))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn bind(&mut self, name: &str, value: Value) -> Result<(), vyre::Error> {
        let slot = self.slots.slot(name).ok_or_else(|| {
            Error::interp(format!(
                "local binding `{name}` has no preassigned slot. Fix: rebuild the local slot layout from the full Program before interpretation."
            ))
        })?;
        if self.locals[slot].is_some() {
            return Err(Error::interp(format!(
                "duplicate local binding `{name}`. Fix: choose a unique local name; shadowing is not allowed."
            )));
        }
        self.locals[slot] = Some(value);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(slot);
        }
        Ok(())
    }

    /// Bind an immutable loop variable.
    ///
    ///
    /// ```rust,no_run
    /// use vyre_reference::{value::Value, workgroup::{Invocation, InvocationIds}};
    /// fn main() -> Result<(), vyre_foundation::Error> {
    ///     let mut invocation = Invocation::new(InvocationIds::ZERO, &[]);
    ///     invocation.bind_loop_var("example", Value::U32(1))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn bind_loop_var(&mut self, name: &str, value: Value) -> Result<(), vyre::Error> {
        self.bind(name, value)?;
        let slot = self.slots.slot(name).ok_or_else(|| {
            Error::interp(format!(
                "local binding `{name}` disappeared after bind. Fix: keep local slot layout immutable during interpretation."
            ))
        })?;
        self.immutable[slot] = true;
        Ok(())
    }

    /// Assign an existing mutable local.
    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), vyre::Error> {
        let slot = self.slots.slot(name).ok_or_else(|| {
            Error::interp(format!(
                "assignment to undeclared variable `{name}`. Fix: add a Let before assigning it."
            ))
        })?;
        if self.immutable[slot] {
            return Err(Error::interp(format!(
                "assignment to loop variable `{name}`. Fix: loop variables are immutable."
            )));
        }
        let Some(local) = self.locals.get_mut(slot).and_then(Option::as_mut) else {
            return Err(Error::interp(format!(
                "assignment to undeclared variable `{name}`. Fix: add a Let before assigning it."
            )));
        };
        *local = value;
        Ok(())
    }

    pub(crate) fn frames_mut(&mut self) -> &mut Vec<Frame<'a>> {
        &mut self.frames
    }
}

/// Deferred byte-copy transfer for the workgroup reference scheduler.
pub(crate) enum AsyncTransfer {
    /// Copy `payload` into `destination` starting at byte offset `start`.
    Copy {
        destination: Arc<str>,
        start: usize,
        payload: Vec<u8>,
    },
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn create_invocations(
    program: &Program,
    workgroup: [u32; 3],
    slots: Arc<LocalSlots>,
) -> Result<Vec<Invocation<'_>>, vyre::Error> {
    let global_dim = |wgid: u32, size: u32, local: u32| {
        wgid
            .checked_mul(size)
            .and_then(|base| base.checked_add(local))
            .ok_or_else(|| Error::interp(
                "workgroup * dispatch dimensions overflow u32 global id. Fix: reduce workgroup id or workgroup size so each global_invocation_id component fits in u32.",
            ))
    };
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
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                let local = [x, y, z];
                let global = [
                    global_dim(workgroup[0], sx, x)?,
                    global_dim(workgroup[1], sy, y)?,
                    global_dim(workgroup[2], sz, z)?,
                ];
                invocations.push(Invocation::with_slots(
                    InvocationIds {
                        global,
                        workgroup,
                        local,
                    },
                    program.entry(),
                    Arc::clone(&slots),
                ));
            }
        }
    }
    Ok(invocations)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn workgroup_memory(program: &Program) -> Result<BufferMap, vyre::Error> {
    let mut workgroup = BufferMap::new();
    let mut allocated = 0usize;
    for decl in program
        .buffers()
        .iter()
        .filter(|decl| decl.access() == BufferAccess::Workgroup)
    {
        let element_size = decl.element().min_bytes();
        let len = (decl.count() as usize)
            .checked_mul(element_size)
            .ok_or_else(|| Error::interp(format!(
                    "workgroup buffer `{}` byte size overflows usize. Fix: reduce count or element size.",
                    decl.name()
            )))?;
        allocated = allocated
            .checked_add(len)
            .ok_or_else(|| Error::interp(
                "total workgroup memory byte size overflows usize. Fix: reduce workgroup buffer declarations.",
            ))?;
        if allocated > MAX_WORKGROUP_BYTES {
            return Err(Error::interp(format!(
                "workgroup memory requires {allocated} bytes, exceeding the {MAX_WORKGROUP_BYTES}-byte reference budget. Fix: reduce workgroup buffer counts."
            )));
        }
        workgroup.insert(decl.name(), Buffer::new(vec![0; len], decl.element()));
    }
    Ok(workgroup)
}
