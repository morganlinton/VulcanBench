//! Categorical primitives (P-PRIM-16/17/18).
//!
//! Pure-CPU primitives over finite categories: Yoneda embedding,
//! adjoint-pair detection, Kan extension along a functor. The pass
//! scheduler / functorial_pass_composition substrate consumes these
//! to reason about pass equivalences and free-functor laws.
//!
//! Categories here are finite: a fixed object set + a Hom-set lookup
//! table. Morphisms are u32 ids, composition is a `(f, g) → f∘g`
//! lookup, identities are `id_X` per object. Functors map objects
//! and morphisms.

pub mod adjoint;
pub mod kan_extension;
pub mod yoneda;

pub use adjoint::{is_adjoint_pair, AdjointPair, FiniteFunctor};
pub use kan_extension::{kan_extension_left, kan_extension_right};
pub use yoneda::{yoneda_embedding, yoneda_natural_iso, FiniteCategory};
