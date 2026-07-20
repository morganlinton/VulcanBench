use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vyre_lower::verify::{classify_operand, OperandClass};
use vyre_lower::{KernelBody, KernelDescriptor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DanglingRef {
    pub ref_id: u32,
    pub produced_in_body_path: Vec<usize>,
    pub producing_op_index: usize,
    pub producing_op_kind: String,
    pub referenced_in_body_path: Vec<usize>,
    pub referencing_op_index: usize,
    pub referencing_op_kind: String,
    pub operand_position: usize,
}

pub fn find_dangling_refs(desc: &KernelDescriptor) -> Vec<DanglingRef> {
    let mut id_origins = BTreeMap::new();

    fn build_id_origins(
        body: &KernelBody,
        path: &mut Vec<usize>,
        map: &mut BTreeMap<u32, (Vec<usize>, usize, String)>,
    ) {
        for (i, op) in body.ops.iter().enumerate() {
            for r in op.result_ids() {
                map.insert(r, (path.clone(), i, format!("{:?}", op.kind)));
            }
        }
        for (idx, child) in body.child_bodies.iter().enumerate() {
            path.push(idx);
            build_id_origins(child, path, map);
            path.pop();
        }
    }
    build_id_origins(&desc.body, &mut vec![], &mut id_origins);

    let mut refs = Vec::new();
    check_body(
        &desc.body,
        &mut vec![],
        &BTreeSet::new(),
        &mut refs,
        &id_origins,
    );
    refs
}

fn check_body(
    body: &KernelBody,
    path: &mut Vec<usize>,
    inherited_results: &BTreeSet<u32>,
    refs: &mut Vec<DanglingRef>,
    id_origins: &BTreeMap<u32, (Vec<usize>, usize, String)>,
) {
    let mut produced: BTreeSet<u32> = BTreeSet::new();
    for op in &body.ops {
        for r in op.result_ids() {
            produced.insert(r);
        }
    }

    let mut produced_so_far: BTreeSet<u32> = BTreeSet::new();
    let child_results: Vec<BTreeSet<u32>> =
        body.child_bodies.iter().map(collect_body_results).collect();
    let mut completed_child_results: BTreeSet<u32> = BTreeSet::new();
    let mut child_scopes = vec![BTreeSet::new(); body.child_bodies.len()];

    for (i, op) in body.ops.iter().enumerate() {
        for (pos, &val) in op.operands.iter().enumerate() {
            let cls = classify_operand(&op.kind, pos);
            match cls {
                OperandClass::ResultRef => {
                    if !produced_so_far.contains(&val)
                        && !produced.contains(&val)
                        && !inherited_results.contains(&val)
                        && !completed_child_results.contains(&val)
                    {
                        let (prod_path, prod_idx, prod_kind) =
                            if let Some(orig) = id_origins.get(&val) {
                                (orig.0.clone(), orig.1, orig.2.clone())
                            } else {
                                (vec![], 0, "Unknown".to_string())
                            };

                        refs.push(DanglingRef {
                            ref_id: val,
                            produced_in_body_path: prod_path,
                            producing_op_index: prod_idx,
                            producing_op_kind: prod_kind,
                            referenced_in_body_path: path.clone(),
                            referencing_op_index: i,
                            referencing_op_kind: format!("{:?}", op.kind),
                            operand_position: pos,
                        });
                    }
                }
                OperandClass::ChildBodyIdx => {
                    if (val as usize) < body.child_bodies.len() {
                        let child_scope = &mut child_scopes[val as usize];
                        child_scope.extend(inherited_results.iter().copied());
                        child_scope.extend(produced_so_far.iter().copied());
                        child_scope.extend(completed_child_results.iter().copied());
                    }
                }
                _ => {}
            }
        }

        for r in op.result_ids() {
            produced_so_far.insert(r);
        }

        for (pos, &val) in op.operands.iter().enumerate() {
            if classify_operand(&op.kind, pos) == OperandClass::ChildBodyIdx {
                if let Some(results) = child_results.get(val as usize) {
                    completed_child_results.extend(results.iter().copied());
                }
            }
        }
    }

    for (idx, child) in body.child_bodies.iter().enumerate() {
        path.push(idx);
        check_body(child, path, &child_scopes[idx], refs, id_origins);
        path.pop();
    }
}

fn collect_body_results(body: &KernelBody) -> BTreeSet<u32> {
    let mut results = BTreeSet::new();
    for op in &body.ops {
        for result in op.result_ids() {
            results.insert(result);
        }
    }
    for child in &body.child_bodies {
        results.extend(collect_body_results(child));
    }
    results
}
