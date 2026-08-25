//! Tier 2.5 geometric-algebra primitives (#8).
//!
//! Geometric (Clifford) algebra unifies vectors, quaternions, dual
//! quaternions, projective transformations, spinors. Recent ML work
//! (Brandstetter 2022 CGENN, Ruhe 2023 Clifford GNN, Spellings 2021
//! GTN) shows it as the substrate for equivariant networks, physics
//! simulation, robotics, 3D vision.
//!
//! Multivector products are structured shuffles + fused multiply-add  -
//! identical hardware to matmul, so packaging this as a primitive
//! makes equivariance work GPU-native for the first time at the
//! IR level.

/// Clifford / geometric product on Cl(2, 0) multivectors (4-component
/// scalar / e1 / e2 / e12).
pub mod clifford;

/// SE(3)-equivariant tensor field network scalar (l=0) channel mix
/// step (#33). User: equivariant NN, molecular dynamics, cryo-EM.
pub mod tfn;
