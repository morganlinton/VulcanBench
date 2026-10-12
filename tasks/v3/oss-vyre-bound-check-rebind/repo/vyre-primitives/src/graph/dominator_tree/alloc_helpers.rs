pub(super) fn resize_dominator_vec<T: Clone>(
    out: &mut Vec<T>,
    len: usize,
    value: T,
    context: &str,
) -> Result<(), String> {
    if len > out.len() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "dominator tree CPU oracle",
            context,
        )?;
    }
    out.resize(len, value);
    Ok(())
}

pub(super) fn push_dominator_vec<T>(
    out: &mut Vec<T>,
    value: T,
    context: &str,
) -> Result<(), String> {
    crate::graph::scratch::reserve_graph_items(out, 1, "dominator tree CPU oracle", context)?;
    out.push(value);
    Ok(())
}
