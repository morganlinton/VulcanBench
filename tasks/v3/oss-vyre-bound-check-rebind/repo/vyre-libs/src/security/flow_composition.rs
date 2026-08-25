use vyre::ir::Program;
use vyre_foundation::execution_plan::fusion::fuse_programs;
use vyre_foundation::ir::DataType;
use vyre_primitives::bitset::and::bitset_and;
#[cfg(test)]
use vyre_primitives::bitset::and::cpu_ref as bitset_and_cpu_ref;
use vyre_primitives::bitset::and_not::bitset_and_not;
#[cfg(test)]
use vyre_primitives::bitset::and_not::cpu_ref as bitset_and_not_cpu_ref;
use vyre_primitives::bitset::any::bitset_any;
use vyre_primitives::graph::csr_forward_traverse::{bitset_words, csr_forward_traverse};
#[cfg(test)]
use vyre_primitives::graph::csr_forward_traverse::cpu_ref as csr_forward_cpu_ref;
use vyre_primitives::graph::program_graph::ProgramGraphShape;

use crate::region::{reparent_program_children, wrap_anonymous};
use crate::security::flows_to::{FLOWS_TO_MASK, OP_ID as FLOWS_TO_OP_ID};

#[derive(Clone, Copy)]
pub(crate) struct SecurityFlowOptions<'a> {
    pub(crate) op_id: &'static str,
    pub(crate) shape: ProgramGraphShape,
    pub(crate) source_buf: &'a str,
    pub(crate) sink_buf: Option<&'a str>,
    pub(crate) sanitizer_buf: Option<&'a str>,
    pub(crate) clean_buf: Option<&'a str>,
    pub(crate) reach_buf: &'a str,
    pub(crate) alive_buf: Option<&'a str>,
    pub(crate) hits_buf: Option<&'a str>,
    pub(crate) out_scalar_buf: Option<&'a str>,
    pub(crate) edge_mask: u32,
    pub(crate) emit_hit_witness: bool,
}

impl<'a> SecurityFlowOptions<'a> {
    pub(crate) fn reach(
        op_id: &'static str,
        shape: ProgramGraphShape,
        source_buf: &'a str,
        reach_buf: &'a str,
        edge_mask: u32,
    ) -> Self {
        Self {
            op_id,
            shape,
            source_buf,
            sink_buf: None,
            sanitizer_buf: None,
            clean_buf: None,
            reach_buf,
            alive_buf: None,
            hits_buf: None,
            out_scalar_buf: None,
            edge_mask,
            emit_hit_witness: false,
        }
    }

    pub(crate) fn hit(
        op_id: &'static str,
        shape: ProgramGraphShape,
        source_buf: &'a str,
        sink_buf: &'a str,
        reach_buf: &'a str,
        hits_buf: &'a str,
        out_scalar_buf: &'a str,
    ) -> Self {
        Self {
            op_id,
            shape,
            source_buf,
            sink_buf: Some(sink_buf),
            sanitizer_buf: None,
            clean_buf: None,
            reach_buf,
            alive_buf: None,
            hits_buf: Some(hits_buf),
            out_scalar_buf: Some(out_scalar_buf),
            edge_mask: FLOWS_TO_MASK,
            emit_hit_witness: true,
        }
    }

    pub(crate) fn sanitized_hit(
        op_id: &'static str,
        shape: ProgramGraphShape,
        source_buf: &'a str,
        sink_buf: &'a str,
        sanitizer_buf: &'a str,
        clean_buf: &'a str,
        reach_buf: &'a str,
        alive_buf: &'a str,
        hits_buf: &'a str,
        out_scalar_buf: &'a str,
    ) -> Self {
        Self {
            op_id,
            shape,
            source_buf,
            sink_buf: Some(sink_buf),
            sanitizer_buf: Some(sanitizer_buf),
            clean_buf: Some(clean_buf),
            reach_buf,
            alive_buf: Some(alive_buf),
            hits_buf: Some(hits_buf),
            out_scalar_buf: Some(out_scalar_buf),
            edge_mask: FLOWS_TO_MASK,
            emit_hit_witness: true,
        }
    }
}

pub(crate) fn fuse_security_flow(op_id: &'static str, parts: &[Program], output: &str) -> Program {
    let fused = match fuse_programs(parts) {
        Ok(fused) => fused,
        Err(error) => {
            return crate::builder::invalid_output_program(
                op_id,
                output,
                DataType::U32,
                format!("Fix: security flow composition failed to fuse: {error}"),
            );
        }
    };
    Program::wrapped(
        fused.buffers().to_vec(),
        fused.workgroup_size(),
        vec![wrap_anonymous(
            op_id,
            reparent_program_children(&fused, op_id),
        )],
    )
}

#[cfg(test)]
pub(crate) fn dataflow_reach_step_cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    source: &[u32],
) -> Vec<u32> {
    csr_forward_cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        source,
        FLOWS_TO_MASK,
    )
}

#[cfg(test)]
pub(crate) fn any_dataflow_hit_cpu_ref(reach: &[u32], sink: &[u32]) -> u32 {
    let hits = bitset_and_cpu_ref(reach, sink);
    u32::from(hits.iter().any(|word| *word != 0))
}

pub(crate) fn dataflow_hit_program(
    op_id: &'static str,
    shape: ProgramGraphShape,
    source_buf: &str,
    sink_buf: &str,
    reach_buf: &str,
    hits_buf: &str,
    out_scalar_buf: &str,
) -> Program {
    security_flow_program(SecurityFlowOptions::hit(
        op_id,
        shape,
        source_buf,
        sink_buf,
        reach_buf,
        hits_buf,
        out_scalar_buf,
    ))
}

pub(crate) fn sanitized_dataflow_hit_program(
    op_id: &'static str,
    shape: ProgramGraphShape,
    source_buf: &str,
    sink_buf: &str,
    sanitizer_buf: &str,
    clean_buf: &str,
    reach_buf: &str,
    alive_buf: &str,
    hits_buf: &str,
    out_scalar_buf: &str,
) -> Program {
    security_flow_program(SecurityFlowOptions::sanitized_hit(
        op_id,
        shape,
        source_buf,
        sink_buf,
        sanitizer_buf,
        clean_buf,
        reach_buf,
        alive_buf,
        hits_buf,
        out_scalar_buf,
    ))
}

pub(crate) fn dataflow_reach_program(
    op_id: &'static str,
    shape: ProgramGraphShape,
    source_buf: &str,
    reach_buf: &str,
    edge_mask: u32,
) -> Program {
    security_flow_program(SecurityFlowOptions::reach(
        op_id, shape, source_buf, reach_buf, edge_mask,
    ))
}

pub(crate) fn security_flow_program(options: SecurityFlowOptions<'_>) -> Program {
    crate::security::assert_security_inputs(
        options.op_id,
        options.shape.node_count,
        &[("source_buf", options.source_buf), ("reach_buf", options.reach_buf)],
    );
    let words = bitset_words(options.shape.node_count);
    let mut parts = Vec::new();
    let traverse_source = if let Some(sanitizer_buf) = options.sanitizer_buf {
        let Some(clean_buf) = options.clean_buf else {
            return crate::builder::invalid_output_program(
                options.op_id,
                options.reach_buf,
                DataType::U32,
                "Fix: security flow with sanitizer requires clean_buf.".to_string(),
            );
        };
        parts.push(bitset_and_not(
            options.source_buf,
            sanitizer_buf,
            clean_buf,
            words,
        ));
        clean_buf
    } else {
        options.source_buf
    };
    let traverse = crate::region::tag_program(
        FLOWS_TO_OP_ID,
        csr_forward_traverse(
            options.shape,
            traverse_source,
            options.reach_buf,
            options.edge_mask,
        ),
    );
    if options.sink_buf.is_none() {
        if parts.is_empty() && options.op_id == FLOWS_TO_OP_ID {
            return traverse;
        }
        parts.push(traverse);
        return fuse_security_flow(options.op_id, &parts, options.reach_buf);
    }
    parts.push(traverse);
    let hit_source = if let Some(sanitizer_buf) = options.sanitizer_buf {
        let Some(alive_buf) = options.alive_buf else {
            return crate::builder::invalid_output_program(
                options.op_id,
                options.reach_buf,
                DataType::U32,
                "Fix: sanitized security flow requires alive_buf.".to_string(),
            );
        };
        parts.push(bitset_and_not(
            options.reach_buf,
            sanitizer_buf,
            alive_buf,
            words,
        ));
        alive_buf
    } else {
        options.reach_buf
    };
    let Some(sink_buf) = options.sink_buf else {
        return crate::builder::invalid_output_program(
            options.op_id,
            options.reach_buf,
            DataType::U32,
            "Fix: security flow hit mode requires sink_buf.".to_string(),
        );
    };
    let Some(hits_buf) = options.hits_buf else {
        return crate::builder::invalid_output_program(
            options.op_id,
            options.reach_buf,
            DataType::U32,
            "Fix: security flow hit mode requires hits_buf witness storage.".to_string(),
        );
    };
    let Some(out_scalar_buf) = options.out_scalar_buf else {
        return crate::builder::invalid_output_program(
            options.op_id,
            options.reach_buf,
            DataType::U32,
            "Fix: security flow hit mode requires out_scalar_buf.".to_string(),
        );
    };
    let intersect = bitset_and(hit_source, sink_buf, hits_buf, words);
    parts.push(intersect);
    if options.emit_hit_witness {
        parts.push(bitset_any(hits_buf, out_scalar_buf, words));
    }
    fuse_security_flow(options.op_id, &parts, out_scalar_buf)
}

#[cfg(test)]
pub(crate) fn sanitized_dataflow_hit_cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    source: &[u32],
    sink: &[u32],
    sanitizer: &[u32],
) -> u32 {
    let clean = bitset_and_not_cpu_ref(source, sanitizer);
    let reach = dataflow_reach_step_cpu_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        &clean,
    );
    let alive = bitset_and_not_cpu_ref(&reach, sanitizer);
    any_dataflow_hit_cpu_ref(&alive, sink)
}

#[cfg(test)]
pub(crate) fn linear_dataflow(node_count: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut offsets = vec![0u32; (node_count + 1) as usize];
    let mut targets = Vec::new();
    let mut masks = Vec::new();
    for i in 0..node_count.saturating_sub(1) {
        offsets[i as usize + 1] = offsets[i as usize] + 1;
        targets.push(i + 1);
        masks.push(vyre_primitives::predicate::edge_kind::ASSIGNMENT);
    }
    let penultimate = offsets[node_count as usize - 1];
    if let Some(last) = offsets.last_mut() {
        *last = penultimate;
    }
    (offsets, targets, masks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameterized_reach_builder_matches_flows_to_public_wrapper() {
        let shape = ProgramGraphShape::new(4, 3);
        let expected = crate::security::flows_to::flows_to(shape, "fin", "fout");
        let actual = security_flow_program(SecurityFlowOptions::reach(
            crate::security::flows_to::OP_ID,
            shape,
            "fin",
            "fout",
            FLOWS_TO_MASK,
        ));

        assert_eq!(actual.fingerprint(), expected.fingerprint());
    }

    #[test]
    fn parameterized_hit_builder_matches_flows_to_to_sink_public_wrapper() {
        let shape = ProgramGraphShape::new(4, 3);
        let expected = crate::security::flows_to_to_sink::flows_to_to_sink(
            shape,
            "source",
            "sink",
            "reach",
            "hits",
            "out_scalar",
        );
        let actual = security_flow_program(SecurityFlowOptions::hit(
            crate::security::flows_to_to_sink::OP_ID,
            shape,
            "source",
            "sink",
            "reach",
            "hits",
            "out_scalar",
        ));

        assert_eq!(actual.fingerprint(), expected.fingerprint());
    }

    #[test]
    fn parameterized_sanitized_builder_matches_public_wrapper() {
        let shape = ProgramGraphShape::new(4, 3);
        let expected = crate::security::flows_to_with_sanitizer::flows_to_with_sanitizer(
            shape,
            "source",
            "sink",
            "sanitizer",
            "clean",
            "reach",
            "alive",
            "hits",
            "out_scalar",
        );
        let actual = security_flow_program(SecurityFlowOptions::sanitized_hit(
            crate::security::flows_to_with_sanitizer::OP_ID,
            shape,
            "source",
            "sink",
            "sanitizer",
            "clean",
            "reach",
            "alive",
            "hits",
            "out_scalar",
        ));

        assert_eq!(actual.fingerprint(), expected.fingerprint());
    }
}
