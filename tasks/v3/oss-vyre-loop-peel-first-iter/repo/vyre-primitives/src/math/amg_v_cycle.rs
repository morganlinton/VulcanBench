//! Algebraic Multigrid (AMG) V-cycle primitive (#P-PRIM-3).
//!
//! Composes `jacobi_smooth_step` with restriction and prolongation to
//! solve linear systems $Ax = b$ across multiple scales.
//!
//! Sequence (2-level):
//! 1. Pre-smooth: $x = \text{smooth}(A, b, x, \omega)$
//! 2. Restrict: $r = b - Ax$; $b_c = R r$
//! 3. Coarse solve: $x_c = \text{solve}(A_c, b_c)$ (via Jacobi for this primitive)
//! 4. Prolong: $x = x + P x_c$
//! 5. Post-smooth: $x = \text{smooth}(A, b, x, \omega)$

use crate::math::multigrid::jacobi_smooth_step;
#[cfg(any(test, feature = "cpu-parity"))]
use crate::math::multigrid::jacobi_smooth_step_cpu_into;
use std::sync::Arc;
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::amg_v_cycle";
const V_CYCLE_PHASE_OP_ID: &str = "vyre-primitives::math::amg_v_cycle::v_cycle_phase";

/// Build an AMG V-cycle Program.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn amg_v_cycle(
    a: &str,
    b: &str,
    x: &str,
    r_mat: &str,
    p_mat: &str,
    a_c: &str,
    omega: &str,
    scratch_fine: &str,
    scratch_coarse_b: &str,
    scratch_coarse_x: &str,
    n_fine: u32,
    n_coarse: u32,
) -> Program {
    if n_fine == 0 {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            "Fix: amg_v_cycle requires n_fine > 0, got 0.".to_string(),
        );
    }
    if n_coarse == 0 {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            "Fix: amg_v_cycle requires n_coarse > 0, got 0.".to_string(),
        );
    }
    if n_coarse >= n_fine {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            format!("Fix: amg_v_cycle requires n_coarse < n_fine, got n_coarse={n_coarse}, n_fine={n_fine}."),
        );
    }
    let Some(fine_cells) = n_fine.checked_mul(n_fine) else {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            format!("Fix: amg_v_cycle fine matrix cells overflow u32: n_fine={n_fine}."),
        );
    };
    let Some(transfer_cells) = n_fine.checked_mul(n_coarse) else {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            format!(
                "Fix: amg_v_cycle transfer matrix cells overflow u32: n_fine={n_fine}, n_coarse={n_coarse}."
            ),
        );
    };
    let Some(coarse_cells) = n_coarse.checked_mul(n_coarse) else {
        return crate::invalid_output_program(
            OP_ID,
            x,
            DataType::U32,
            format!("Fix: amg_v_cycle coarse matrix cells overflow u32: n_coarse={n_coarse}."),
        );
    };

    let mut nodes = Vec::new();

    // 1. Pre-smooth
    let pre_smooth = jacobi_smooth_step(a, b, x, omega, scratch_fine, n_fine);
    nodes.extend(pre_smooth.entry().to_vec());
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));
    // Copy scratch_fine back to x
    nodes.push(Node::loop_for(
        "__i",
        Expr::u32(0),
        Expr::u32(n_fine),
        vec![Node::store(
            x,
            Expr::var("__i"),
            Expr::load(scratch_fine, Expr::var("__i")),
        )],
    ));
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));

    // 2. Restrict: r = b - Ax; b_c = R r
    nodes.push(Node::loop_for(
        "i",
        Expr::u32(0),
        Expr::u32(n_fine),
        vec![
            Node::let_bind("ax_i", Expr::u32(0)),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(n_fine),
                vec![Node::assign(
                    "ax_i",
                    Expr::add(
                        Expr::var("ax_i"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(
                                a,
                                Expr::add(
                                    Expr::mul(Expr::var("i"), Expr::u32(n_fine)),
                                    Expr::var("j"),
                                ),
                            ),
                            Expr::load(x, Expr::var("j")),
                        ),
                    ),
                )],
            ),
            Node::store(
                scratch_fine,
                Expr::var("i"),
                Expr::sub(Expr::load(b, Expr::var("i")), Expr::var("ax_i")),
            ),
        ],
    ));
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));

    // b_c = R * r
    nodes.push(Node::loop_for(
        "ic",
        Expr::u32(0),
        Expr::u32(n_coarse),
        vec![
            Node::let_bind("bc_i", Expr::u32(0)),
            Node::loop_for(
                "jf",
                Expr::u32(0),
                Expr::u32(n_fine),
                vec![Node::assign(
                    "bc_i",
                    Expr::add(
                        Expr::var("bc_i"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(
                                r_mat,
                                Expr::add(
                                    Expr::mul(Expr::var("ic"), Expr::u32(n_fine)),
                                    Expr::var("jf"),
                                ),
                            ),
                            Expr::load(scratch_fine, Expr::var("jf")),
                        ),
                    ),
                )],
            ),
            Node::store(scratch_coarse_b, Expr::var("ic"), Expr::var("bc_i")),
        ],
    ));
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));

    // 3. Coarse solve
    nodes.push(Node::store(scratch_coarse_x, Expr::u32(0), Expr::u32(0)));
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));
    for _ in 0..4 {
        let coarse_smooth = jacobi_smooth_step(
            a_c,
            scratch_coarse_b,
            scratch_coarse_x,
            omega,
            "temp_coarse",
            n_coarse,
        );
        nodes.extend(coarse_smooth.entry().to_vec());
        nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));
        nodes.push(Node::loop_for(
            "__k",
            Expr::u32(0),
            Expr::u32(n_coarse),
            vec![Node::store(
                scratch_coarse_x,
                Expr::var("__k"),
                Expr::load("temp_coarse", Expr::var("__k")),
            )],
        ));
        nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));
    }

    // 4. Prolong: x = x + P * x_c
    nodes.push(Node::loop_for(
        "if",
        Expr::u32(0),
        Expr::u32(n_fine),
        vec![
            Node::let_bind("px_i", Expr::u32(0)),
            Node::loop_for(
                "jc",
                Expr::u32(0),
                Expr::u32(n_coarse),
                vec![Node::assign(
                    "px_i",
                    Expr::add(
                        Expr::var("px_i"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(
                                p_mat,
                                Expr::add(
                                    Expr::mul(Expr::var("if"), Expr::u32(n_coarse)),
                                    Expr::var("jc"),
                                ),
                            ),
                            Expr::load(scratch_coarse_x, Expr::var("jc")),
                        ),
                    ),
                )],
            ),
            Node::store(
                x,
                Expr::var("if"),
                Expr::add(Expr::load(x, Expr::var("if")), Expr::var("px_i")),
            ),
        ],
    ));
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));

    // 5. Post-smooth
    let post_smooth = jacobi_smooth_step(a, b, x, omega, scratch_fine, n_fine);
    nodes.extend(post_smooth.entry().to_vec());
    nodes.push(Node::barrier_with_ordering(MemoryOrdering::GridSync));
    nodes.push(Node::loop_for(
        "__m",
        Expr::u32(0),
        Expr::u32(n_fine),
        vec![Node::store(
            x,
            Expr::var("__m"),
            Expr::load(scratch_fine, Expr::var("__m")),
        )],
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(fine_cells),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n_fine),
            BufferDecl::storage(x, 2, BufferAccess::ReadWrite, DataType::U32).with_count(n_fine),
            BufferDecl::storage(r_mat, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(transfer_cells),
            BufferDecl::storage(p_mat, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(transfer_cells),
            BufferDecl::storage(a_c, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(coarse_cells),
            BufferDecl::storage(omega, 6, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(scratch_fine, 7, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_fine),
            BufferDecl::storage(scratch_coarse_b, 8, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_coarse),
            BufferDecl::storage(scratch_coarse_x, 9, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_coarse),
            BufferDecl::storage("temp_coarse", 10, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_coarse),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::Region {
                generator: Ident::from(V_CYCLE_PHASE_OP_ID),
                source_region: Some(GeneratorRef {
                    name: OP_ID.to_string(),
                }),
                body: Arc::new(nodes),
            }]),
        }],
    )
}

/// CPU reference: 2-level AMG V-cycle in f64.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn cpu_ref(
    a: &[f64],
    b: &[f64],
    x: &[f64],
    r_mat: &[f64],
    p_mat: &[f64],
    a_c: &[f64],
    omega: f64,
    n_fine: u32,
    n_coarse: u32,
) -> Vec<f64> {
    try_cpu_ref(a, b, x, r_mat, p_mat, a_c, omega, n_fine, n_coarse)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - amg_v_cycle cpu_ref failed: invalid V-cycle dimensions")
}

/// Fallible CPU reference: 2-level AMG V-cycle in f64.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_cpu_ref(
    a: &[f64],
    b: &[f64],
    x: &[f64],
    r_mat: &[f64],
    p_mat: &[f64],
    a_c: &[f64],
    omega: f64,
    n_fine: u32,
    n_coarse: u32,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    let mut scratch = AmgVcycleScratch::default();
    try_cpu_ref_into(
        a,
        b,
        x,
        r_mat,
        p_mat,
        a_c,
        omega,
        n_fine,
        n_coarse,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Reusable scratch for [`cpu_ref_into`].
#[derive(Debug, Default, Clone)]
#[cfg(any(test, feature = "cpu-parity"))]
pub struct AmgVcycleScratch {
    x_curr: Vec<f64>,
    residual: Vec<f64>,
    coarse_rhs: Vec<f64>,
    coarse_x: Vec<f64>,
    coarse_next: Vec<f64>,
}

/// CPU reference: 2-level AMG V-cycle in f64, writing into caller-owned storage.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    a: &[f64],
    b: &[f64],
    x: &[f64],
    r_mat: &[f64],
    p_mat: &[f64],
    a_c: &[f64],
    omega: f64,
    n_fine: u32,
    n_coarse: u32,
    scratch: &mut AmgVcycleScratch,
    out: &mut Vec<f64>,
) {
    try_cpu_ref_into(
        a, b, x, r_mat, p_mat, a_c, omega, n_fine, n_coarse, scratch, out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - amg_v_cycle cpu_ref_into failed: invalid V-cycle dimensions");
}

/// Fallible CPU reference: 2-level AMG V-cycle in f64, writing into caller-owned storage.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    a: &[f64],
    b: &[f64],
    x: &[f64],
    r_mat: &[f64],
    p_mat: &[f64],
    a_c: &[f64],
    omega: f64,
    n_fine: u32,
    n_coarse: u32,
    scratch: &mut AmgVcycleScratch,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let (nf, nc, fine_cells, transfer_cells, coarse_cells) =
        checked_cpu_v_cycle_sizes(n_fine, n_coarse)?;
    require_len("a", a.len(), fine_cells)?;
    require_len("b", b.len(), nf)?;
    require_len("x", x.len(), nf)?;
    require_len("r_mat", r_mat.len(), transfer_cells)?;
    require_len("p_mat", p_mat.len(), transfer_cells)?;
    require_len("a_c", a_c.len(), coarse_cells)?;

    jacobi_smooth_step_cpu_into(a, b, x, omega, n_fine, &mut scratch.x_curr);

    reserve_cpu_scratch(&mut scratch.residual, nf, "residual")?;
    scratch.residual.resize(nf, 0.0);
    for i in 0..nf {
        let mut ax_i = 0.0;
        for j in 0..nf {
            ax_i += a[i * nf + j] * scratch.x_curr[j];
        }
        scratch.residual[i] = b[i] - ax_i;
    }

    reserve_cpu_scratch(&mut scratch.coarse_rhs, nc, "coarse_rhs")?;
    scratch.coarse_rhs.resize(nc, 0.0);
    for i in 0..nc {
        for j in 0..nf {
            scratch.coarse_rhs[i] += r_mat[i * nf + j] * scratch.residual[j];
        }
    }

    reserve_cpu_scratch(&mut scratch.coarse_x, nc, "coarse_x")?;
    scratch.coarse_x.resize(nc, 0.0);
    reserve_cpu_scratch(&mut scratch.coarse_next, nc, "coarse_next")?;
    scratch.coarse_next.clear();
    for _ in 0..4 {
        jacobi_smooth_step_cpu_into(
            a_c,
            &scratch.coarse_rhs,
            &scratch.coarse_x,
            omega,
            n_coarse,
            &mut scratch.coarse_next,
        );
        std::mem::swap(&mut scratch.coarse_x, &mut scratch.coarse_next);
    }

    for i in 0..nf {
        let mut px_i = 0.0;
        for j in 0..nc {
            px_i += p_mat[i * nc + j] * scratch.coarse_x[j];
        }
        scratch.x_curr[i] += px_i;
    }

    jacobi_smooth_step_cpu_into(a, b, &scratch.x_curr, omega, n_fine, out);
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]

fn checked_cpu_v_cycle_sizes(
    n_fine: u32,
    n_coarse: u32,
) -> Result<(usize, usize, usize, usize, usize), String> {
    if n_fine == 0 {
        return Err(
            "amg_v_cycle CPU oracle requires n_fine > 0. Fix: provide a non-empty fine grid."
                .to_string(),
        );
    }
    if n_coarse == 0 {
        return Err(
            "amg_v_cycle CPU oracle requires n_coarse > 0. Fix: provide a non-empty coarse grid."
                .to_string(),
        );
    }
    if n_coarse >= n_fine {
        return Err(format!("amg_v_cycle CPU oracle requires n_coarse < n_fine, got n_coarse={n_coarse}, n_fine={n_fine}."));
    }
    let nf = usize::try_from(n_fine)
        .map_err(|_| format!("amg_v_cycle CPU oracle n_fine={n_fine} does not fit usize."))?;
    let nc = usize::try_from(n_coarse)
        .map_err(|_| format!("amg_v_cycle CPU oracle n_coarse={n_coarse} does not fit usize."))?;
    let fine_cells = nf.checked_mul(nf).ok_or_else(|| {
        format!("amg_v_cycle CPU oracle fine matrix cells overflow: n_fine={n_fine}.")
    })?;
    let transfer_cells = nf.checked_mul(nc).ok_or_else(|| {
        format!("amg_v_cycle CPU oracle transfer matrix cells overflow: n_fine={n_fine}, n_coarse={n_coarse}.")
    })?;
    let coarse_cells = nc.checked_mul(nc).ok_or_else(|| {
        format!("amg_v_cycle CPU oracle coarse matrix cells overflow: n_coarse={n_coarse}.")
    })?;
    Ok((nf, nc, fine_cells, transfer_cells, coarse_cells))
}

#[cfg(any(test, feature = "cpu-parity"))]
fn require_len(name: &str, got: usize, need: usize) -> Result<(), String> {
    if got < need {
        Err(format!(
            "amg_v_cycle CPU oracle buffer `{name}` is too short: got {got}, need {need}. Fix: pass the full dense V-cycle buffer."
        ))
    } else {
        Ok(())
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_cpu_scratch(out: &mut Vec<f64>, len: usize, name: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "AMG V-cycle CPU oracle",
            name,
        )?;
    }
    out.clear();
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || amg_v_cycle("a", "b", "x", "r", "p", "ac", "om", "sf", "scb", "scx", 4, 2),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[0; 16]), // a
                to_bytes(&[0; 4]),  // b
                to_bytes(&[0; 4]),  // x
                to_bytes(&[0; 8]),  // r
                to_bytes(&[0; 8]),  // p
                to_bytes(&[0; 4]),  // ac
                to_bytes(&[0]),     // om
                to_bytes(&[0; 4]),  // sf
                to_bytes(&[0; 2]),  // scb
                to_bytes(&[0; 2]),  // scx
                to_bytes(&[0; 2]),  // temp_coarse
            ]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[0; 4]), // x
                to_bytes(&[0; 4]), // sf
                to_bytes(&[0; 2]), // scb
                to_bytes(&[0; 2]), // scx
                to_bytes(&[0; 2]), // temp_coarse
            ]]
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        V_CYCLE_PHASE_OP_ID,
        || {
            Program::wrapped(
                vec![
                    BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                        .with_count(1),
                    BufferDecl::output("out", 1, DataType::U32).with_count(1),
                ],
                [1, 1, 1],
                vec![Node::Region {
                    generator: Ident::from(V_CYCLE_PHASE_OP_ID),
                    source_region: None,
                    body: Arc::new(vec![Node::store(
                        "out",
                        Expr::u32(0),
                        Expr::load("input", Expr::u32(0)),
                    )]),
                }],
            )
        },
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![to_bytes(&[9]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![to_bytes(&[9])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_identity_holds() {
        let n_fine = 4;
        let n_coarse = 2;
        let a = vec![
            2.0, -1.0, 0.0, 0.0, -1.0, 2.0, -1.0, 0.0, 0.0, -1.0, 2.0, -1.0, 0.0, 0.0, -1.0, 2.0,
        ];
        let b = vec![1.0, 0.0, 0.0, 1.0];
        let x = vec![0.0; 4];
        let r_mat = vec![1.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0, 0.5];
        let p_mat = vec![1.0, 0.0, 0.5, 0.5, 0.0, 1.0, 0.0, 0.5];
        let a_c = vec![2.0, -0.5, -0.5, 2.0];
        let omega = 2.0 / 3.0;

        let x_out = cpu_ref(&a, &b, &x, &r_mat, &p_mat, &a_c, omega, n_fine, n_coarse);
        assert_eq!(x_out.len(), 4);
    }

    #[test]
    fn cpu_ref_into_reuses_output_and_scratch_storage() {
        let n_fine = 4;
        let n_coarse = 2;
        let a = vec![
            2.0, -1.0, 0.0, 0.0, -1.0, 2.0, -1.0, 0.0, 0.0, -1.0, 2.0, -1.0, 0.0, 0.0, -1.0, 2.0,
        ];
        let b = vec![1.0, 0.0, 0.0, 1.0];
        let x = vec![0.0; 4];
        let r_mat = vec![1.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0, 0.5];
        let p_mat = vec![1.0, 0.0, 0.5, 0.5, 0.0, 1.0, 0.0, 0.5];
        let a_c = vec![2.0, -0.5, -0.5, 2.0];
        let omega = 2.0 / 3.0;
        let mut scratch = AmgVcycleScratch::default();
        let mut out = Vec::with_capacity(8);

        try_cpu_ref_into(
            &a,
            &b,
            &x,
            &r_mat,
            &p_mat,
            &a_c,
            omega,
            n_fine,
            n_coarse,
            &mut scratch,
            &mut out,
        )
        .unwrap();
        let out_ptr = out.as_ptr();
        let residual_ptr = scratch.residual.as_ptr();
        let first = out.clone();
        out.extend([99.0; 4]);
        try_cpu_ref_into(
            &a,
            &b,
            &x,
            &r_mat,
            &p_mat,
            &a_c,
            omega,
            n_fine,
            n_coarse,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(out, first);
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.residual.as_ptr(), residual_ptr);
    }

    #[test]
    fn try_cpu_ref_rejects_short_dense_inputs() {
        let err = try_cpu_ref(
            &[1.0],
            &[1.0, 2.0],
            &[0.0, 0.0],
            &[1.0, 0.0],
            &[1.0, 0.0],
            &[1.0],
            1.0,
            2,
            1,
        )
        .unwrap_err();
        assert!(err.contains("buffer `a` is too short"), "{err}");
    }

    #[test]
    fn generated_cpu_ref_matches_reusable_path() {
        for case in 0..24 {
            let n_fine = 3 + (case % 3);
            let n_coarse = 1 + (case % (n_fine - 1));
            let nf = n_fine as usize;
            let nc = n_coarse as usize;
            let mut a = vec![0.0; nf * nf];
            for i in 0..nf {
                a[i * nf + i] = 2.0 + case as f64 * 0.01;
                if i + 1 < nf {
                    a[i * nf + i + 1] = -0.25;
                    a[(i + 1) * nf + i] = -0.25;
                }
            }
            let b: Vec<f64> = (0..nf).map(|i| 1.0 + i as f64 * 0.125).collect();
            let x: Vec<f64> = (0..nf).map(|i| i as f64 * 0.01).collect();
            let mut r_mat = vec![0.0; nc * nf];
            let mut p_mat = vec![0.0; nf * nc];
            for i in 0..nc {
                r_mat[i * nf + (i * nf / nc)] = 1.0;
            }
            for i in 0..nf {
                p_mat[i * nc + (i * nc / nf).min(nc - 1)] = 1.0;
            }
            let mut a_c = vec![0.0; nc * nc];
            for i in 0..nc {
                a_c[i * nc + i] = 1.5 + case as f64 * 0.01;
            }
            let expected = cpu_ref(&a, &b, &x, &r_mat, &p_mat, &a_c, 0.5, n_fine, n_coarse);
            let mut scratch = AmgVcycleScratch::default();
            let mut out = Vec::with_capacity(expected.len() + 3);

            try_cpu_ref_into(
                &a,
                &b,
                &x,
                &r_mat,
                &p_mat,
                &a_c,
                0.5,
                n_fine,
                n_coarse,
                &mut scratch,
                &mut out,
            )
            .unwrap();

            assert_eq!(out.len(), expected.len(), "case {case}");
            for (idx, (&actual, &want)) in out.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (actual - want).abs() < 1e-10,
                    "case {case} idx {idx}: expected {want}, got {actual}"
                );
            }
        }
    }

    #[test]
    fn program_has_correct_buffers() {
        let p = amg_v_cycle(
            "a", "b", "x", "r", "p", "ac", "om", "sf", "scb", "scx", 4, 2,
        );
        assert_eq!(p.buffers().len(), 11);
    }
}
