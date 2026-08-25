//! Frontier-typed IR dependency waves.

use std::collections::HashMap;

/// Work domain for a frontier-typed node.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FrontierDomain {
    /// Lexing, token classification, or preprocessing.
    Parser,
    /// Declaration, type, scope, or semantic facts.
    Semantic,
    /// Dataflow facts over graph layouts.
    Dataflow,
    /// Diagnostic aggregation and provenance.
    Diagnostic,
}

/// One frontier-typed IR node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierNode {
    /// Stable node id.
    pub id: u32,
    /// Work domain.
    pub domain: FrontierDomain,
    /// Estimated active items in this frontier.
    pub active_items: u32,
}

/// Directed dependency edge `before -> after`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontierDependency {
    /// Prerequisite node.
    pub before: u32,
    /// Dependent node.
    pub after: u32,
}

/// One dependency wave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontierWave {
    /// Wave index.
    pub index: u32,
    /// Domains present in the wave.
    pub domains: Vec<FrontierDomain>,
    /// Node ids in stable order.
    pub node_ids: Vec<u32>,
    /// Total active items in the wave.
    pub active_items: u64,
}

/// Frontier-typed execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontierTypedPlan {
    /// Dependency waves.
    pub waves: Vec<FrontierWave>,
}

/// Frontier planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontierTypedPlanError {
    /// Duplicate node id.
    DuplicateNode { id: u32 },
    /// Dependency references an unknown node.
    UnknownDependencyNode { id: u32 },
    /// Dependency graph contains a cycle.
    Cycle { unscheduled_nodes: usize },
    /// The plan exceeds the stable CUDA frontier wave encoding.
    PlanTooLarge {
        /// Field that exceeded its representable range.
        field: &'static str,
    },
}

impl std::fmt::Display for FrontierTypedPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode { id } => write!(
                f,
                "frontier-typed IR has duplicate node id {id}. Fix: assign globally unique wave node ids."
            ),
            Self::UnknownDependencyNode { id } => write!(
                f,
                "frontier-typed IR dependency references unknown node {id}. Fix: emit all nodes before planning dependencies."
            ),
            Self::Cycle { unscheduled_nodes } => write!(
                f,
                "frontier-typed IR contains a dependency cycle with {unscheduled_nodes} unscheduled node(s). Fix: insert an explicit fixed-point frontier node."
            ),
            Self::PlanTooLarge { field } => write!(
                f,
                "frontier-typed IR {field} exceeds the stable CUDA frontier encoding. Fix: shard the frontier graph before planning."
            ),
        }
    }
}

impl std::error::Error for FrontierTypedPlanError {}

/// Plan frontier-typed dependency waves.
pub fn plan_frontier_typed_ir(
    nodes: &[FrontierNode],
    dependencies: &[FrontierDependency],
) -> Result<FrontierTypedPlan, FrontierTypedPlanError> {
    let mut node_indices = HashMap::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        if node_indices.insert(node.id, index).is_some() {
            return Err(FrontierTypedPlanError::DuplicateNode { id: node.id });
        }
    }
    let mut successors: Vec<Vec<usize>> = (0..nodes.len()).map(|_| Vec::new()).collect();
    let mut indegree = vec![0_u32; nodes.len()];
    for dependency in dependencies {
        let before = match node_indices.get(&dependency.before) {
            Some(&index) => index,
            None => {
                return Err(FrontierTypedPlanError::UnknownDependencyNode {
                    id: dependency.before,
                });
            }
        };
        let after = match node_indices.get(&dependency.after) {
            Some(&index) => index,
            None => {
                return Err(FrontierTypedPlanError::UnknownDependencyNode {
                    id: dependency.after,
                });
            }
        };
        successors[before].push(after);
        indegree[after] =
            indegree[after]
                .checked_add(1)
                .ok_or(FrontierTypedPlanError::PlanTooLarge {
                    field: "dependency indegree",
                })?;
    }

    let mut ready = Vec::with_capacity(nodes.len());
    for (index, &degree) in indegree.iter().enumerate() {
        if degree == 0 {
            ready.push(index);
        }
    }

    let mut scheduled = 0_usize;
    let mut waves = Vec::new();
    let mut next_ready = Vec::new();
    while !ready.is_empty() {
        ready.sort_unstable_by_key(|&index| (nodes[index].domain, nodes[index].id));
        let wave_index =
            u32::try_from(waves.len()).map_err(|_| FrontierTypedPlanError::PlanTooLarge {
                field: "wave count",
            })?;
        let mut domains = Vec::new();
        let mut node_ids = Vec::with_capacity(ready.len());
        let mut active_items = 0_u64;
        next_ready.clear();
        for &node_index in &ready {
            let node = nodes[node_index];
            if !domains.contains(&node.domain) {
                domains.push(node.domain);
            }
            node_ids.push(node.id);
            active_items = active_items
                .checked_add(u64::from(node.active_items))
                .ok_or(FrontierTypedPlanError::PlanTooLarge {
                    field: "active item count",
                })?;
            scheduled += 1;
            for &successor in &successors[node_index] {
                indegree[successor] -= 1;
                if indegree[successor] == 0 {
                    next_ready.push(successor);
                }
            }
        }
        waves.push(FrontierWave {
            index: wave_index,
            domains,
            node_ids,
            active_items,
        });
        std::mem::swap(&mut ready, &mut next_ready);
    }

    if scheduled != nodes.len() {
        return Err(FrontierTypedPlanError::Cycle {
            unscheduled_nodes: nodes.len() - scheduled,
        });
    }

    Ok(FrontierTypedPlan { waves })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontier_typed_ir_groups_independent_work_into_waves() {
        let plan = plan_frontier_typed_ir(
            &[
                node(0, FrontierDomain::Parser, 10),
                node(1, FrontierDomain::Semantic, 20),
                node(2, FrontierDomain::Dataflow, 30),
                node(3, FrontierDomain::Diagnostic, 4),
            ],
            &[
                FrontierDependency {
                    before: 0,
                    after: 1,
                },
                FrontierDependency {
                    before: 1,
                    after: 2,
                },
                FrontierDependency {
                    before: 1,
                    after: 3,
                },
            ],
        )
        .expect("Fix: valid frontier-typed plan should build");

        assert_eq!(plan.waves.len(), 3);
        assert_eq!(plan.waves[0].node_ids, vec![0]);
        assert_eq!(plan.waves[1].node_ids, vec![1]);
        assert_eq!(plan.waves[2].node_ids, vec![2, 3]);
        assert_eq!(plan.waves[2].active_items, 34);
    }

    #[test]
    fn frontier_typed_ir_rejects_unknown_duplicate_and_cycle() {
        assert_eq!(
            plan_frontier_typed_ir(
                &[
                    node(1, FrontierDomain::Parser, 1),
                    node(1, FrontierDomain::Semantic, 1)
                ],
                &[],
            )
            .expect_err("duplicate node ids should fail"),
            FrontierTypedPlanError::DuplicateNode { id: 1 }
        );
        assert_eq!(
            plan_frontier_typed_ir(
                &[node(1, FrontierDomain::Parser, 1)],
                &[FrontierDependency {
                    before: 1,
                    after: 2,
                }],
            )
            .expect_err("unknown dependency should fail"),
            FrontierTypedPlanError::UnknownDependencyNode { id: 2 }
        );
        assert_eq!(
            plan_frontier_typed_ir(
                &[
                    node(1, FrontierDomain::Parser, 1),
                    node(2, FrontierDomain::Semantic, 1)
                ],
                &[
                    FrontierDependency {
                        before: 1,
                        after: 2,
                    },
                    FrontierDependency {
                        before: 2,
                        after: 1,
                    },
                ],
            )
            .expect_err("cycle should fail"),
            FrontierTypedPlanError::Cycle {
                unscheduled_nodes: 2,
            }
        );
    }

    #[test]
    fn frontier_typed_ir_planner_uses_adjacency_not_wave_rescans() {
        let source = include_str!("frontier_typed_ir.rs");
        assert!(
            !source.contains(concat!("BTree", "Set")),
            "Fix: frontier-typed IR planning must not use ordered sets in the release scheduler hot path."
        );
        assert!(
            !source.contains(concat!(
                "dependencies",
                "\n",
                "                    .iter()"
            )),
            "Fix: frontier-typed IR planning must not rescan every dependency for every candidate node."
        );
    }

    #[test]
    fn frontier_typed_ir_plans_wide_dag_with_stable_wave_order() {
        let mut nodes = Vec::new();
        let mut dependencies = Vec::new();
        for id in 0..512_u32 {
            nodes.push(node(
                id,
                if id % 2 == 0 {
                    FrontierDomain::Dataflow
                } else {
                    FrontierDomain::Parser
                },
                1,
            ));
            nodes.push(node(10_000 + id, FrontierDomain::Diagnostic, 2));
            dependencies.push(FrontierDependency {
                before: id,
                after: 10_000 + id,
            });
        }

        let plan =
            plan_frontier_typed_ir(&nodes, &dependencies).expect("Fix: wide DAG should plan");

        assert_eq!(plan.waves.len(), 2);
        assert_eq!(plan.waves[0].node_ids.len(), 512);
        assert_eq!(plan.waves[1].node_ids.len(), 512);
        assert_eq!(plan.waves[0].active_items, 512);
        assert_eq!(plan.waves[1].active_items, 1024);
        assert_eq!(plan.waves[0].node_ids[0], 1);
        assert_eq!(plan.waves[0].node_ids[1], 3);
    }

    fn node(id: u32, domain: FrontierDomain, active_items: u32) -> FrontierNode {
        FrontierNode {
            id,
            domain,
            active_items,
        }
    }
}
