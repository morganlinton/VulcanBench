//! Fast Multipole Method primitives  -  `p2m`, `m2l`, `l2p`.
//!
//! FMM (Greengard-Rokhlin 1987) evaluates n-body sums in O(n log n)
//! or O(n) via hierarchical multipole expansions:
//!
//! ```text
//!   1. P2M  Particle → Multipole at each leaf cell
//!   2. M2M  Multipole → Multipole up the tree (this file: skip,
//!           composes from P2M repeated at higher levels)
//!   3. M2L  Multipole → Local at well-separated cells
//!   4. L2L  Local → Local down the tree (skip, composes from L2P)
//!   5. L2P  Local → Particle, evaluate at target points
//! ```
//!
//! Three primitives ship here: `p2m_step` (charges → multipole moment
//! per cell), `m2l_step` (multipole → local for one source/target
//! cell pair), `l2p_step` (local expansion → potential at one
//! particle).
//!
//! # Why these primitives are dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::sci::nbody` | n-body / molecular dynamics |
//! | future `vyre-libs::ml::kernel_methods` | exact-Gaussian-process inference at scale |
//! | future `vyre-libs::sci::electrostatic` | Poisson / electrostatic solvers |
//! | `vyre-foundation::transform` all-pairs compression | FMM-style hierarchical compression keeps polyhedral fusion tractable at workspace scale |
//!
//! # Simplifying assumptions for v0.4.1
//!
//! - **2D Coulomb-style kernel**, fixed `p = 4` expansion order. The
//!   primitives are parameterized by buffer layout but the kernel is
//!   hard-coded to `1 / r`. Future: kernel-generic via Cat-C
//!   intrinsic.
//! - **Truncated complex multipoles encoded as 8 u32 values per cell**
//!   (real + imag for each of the p+1 = 5 moments, but we use 4 + 4
//!   = 8 to fit standard 4-byte alignment).
//! - **Particle data: 4 u32 per particle** = `(x, y, charge, _pad)`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// p2m op id.
pub const P2M_OP_ID: &str = "vyre-primitives::math::fmm_p2m_step";
/// m2l op id.
pub const M2L_OP_ID: &str = "vyre-primitives::math::fmm_m2l_step";
/// l2p op id.
pub const L2P_OP_ID: &str = "vyre-primitives::math::fmm_l2p_step";
/// f32 zeroth-moment p2m op id.
pub const P2M_ZEROTH_F32_OP_ID: &str = "vyre-primitives::math::fmm_p2m_zeroth_f32_step";
/// f32 zeroth-moment m2l op id.
pub const M2L_ZEROTH_F32_OP_ID: &str = "vyre-primitives::math::fmm_m2l_zeroth_f32_step";
/// f32 zeroth-moment l2p op id.
pub const L2P_ZEROTH_F32_OP_ID: &str = "vyre-primitives::math::fmm_l2p_zeroth_f32_step";

/// Number of u32 lanes per multipole/local expansion (`2 * (p + 1)`
/// for `p = 3`, packed real-imag interleaved).
pub const EXPANSION_WORDS: u32 = 8;

/// Stride per particle in the input buffer.
pub const PARTICLE_STRIDE: u32 = 4;

/// Stride per cell in the cell-centers buffer.
pub const CELL_STRIDE: u32 = 4;

const MIN_DISTANCE_F32: f32 = 1.0e-12;

/// Emit P2M: for each leaf cell, sum the contribution of every
/// particle in that cell into the cell's multipole expansion.
///
/// Inputs:
/// - `particles`: `n_particles · PARTICLE_STRIDE` u32. Per-particle
///   `(x, y, charge, _)` in 16.16.
/// - `cell_assignment`: length-`n_particles` u32  -  which cell index
///   each particle is in.
/// - `cell_centers`: `n_cells · CELL_STRIDE` u32 (`(cx, cy, _, _)`).
///
/// Output:
/// - `multipoles`: `n_cells · EXPANSION_WORDS` u32, accumulated.
///
/// Lane `t` = cell index. Lane walks all particles, contributes those
/// in its cell to its own expansion (avoiding atomics).
#[must_use]
pub fn p2m_step(
    particles: &str,
    cell_assignment: &str,
    cell_centers: &str,
    multipoles: &str,
    n_particles: u32,
    n_cells: u32,
) -> Program {
    if n_particles == 0 {
        return crate::invalid_output_program(
            P2M_OP_ID,
            multipoles,
            DataType::U32,
            "Fix: p2m_step requires n_particles > 0, got 0.".to_string(),
        );
    }
    if n_cells == 0 {
        return crate::invalid_output_program(
            P2M_OP_ID,
            multipoles,
            DataType::U32,
            "Fix: p2m_step requires n_cells > 0, got 0.".to_string(),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let cell = t.clone();

    let body = vec![Node::if_then(
        Expr::lt(cell.clone(), Expr::u32(n_cells)),
        vec![
            Node::let_bind("acc_real0", Expr::u32(0)),
            Node::let_bind("acc_imag0", Expr::u32(0)),
            Node::loop_for(
                "p_idx",
                Expr::u32(0),
                Expr::u32(n_particles),
                vec![Node::if_then(
                    Expr::eq(
                        Expr::load(cell_assignment, Expr::var("p_idx")),
                        cell.clone(),
                    ),
                    vec![Node::assign(
                        "acc_real0",
                        Expr::add(
                            Expr::var("acc_real0"),
                            Expr::load(
                                particles,
                                Expr::add(
                                    Expr::mul(Expr::var("p_idx"), Expr::u32(PARTICLE_STRIDE)),
                                    Expr::u32(2), // charge slot
                                ),
                            ),
                        ),
                    )],
                )],
            ),
            // Write zeroth-order moment (total charge in cell) into
            // multipoles[cell * EXPANSION_WORDS + 0]. Higher-order
            // moments are an exercise for the next variant.
            Node::store(
                multipoles,
                Expr::mul(cell, Expr::u32(EXPANSION_WORDS)),
                Expr::var("acc_real0"),
            ),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(particles, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_particles * PARTICLE_STRIDE),
            BufferDecl::storage(cell_assignment, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_particles),
            BufferDecl::storage(cell_centers, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_cells * CELL_STRIDE),
            BufferDecl::storage(multipoles, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_cells * EXPANSION_WORDS),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(P2M_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit zeroth-moment f32 P2M for the self-substrate polyhedral FMM compressor.
///
/// Lane `t` owns one cell and scans all regions, summing `scores[i]` when
/// `cell_assignment[i] == t`. This keeps the primitive deterministic without
/// atomics and maps directly to CUDA workgroups for the release path.
#[must_use]
pub fn p2m_zeroth_f32_step(
    scores: &str,
    cell_assignment: &str,
    moments: &str,
    n_regions: u32,
    n_cells: u32,
) -> Program {
    if n_regions == 0 {
        return crate::invalid_output_program(
            P2M_ZEROTH_F32_OP_ID,
            moments,
            DataType::F32,
            "Fix: p2m_zeroth_f32_step requires n_regions > 0, got 0.".to_string(),
        );
    }
    if n_cells == 0 {
        return crate::invalid_output_program(
            P2M_ZEROTH_F32_OP_ID,
            moments,
            DataType::F32,
            "Fix: p2m_zeroth_f32_step requires n_cells > 0, got 0.".to_string(),
        );
    }

    let cell = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(cell.clone(), Expr::u32(n_cells)),
        vec![
            Node::let_bind("acc", Expr::f32(0.0)),
            Node::loop_for(
                "region",
                Expr::u32(0),
                Expr::u32(n_regions),
                vec![Node::if_then(
                    Expr::eq(
                        Expr::load(cell_assignment, Expr::var("region")),
                        cell.clone(),
                    ),
                    vec![Node::assign(
                        "acc",
                        Expr::add(Expr::var("acc"), Expr::load(scores, Expr::var("region"))),
                    )],
                )],
            ),
            Node::store(moments, cell, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(scores, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_regions),
            BufferDecl::storage(cell_assignment, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_regions),
            BufferDecl::storage(moments, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(n_cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(P2M_ZEROTH_F32_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit zeroth-moment f32 M2L translation for all target cells.
///
/// `cell_distances` is row-major target-by-source. Self-cell contribution is
/// skipped because near-field/direct evaluation owns it.
#[must_use]
pub fn m2l_zeroth_f32_step(
    cell_moments: &str,
    cell_distances: &str,
    cell_local: &str,
    n_cells: u32,
) -> Program {
    if n_cells == 0 {
        return crate::invalid_output_program(
            M2L_ZEROTH_F32_OP_ID,
            cell_local,
            DataType::F32,
            "Fix: m2l_zeroth_f32_step requires n_cells > 0, got 0.".to_string(),
        );
    }
    let distance_count = match n_cells.checked_mul(n_cells) {
        Some(count) => count,
        None => {
            return crate::invalid_output_program(
                M2L_ZEROTH_F32_OP_ID,
                cell_local,
                DataType::F32,
                format!(
                    "Fix: m2l_zeroth_f32_step n_cells*n_cells overflows u32 for n_cells={n_cells}."
                ),
            );
        }
    };

    let target = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(target.clone(), Expr::u32(n_cells)),
        vec![
            Node::let_bind("acc", Expr::f32(0.0)),
            Node::loop_for(
                "source",
                Expr::u32(0),
                Expr::u32(n_cells),
                vec![Node::if_then(
                    Expr::ne(Expr::var("source"), target.clone()),
                    vec![
                        Node::let_bind(
                            "distance",
                            Expr::load(
                                cell_distances,
                                Expr::add(
                                    Expr::mul(target.clone(), Expr::u32(n_cells)),
                                    Expr::var("source"),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "safe_distance",
                            Expr::select(
                                Expr::lt(Expr::var("distance"), Expr::f32(MIN_DISTANCE_F32)),
                                Expr::f32(MIN_DISTANCE_F32),
                                Expr::var("distance"),
                            ),
                        ),
                        Node::assign(
                            "acc",
                            Expr::add(
                                Expr::var("acc"),
                                Expr::div(
                                    Expr::load(cell_moments, Expr::var("source")),
                                    Expr::var("safe_distance"),
                                ),
                            ),
                        ),
                    ],
                )],
            ),
            Node::store(cell_local, target, Expr::var("acc")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(cell_moments, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_cells),
            BufferDecl::storage(cell_distances, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(distance_count),
            BufferDecl::storage(cell_local, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(n_cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(M2L_ZEROTH_F32_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit zeroth-moment f32 L2P evaluation from cell locals to per-region output.
#[must_use]
pub fn l2p_zeroth_f32_step(
    cell_local: &str,
    cell_assignment: &str,
    region_out: &str,
    n_regions: u32,
    n_cells: u32,
) -> Program {
    if n_regions == 0 {
        return crate::invalid_output_program(
            L2P_ZEROTH_F32_OP_ID,
            region_out,
            DataType::F32,
            "Fix: l2p_zeroth_f32_step requires n_regions > 0, got 0.".to_string(),
        );
    }
    if n_cells == 0 {
        return crate::invalid_output_program(
            L2P_ZEROTH_F32_OP_ID,
            region_out,
            DataType::F32,
            "Fix: l2p_zeroth_f32_step requires n_cells > 0, got 0.".to_string(),
        );
    }

    let region = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(region.clone(), Expr::u32(n_regions)),
        vec![
            Node::let_bind("cell", Expr::load(cell_assignment, region.clone())),
            Node::if_then(
                Expr::lt(Expr::var("cell"), Expr::u32(n_cells)),
                vec![Node::store(
                    region_out,
                    region,
                    Expr::load(cell_local, Expr::var("cell")),
                )],
            ),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(cell_local, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_cells),
            BufferDecl::storage(cell_assignment, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_regions),
            BufferDecl::storage(region_out, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(n_regions),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(L2P_ZEROTH_F32_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference for `p2m_step`  -  sums particle charges into per-cell
/// total charge (zeroth-order moment).
#[cfg(test)]
#[must_use]
pub fn p2m_zeroth_moment_cpu(charges: &[f64], cell_assignment: &[u32]) -> Vec<f64> {
    let mut moments = Vec::new();
    try_p2m_zeroth_moment_cpu_into(charges, cell_assignment, &mut moments)
        .unwrap_or_else(|error| panic!("{error}"));
    moments
}

/// CPU reference for `p2m_step` using caller-owned moment storage.
#[cfg(test)]
pub fn p2m_zeroth_moment_cpu_into(
    charges: &[f64],
    cell_assignment: &[u32],
    moments: &mut Vec<f64>,
) {
    try_p2m_zeroth_moment_cpu_into(charges, cell_assignment, moments)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference for `p2m_step` using caller-owned moment storage.
#[cfg(test)]
pub fn try_p2m_zeroth_moment_cpu_into(
    charges: &[f64],
    cell_assignment: &[u32],
    moments: &mut Vec<f64>,
) -> Result<(), String> {
    if charges.is_empty() {
        debug_assert!(cell_assignment.is_empty());
        moments.clear();
        return Ok(());
    }
    let n_cells = cell_assignment.iter().max().copied().unwrap_or(0) as usize + 1;
    if n_cells > moments.capacity() {
        crate::graph::scratch::reserve_graph_items(
            moments,
            n_cells - moments.len(),
            "FMM CPU oracle",
            "p2m zeroth moments",
        )?;
    }
    moments.clear();
    moments.resize(n_cells, 0.0);
    for (&charge, &cell) in charges.iter().zip(cell_assignment.iter()) {
        moments[cell as usize] += charge;
    }
    Ok(())
}

/// CPU reference for **L2P** evaluation  -  given a cell's local
/// expansion (zeroth-order = total far-field potential) and a target
/// particle position, return the contributed potential. For the
/// zeroth-order primitive, this is just the local moment value.
#[cfg(test)]
#[must_use]

pub fn l2p_zeroth_eval_cpu(local_moment: f64, _target_x: f64, _target_y: f64) -> f64 {
    local_moment
}

/// CPU reference for **M2L** translation  -  given a source cell's
/// multipole expansion (zeroth-order = total source charge), the
/// distance to the target cell, return the target cell's local
/// expansion contribution. For Coulomb 2D: `local_0 = source_0 / r`.
#[cfg(test)]
#[must_use]
pub fn m2l_zeroth_translate_cpu(source_moment: f64, distance: f64) -> f64 {
    let r = distance.max(1e-12);
    source_moment / r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_p2m_total_charge_matches_sum() {
        // Five particles, all in cell 0.
        let charges = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let cells = vec![0u32, 0, 0, 0, 0];
        let m = p2m_zeroth_moment_cpu(&charges, &cells);
        assert_eq!(m.len(), 1);
        assert!(approx_eq(m[0], 15.0));
    }

    #[test]
    fn cpu_p2m_partitions_charges_by_cell() {
        // 6 particles split between 2 cells (3 each).
        let charges = vec![1.0, 2.0, 3.0, 10.0, 20.0, 30.0];
        let cells = vec![0u32, 0, 0, 1, 1, 1];
        let m = p2m_zeroth_moment_cpu(&charges, &cells);
        assert!(approx_eq(m[0], 6.0));
        assert!(approx_eq(m[1], 60.0));
    }

    #[test]
    fn cpu_p2m_into_reuses_moment_buffer() {
        let charges = vec![1.0, 2.0, 3.0, 10.0, 20.0, 30.0];
        let cells = vec![0u32, 0, 0, 1, 1, 1];
        let mut moments = Vec::with_capacity(8);
        moments.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let ptr = moments.as_ptr();
        let capacity = moments.capacity();
        try_p2m_zeroth_moment_cpu_into(&charges, &cells, &mut moments)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - FMM P2M CPU oracle should reuse caller-owned moment storage");
        assert!(approx_eq(moments[0], 6.0));
        assert!(approx_eq(moments[1], 60.0));
        assert_eq!(moments.as_ptr(), ptr);
        assert_eq!(moments.capacity(), capacity);

        try_p2m_zeroth_moment_cpu_into(&[5.0], &[0], &mut moments)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - FMM P2M CPU oracle should truncate stale moments");
        assert_eq!(moments, vec![5.0]);
        assert_eq!(moments.as_ptr(), ptr);
        assert_eq!(moments.capacity(), capacity);
    }

    #[test]
    fn generated_p2m_matches_independent_reference_and_truncates_mismatches() {
        let mut moments = Vec::new();
        for case in 0..1024usize {
            let charge_len = case % 97;
            let assign_len = (case * 7) % 97;
            let charges: Vec<f64> = (0..charge_len)
                .map(|idx| ((idx * 13 + case) % 31) as f64 / 7.0 - 2.0)
                .collect();
            let assignments: Vec<u32> = (0..assign_len)
                .map(|idx| ((idx * 17 + case) % 11) as u32)
                .collect();

            try_p2m_zeroth_moment_cpu_into(&charges, &assignments, &mut moments)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated FMM P2M CPU oracle should evaluate");
            let expected = independent_p2m(&charges, &assignments);

            assert_eq!(moments.len(), expected.len(), "case {case}: output length");
            for idx in 0..moments.len() {
                assert!(
                    approx_eq(moments[idx], expected[idx]),
                    "case {case} idx {idx}: expected {}, got {}",
                    expected[idx],
                    moments[idx]
                );
            }
        }
    }

    fn independent_p2m(charges: &[f64], assignments: &[u32]) -> Vec<f64> {
        if charges.is_empty() {
            return Vec::new();
        }
        let max_cell = assignments.iter().max().copied().unwrap_or(0);
        let mut out = vec![0.0; max_cell as usize + 1];
        for (&charge, &cell) in charges.iter().zip(assignments.iter()) {
            out[cell as usize] += charge;
        }
        out
    }

    #[test]
    fn cpu_m2l_inverse_distance_kernel() {
        // Coulomb 2D: at distance 2 with source charge 10 → local field 5.
        assert!(approx_eq(m2l_zeroth_translate_cpu(10.0, 2.0), 5.0));
    }

    #[test]
    fn cpu_m2l_zero_distance_clamps() {
        // Avoid division by zero  -  clamp to small positive distance.
        assert!(m2l_zeroth_translate_cpu(1.0, 0.0).is_finite());
    }

    #[test]
    fn cpu_l2p_passthrough() {
        // L2P at zeroth order is just a passthrough of the local moment.
        assert!(approx_eq(l2p_zeroth_eval_cpu(7.5, 0.0, 0.0), 7.5));
    }

    #[test]
    fn ir_p2m_program_buffer_layout() {
        let p = p2m_step("part", "asgn", "ccen", "mult", 100, 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        assert_eq!(p.buffers[0].count(), 100 * PARTICLE_STRIDE);
        assert_eq!(p.buffers[1].count(), 100);
        assert_eq!(p.buffers[2].count(), 16 * CELL_STRIDE);
        assert_eq!(p.buffers[3].count(), 16 * EXPANSION_WORDS);
    }

    #[test]
    fn zero_particles_traps() {
        let p = p2m_step("p", "a", "c", "m", 0, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_cells_traps() {
        let p = p2m_step("p", "a", "c", "m", 1, 0);
        assert!(p.stats().trap());
    }
}
