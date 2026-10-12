use super::*;
/// Decode semantic ProgramGraph sections from a compiled `vyre-frontend-c` object.
///
/// Accepts either a standalone `VYRECOB2` blob or a larger object containing an
/// embedded `VYRECOB2` payload. This is the stable API for security/static
/// analysis tools that consume the C frontend output.
pub fn decode_object_semantic_graph(object_bytes: &[u8]) -> Result<CObjectSemanticGraph, String> {
    decode_embedded_object(object_bytes, decode_object_semantic_graph_from_container)
}

pub(crate) fn decode_object_semantic_graph_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectSemanticGraph, String> {
    let node_section = container
        .section(SectionTag::SemanticProgramGraphNodes)
        .ok_or_else(|| {
            "vyre-frontend-c object is missing SemanticProgramGraphNodes. Fix: compile with the VAST/ProgramGraph lowering pipeline enabled.".to_string()
        })?;
    let edge_section = container
        .section(SectionTag::SemanticProgramGraphEdges)
        .ok_or_else(|| {
            "vyre-frontend-c object is missing SemanticProgramGraphEdges. Fix: compile with the semantic ProgramGraph lowering pipeline enabled.".to_string()
    })?;
    let nodes = decode_c_ast_semantic_pg_nodes(node_section)?;
    validate_semantic_pg_nodes(&nodes)?;
    let edges = decode_c_ast_semantic_pg_edges(edge_section)?;
    validate_semantic_pg_edges(&edges, nodes.len())?;
    let builtin_role_nodes = checked_count_u64(
        nodes.iter().filter(|node| node.has_builtin_role()).count(),
        "semantic builtin-role node count",
    )?;
    Ok(CObjectSemanticGraph {
        vyrecob2_version: container.version,
        nodes,
        edges,
        builtin_role_nodes,
    })
}

pub(super) fn validate_semantic_pg_nodes(nodes: &[CAstSemanticPgNode]) -> Result<(), String> {
    let node_count = nodes.len();
    let mut root_count = 0usize;
    for (idx, node) in nodes.iter().enumerate() {
        if !is_known_semantic_category(node.category) {
            return Err(format!(
                "vyre-frontend-c semantic node {idx} has unknown category {}. Fix: regenerate the object with a supported semantic category.",
                node.category
            ));
        }
        if !is_known_semantic_role(node.role) {
            return Err(format!(
                "vyre-frontend-c semantic node {idx} has unknown role {}. Fix: update the object decoder role table or regenerate the object with supported semantic roles.",
                node.role
            ));
        }
        if node.span_end < node.span_start {
            return Err(format!(
                "vyre-frontend-c semantic node {idx} has inverted source span {}..{}. Fix: regenerate the object; semantic spans must be monotonic.",
                node.span_start, node.span_end
            ));
        }
        if node.parent == u32::MAX {
            root_count = root_count.checked_add(1).ok_or_else(|| {
                "vyre-frontend-c semantic ProgramGraph root count overflowed host usize. Fix: regenerate the object with bounded semantic nodes."
                    .to_string()
            })?;
        } else {
            let parent = usize::try_from(node.parent).map_err(|_| {
                format!("vyre-frontend-c semantic node {idx} parent index exceeds usize. Fix: regenerate the object with valid semantic tree indices.")
            })?;
            if parent >= node_count {
                return Err(format!(
                    "vyre-frontend-c semantic node {idx} parent {} is outside {node_count} decoded nodes. Fix: regenerate the object; semantic parent links must reference existing nodes.",
                    node.parent
                ));
            }
        }
        for (field, value) in [
            ("first_child", node.first_child),
            ("next_sibling", node.next_sibling),
        ] {
            if value == u32::MAX {
                continue;
            }
            let target = usize::try_from(value).map_err(|_| {
                format!("vyre-frontend-c semantic node {idx} {field} index exceeds usize. Fix: regenerate the object with valid semantic tree indices.")
            })?;
            if target >= node_count {
                return Err(format!(
                    "vyre-frontend-c semantic node {idx} {field} {value} is outside {node_count} decoded nodes. Fix: regenerate the object; semantic tree links must reference existing nodes."
                ));
            }
        }
    }
    if root_count != 1 {
        return Err(format!(
            "vyre-frontend-c semantic ProgramGraph has {root_count} root nodes; expected exactly one. Fix: regenerate the object with a single translation-unit semantic root."
        ));
    }
    Ok(())
}

pub(super) fn validate_semantic_pg_edges(
    edges: &[CAstSemanticPgEdge],
    node_count: usize,
) -> Result<(), String> {
    for (idx, edge) in edges.iter().enumerate() {
        let empty = edge.kind == 0 && edge.source == u32::MAX && edge.target == u32::MAX;
        if empty {
            continue;
        }
        let source = usize::try_from(edge.source).map_err(|_| {
            format!("vyre-frontend-c semantic edge {idx} source index exceeds usize. Fix: regenerate the object with valid semantic graph indices.")
        })?;
        let target = usize::try_from(edge.target).map_err(|_| {
            format!("vyre-frontend-c semantic edge {idx} target index exceeds usize. Fix: regenerate the object with valid semantic graph indices.")
        })?;
        if source >= node_count || target >= node_count {
            return Err(format!(
                "vyre-frontend-c semantic edge {idx} references source {source} target {target} outside {node_count} decoded semantic nodes. Fix: regenerate the object; semantic edges must reference existing nodes."
            ));
        }
        if !is_known_semantic_category(edge.owner_category) {
            return Err(format!(
                "vyre-frontend-c semantic edge {idx} has unknown owner category {}. Fix: regenerate the object with supported semantic edge metadata.",
                edge.owner_category
            ));
        }
        if !is_known_semantic_role(edge.owner_role) {
            return Err(format!(
                "vyre-frontend-c semantic edge {idx} has unknown owner role {}. Fix: update the object decoder role table or regenerate the object with supported semantic edge roles.",
                edge.owner_role
            ));
        }
    }
    Ok(())
}

/// Read and decode semantic ProgramGraph sections from a compiled object path.
pub fn decode_object_semantic_graph_file(path: &Path) -> Result<CObjectSemanticGraph, String> {
    read_object_file(path, decode_object_semantic_graph)
}

pub(super) fn decode_c_ast_semantic_pg_nodes(
    bytes: &[u8],
) -> Result<Vec<CAstSemanticPgNode>, String> {
    let stride = C_AST_PG_SEMANTIC_NODE_STRIDE_U32 as usize;
    let words = decode_u32_words(bytes)?;
    if words.is_empty() {
        return Err(
            "vyre-frontend-c semantic node section is empty. Fix: regenerate the object; semantic analysis must emit at least the translation-unit/root node."
                .to_string(),
        );
    }
    if words.len() % stride != 0 {
        return Err(format!(
            "vyre-frontend-c semantic node section has {} u32 words, not a multiple of stride {stride}. Fix: regenerate the object.",
            words.len()
        ));
    }
    Ok(words
        .chunks_exact(stride)
        .map(|row| CAstSemanticPgNode {
            kind: row[0],
            span_start: row[1],
            span_end: row[2],
            parent: row[3],
            first_child: row[4],
            next_sibling: row[5],
            category: row[6],
            role: row[7],
            attr0: row[8],
            attr1: row[9],
        })
        .collect())
}

pub(super) fn decode_c_ast_semantic_pg_edges(
    bytes: &[u8],
) -> Result<Vec<CAstSemanticPgEdge>, String> {
    let stride = C_AST_PG_EDGE_STRIDE_U32 as usize;
    let words = decode_u32_words(bytes)?;
    if words.len() % stride != 0 {
        return Err(format!(
            "vyre-frontend-c semantic edge section has {} u32 words, not a multiple of stride {stride}. Fix: regenerate the object.",
            words.len()
        ));
    }
    Ok(words
        .chunks_exact(stride)
        .map(|row| CAstSemanticPgEdge {
            kind: row[0],
            source: row[1],
            target: row[2],
            owner_kind: row[3],
            owner_role: row[4],
            owner_category: row[5],
        })
        .collect())
}
