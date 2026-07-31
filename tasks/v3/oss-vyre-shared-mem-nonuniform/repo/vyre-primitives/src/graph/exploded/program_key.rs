#[cfg(any(test, feature = "cpu-parity"))]
use super::layout::IfdsCsrProgramCacheKey;
#[cfg(any(test, feature = "cpu-parity"))]
use super::validation::max_ifds_col_count;
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_foundation::ir::{Expr, Node, Program};

/// Recover the exploded IFDS program cache key baked into a generated CSR
/// builder [`Program`].
///
/// Test and parity dispatchers use this to route GPU-shaped byte inputs through
/// the CPU reference without re-deriving dimensions from padded buffers alone.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn ifds_program_cache_key_from_program(
    program: &Program,
) -> Result<IfdsCsrProgramCacheKey, String> {
    let intra_count = loop_upper_bound(program, "intra_i")
        .ok_or_else(|| "Fix: exploded IFDS program missing intra_i loop bound.".to_string())?;
    let inter_count = loop_upper_bound(program, "inter_i")
        .ok_or_else(|| "Fix: exploded IFDS program missing inter_i loop bound.".to_string())?;
    let gen_count = loop_upper_bound(program, "gen_i")
        .ok_or_else(|| "Fix: exploded IFDS program missing gen_i loop bound.".to_string())?;
    let kill_count = loop_upper_bound(program, "kill_i")
        .ok_or_else(|| "Fix: exploded IFDS program missing kill_i loop bound.".to_string())?;
    let facts_per_proc = loop_upper_bound(program, "fact")
        .ok_or_else(|| "Fix: exploded IFDS program missing fact loop bound.".to_string())?;
    let total_nodes = loop_upper_bound(program, "prefix_row")
        .or_else(|| loop_upper_bound(program, "cursor_row"))
        .ok_or_else(|| "Fix: exploded IFDS program missing total_nodes loop bound.".to_string())?;

    let num_procs = upper_limit_for_var(program, "intra_p")
        .or_else(|| upper_limit_for_var(program, "inter_sp"))
        .ok_or_else(|| "Fix: exploded IFDS program missing num_procs bound.".to_string())?;
    let blocks_per_proc = upper_limit_for_var(program, "intra_dst_b")
        .or_else(|| upper_limit_for_var(program, "intra_src_b"))
        .or_else(|| upper_limit_for_var(program, "inter_sb"))
        .ok_or_else(|| "Fix: exploded IFDS program missing blocks_per_proc bound.".to_string())?;

    let slots_per_proc = blocks_per_proc
        .checked_mul(facts_per_proc)
        .ok_or_else(|| "Fix: exploded IFDS blocks*facts overflowed u32.".to_string())?;
    let expected_total = num_procs
        .checked_mul(slots_per_proc)
        .ok_or_else(|| "Fix: exploded IFDS procs*blocks*facts overflowed u32.".to_string())?;
    if expected_total != total_nodes {
        return Err(format!(
            "Fix: exploded IFDS program shape mismatch: procs={num_procs} blocks={blocks_per_proc} facts={facts_per_proc} implies total_nodes={expected_total}, program loop bound={total_nodes}."
        ));
    }

    let max_col_count = max_ifds_col_count(intra_count, inter_count, gen_count, facts_per_proc)
        .ok_or_else(|| "Fix: exploded IFDS maximum column count overflowed u32.".to_string())?;

    Ok(IfdsCsrProgramCacheKey {
        num_procs,
        blocks_per_proc,
        facts_per_proc,
        intra_count,
        inter_count,
        gen_count,
        kill_count,
        max_col_count,
    })
}

#[cfg(any(test, feature = "cpu-parity"))]
fn loop_upper_bound(program: &Program, var: &str) -> Option<u32> {
    use vyre_foundation::transform::visit::walk_nodes;

    let mut found: Option<u32> = None;
    walk_nodes(program, |node| {
        if let Node::Loop {
            var: loop_var, to, ..
        } = node
        {
            if loop_var.as_str() == var {
                if let Expr::LitU32(limit) = to {
                    found = Some(*limit);
                }
            }
        }
    });
    found
}

#[cfg(any(test, feature = "cpu-parity"))]
fn upper_limit_for_var(program: &Program, var: &str) -> Option<u32> {
    use vyre_foundation::ir::BinOp;
    use vyre_foundation::transform::visit::walk_exprs;

    let mut found: Option<u32> = None;
    walk_exprs(program, |expr| {
        if let Expr::BinOp {
            op: BinOp::Lt,
            left,
            right,
        } = expr
        {
            if let (Expr::Var(name), Expr::LitU32(limit)) = (left.as_ref(), right.as_ref()) {
                if name.as_str() == var {
                    found = Some(*limit);
                }
            }
        }
    });
    found
}
