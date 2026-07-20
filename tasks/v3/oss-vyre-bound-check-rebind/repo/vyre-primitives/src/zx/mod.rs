//! ZX-calculus rewrite primitives (P-PRIM-5).
//!
//! ZX is a graphical language for linear maps. A ZX-diagram is an
//! undirected multigraph where each vertex (a "spider") carries
//! a color (Z or X) and a phase angle (mod 2π). The substrate
//! ships the rewrite rules the optimizer can apply for diagram
//! simplification:
//!
//! * spider fusion (S1): adjacent same-color spiders merge,
//!   summing their phases.
//! * identity removal (S2): a phase-0 spider with exactly two
//!   neighbors of the same color is dropped, splicing the edge.
//! * color change (H): conjugation by a Hadamard turns a Z-spider
//!   into an X-spider and vice versa.
//!
//! No floating-point  -  phases are stored as numerator over a
//! caller-chosen denominator (`phase_denom`). Two phases are equal
//! iff numerators agree mod `phase_denom`. Caller picks `phase_denom
//! = 8` for the Clifford+T fragment, `= 4` for Clifford-only, `= 2`
//! for the simplest stabilizer fragment.

pub mod rewrite;

pub use rewrite::{
    apply_color_change, apply_identity_removal, apply_spider_fusion, ZxColor, ZxDiagram, ZxSpider,
};
