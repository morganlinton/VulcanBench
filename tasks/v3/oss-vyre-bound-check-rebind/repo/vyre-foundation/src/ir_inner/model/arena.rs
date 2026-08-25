//! Arena-backed expression storage for opt-in IR construction.
#![allow(unsafe_code)]

use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::program::BufferDecl;
use bumpalo::Bump;
use rustc_hash::FxHashMap;
use std::cell::{Cell, UnsafeCell};
use std::sync::Arc;

/// Stable handle to an expression allocated in an [`ExprArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprRef {
    index: usize,
}

impl ExprRef {
    /// Zero-based expression index within the arena.
    #[must_use]
    #[inline]
    pub fn index(self) -> usize {
        self.index
    }
}

/// Bump-allocated expression arena.
///
/// This is an opt-in migration path for builders that create many temporary
/// expression nodes. Existing callers can continue to use boxed [`Expr`] trees
/// through `Program::new`.
#[derive(Default)]
pub struct ExprArena {
    bump: Bump,
    exprs: UnsafeCell<Vec<*const Expr>>,
    len: Cell<usize>,
}

impl ExprArena {
    /// Create an empty expression arena.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate an expression and return its stable arena handle.
    #[must_use]
    pub fn alloc(&self, expr: Expr) -> ExprRef {
        let index = self.len.get();
        let ptr = std::ptr::from_ref::<Expr>(self.bump.alloc(expr));
        // SAFETY: ExprArena is a single-writer builder and never shared across writer threads.
        unsafe {
            (*self.exprs.get()).push(ptr);
        }
        self.len.set(index + 1);
        ExprRef { index }
    }

    /// Borrow an allocated expression by handle.
    #[must_use]
    pub fn get(&self, expr_ref: ExprRef) -> Option<&Expr> {
        // SAFETY: pointers are produced only by `self.bump.alloc` and remain
        // stable until `reset(&mut self)`, which requires exclusive access.
        unsafe {
            let vec: &Vec<*const Expr> = &*self.exprs.get();
            vec.get(expr_ref.index).and_then(|ptr| ptr.as_ref())
        }
    }

    /// Clear allocated expressions.
    pub fn reset(&mut self) {
        // SAFETY: we have exclusive mutable access to the arena, so it is safe to
        // drop all allocated expressions in place.
        unsafe {
            let vec = self.exprs.get_mut();
            for &ptr in vec.iter() {
                std::ptr::drop_in_place(ptr as *mut Expr);
            }
            vec.clear();
        }
        self.len.set(0);
        self.bump.reset();
    }

    /// Number of expressions allocated in this arena.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.len.get()
    }

    /// Return true if no expressions have been allocated.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for ExprArena {
    fn drop(&mut self) {
        // SAFETY: during destruction, we have exclusive access to the arena, so it is safe
        // to drop all allocated expressions in place.
        unsafe {
            let vec = self.exprs.get_mut();
            for &ptr in vec.iter() {
                std::ptr::drop_in_place(ptr as *mut Expr);
            }
        }
    }
}

/// Lightweight program scaffold for arena-backed expression builders.
pub struct ArenaProgram<'a> {
    arena: &'a ExprArena,
    buffers: Vec<BufferDecl>,
    buffer_index: FxHashMap<Arc<str>, usize>,
    workgroup_size: [u32; 3],
    entry: Vec<ExprRef>,
}

impl<'a> ArenaProgram<'a> {
    pub(crate) fn new(
        arena: &'a ExprArena,
        buffers: Vec<BufferDecl>,
        workgroup_size: [u32; 3],
    ) -> Self {
        let mut buffer_index = FxHashMap::default();
        buffer_index.reserve(buffers.len());
        for (index, buffer) in buffers.iter().enumerate() {
            buffer_index
                .entry(Arc::clone(&buffer.name))
                .or_insert(index);
        }
        Self {
            arena,
            buffers,
            buffer_index,
            workgroup_size,
            entry: Vec::new(),
        }
    }

    /// Allocate `expr` in the backing arena and append it to the entry list.
    #[must_use]
    pub fn push_expr(&mut self, expr: Expr) -> ExprRef {
        let expr_ref = self.arena.alloc(expr);
        self.entry.push(expr_ref);
        expr_ref
    }

    /// Return an expression previously appended to this arena program.
    #[must_use]
    pub fn expr(&self, expr_ref: ExprRef) -> Option<&Expr> {
        self.arena.get(expr_ref)
    }

    /// Declared buffers.
    #[must_use]
    pub fn buffers(&self) -> &[BufferDecl] {
        &self.buffers
    }

    /// Look up a declared buffer by name.
    #[must_use]
    pub fn buffer(&self, name: &str) -> Option<&BufferDecl> {
        self.buffer_index
            .get(name)
            .and_then(|&index| self.buffers.get(index))
    }

    /// Workgroup dimensions.
    #[must_use]
    pub fn workgroup_size(&self) -> [u32; 3] {
        self.workgroup_size
    }

    /// Entry expression handles in append order.
    #[must_use]
    pub fn entry(&self) -> &[ExprRef] {
        &self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::{ArenaProgram, ExprArena};
    use crate::ir_inner::model::expr::Expr;
    use crate::ir_inner::model::program::BufferDecl;
    use crate::ir_inner::model::types::DataType;

    #[test]
    fn arena_allocates_stable_expression_refs() {
        let arena = ExprArena::new();
        let first = arena.alloc(Expr::u32(7));
        let second = arena.alloc(Expr::var("x"));
        assert_eq!(first.index(), 0);
        assert_eq!(second.index(), 1);
        assert_eq!(arena.get(first), Some(&Expr::u32(7)));
        assert_eq!(arena.get(second), Some(&Expr::var("x")));
    }

    #[test]
    fn arena_program_keeps_buffers_and_expression_handles() {
        let arena = ExprArena::new();
        let mut program = ArenaProgram::new(
            &arena,
            vec![BufferDecl::read("input", 0, DataType::U32)],
            [64, 1, 1],
        );
        let expr_ref = program.push_expr(Expr::load("input", Expr::u32(0)));
        assert_eq!(program.entry(), &[expr_ref]);
        assert_eq!(program.buffer("input").map(BufferDecl::binding), Some(0));
        assert_eq!(
            program.expr(expr_ref),
            Some(&Expr::load("input", Expr::u32(0)))
        );
    }
}
