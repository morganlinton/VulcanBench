//! Frontier graph partitioning to reduce update contention.

/// Undirected conflict edge between two update targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierConflictEdge {
    /// First target node.
    pub a: u32,
    /// Second target node.
    pub b: u32,
}

/// Color assignment for one target node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierColor {
    /// Target node id.
    pub node: u32,
    /// Assigned color.
    pub color: u32,
}

/// Contention-reducing color plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontierColorPlan {
    /// Color assignment sorted by node id.
    pub colors: Vec<FrontierColor>,
    /// Number of colors used.
    pub color_count: u32,
}

/// Frontier coloring errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontierColorError {
    /// An edge references a node outside `0..node_count`.
    InvalidNode {
        /// Invalid node id.
        node: u32,
        /// Number of nodes.
        node_count: u32,
    },
}

impl std::fmt::Display for FrontierColorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNode { node, node_count } => write!(
                f,
                "frontier partition conflict references node {node} outside node_count={node_count}. Fix: normalize update targets before coloring."
            ),
        }
    }
}

impl std::error::Error for FrontierColorError {}

/// Greedy deterministic coloring for frontier update conflict graphs.
pub fn color_frontier_conflicts(
    node_count: u32,
    conflict_edges: &[FrontierConflictEdge],
) -> Result<FrontierColorPlan, FrontierColorError> {
    for edge in conflict_edges {
        validate_node(edge.a, node_count)?;
        validate_node(edge.b, node_count)?;
    }

    let mut adjacency = vec![Vec::<u32>::new(); node_count as usize];
    for edge in conflict_edges {
        if edge.a == edge.b {
            continue;
        }
        adjacency[edge.a as usize].push(edge.b);
        adjacency[edge.b as usize].push(edge.a);
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    let mut assigned = vec![u32::MAX; node_count as usize];
    let mut used = Vec::new();
    for node in 0..node_count {
        used.clear();
        for &neighbor in &adjacency[node as usize] {
            let color = assigned[neighbor as usize];
            if color != u32::MAX {
                used.push(color);
            }
        }
        used.sort_unstable();
        used.dedup();
        let mut color = 0;
        for used_color in &used {
            if *used_color == color {
                color += 1;
            } else if *used_color > color {
                break;
            }
        }
        assigned[node as usize] = color;
    }

    let color_count = assigned
        .iter()
        .copied()
        .max()
        .map_or(0, |color| color.saturating_add(1));
    let colors = assigned
        .into_iter()
        .enumerate()
        .map(|(node, color)| FrontierColor {
            node: node as u32,
            color,
        })
        .collect();

    Ok(FrontierColorPlan {
        colors,
        color_count,
    })
}

fn validate_node(node: u32, node_count: u32) -> Result<(), FrontierColorError> {
    if node >= node_count {
        Err(FrontierColorError::InvalidNode { node, node_count })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coloring_splits_conflicting_frontier_updates() {
        let plan = color_frontier_conflicts(
            4,
            &[
                FrontierConflictEdge { a: 0, b: 1 },
                FrontierConflictEdge { a: 1, b: 2 },
                FrontierConflictEdge { a: 2, b: 0 },
            ],
        )
        .expect("Fix: triangle conflict should color");

        assert_eq!(plan.color_count, 3);
        assert_ne!(plan.colors[0].color, plan.colors[1].color);
        assert_ne!(plan.colors[1].color, plan.colors[2].color);
        assert_ne!(plan.colors[2].color, plan.colors[0].color);
        assert_eq!(plan.colors[3].color, 0);
    }

    #[test]
    fn coloring_ignores_self_edges_and_deduplicates_neighbors() {
        let plan = color_frontier_conflicts(
            2,
            &[
                FrontierConflictEdge { a: 0, b: 0 },
                FrontierConflictEdge { a: 0, b: 1 },
                FrontierConflictEdge { a: 1, b: 0 },
            ],
        )
        .expect("Fix: duplicate conflicts should color");

        assert_eq!(plan.color_count, 2);
        assert_ne!(plan.colors[0].color, plan.colors[1].color);
    }

    #[test]
    fn coloring_rejects_invalid_nodes() {
        let err = color_frontier_conflicts(2, &[FrontierConflictEdge { a: 0, b: 2 }])
            .expect_err("invalid node should fail");

        assert_eq!(
            err,
            FrontierColorError::InvalidNode {
                node: 2,
                node_count: 2,
            }
        );
    }
}
