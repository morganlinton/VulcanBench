use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vyre_foundation::ir::Program;
use vyre_lower::{KernelDescriptor, KernelOpKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UncarrieredAssign {
    pub name: String,
    pub loop_path: Vec<String>,
    pub has_carrier_op: bool,
    pub has_final_op: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierSummary {
    pub total_ops_observed: usize,
    pub carrier_reads: BTreeMap<String, usize>,
    pub carrier_writes: BTreeMap<String, usize>,
    pub carrier_finals: BTreeMap<String, usize>,
    pub function_locals: Vec<String>,
}

pub fn find_uncarriered_assigns(
    program: &Program,
    desc: &KernelDescriptor,
) -> Vec<UncarrieredAssign> {
    let mut assigns = Vec::new();

    crate::source_walker::walk_source_assigns(program, |name, loop_path| {
        let mut has_carrier_op = false;
        let mut has_final_op = false;

        for op in desc.ops_iter() {
            match &op.kind {
                KernelOpKind::LoopCarrier { name: n } if n.as_ref() == name => {
                    has_carrier_op = true
                }
                KernelOpKind::LoopCarrierEnd { name: n } if n.as_ref() == name => {
                    has_final_op = true
                }
                _ => {}
            }
        }

        if !has_carrier_op || !has_final_op {
            assigns.push(UncarrieredAssign {
                name: name.to_string(),
                loop_path: loop_path.clone(),
                has_carrier_op,
                has_final_op,
            });
        }
    });

    assigns
}

pub fn carrier_summary(desc: &KernelDescriptor) -> CarrierSummary {
    let mut carrier_reads = BTreeMap::new();
    let mut carrier_writes = BTreeMap::new();
    let mut carrier_finals = BTreeMap::new();

    for op in desc.ops_iter() {
        match &op.kind {
            KernelOpKind::LoopCarrier { name } => {
                *carrier_reads.entry(name.to_string()).or_insert(0) += 1;
            }
            KernelOpKind::LoopCarrierEnd { name } => {
                *carrier_finals.entry(name.to_string()).or_insert(0) += 1;
            }
            KernelOpKind::LoopCarrierInit { name } => {
                *carrier_writes.entry(name.to_string()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut function_locals = Vec::new();
    if let Ok(module) = vyre_emit_naga::emit(desc) {
        for ep in &module.entry_points {
            for (_, local) in ep.function.local_variables.iter() {
                if let Some(name) = &local.name {
                    if name.starts_with("vyre_named_carry_") {
                        function_locals.push(name.clone());
                    }
                }
            }
        }
    }

    CarrierSummary {
        total_ops_observed: desc.ops_iter().count(),
        carrier_reads,
        carrier_writes,
        carrier_finals,
        function_locals,
    }
}
