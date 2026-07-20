use std::collections::BTreeMap;
use std::fmt::Write;
use vyre_lower::{BindingVisibility, KernelBody, KernelDescriptor};

pub struct DescriptorDumpOptions {
    pub show_literals: bool,
    pub show_result_ids: bool,
    pub max_ops_per_body: usize,
}

impl Default for DescriptorDumpOptions {
    fn default() -> Self {
        Self {
            show_literals: true,
            show_result_ids: true,
            max_ops_per_body: usize::MAX,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DescriptorDump {
    pub text: String,
    #[serde(
        serialize_with = "crate::path_map_serde::serialize_usize",
        deserialize_with = "crate::path_map_serde::deserialize_usize"
    )]
    pub op_counts_by_path: BTreeMap<Vec<usize>, usize>,
}

pub fn dump_descriptor(desc: &KernelDescriptor, options: &DescriptorDumpOptions) -> DescriptorDump {
    let mut out = String::new();
    let mut counts = BTreeMap::new();

    let id_prefix = if desc.id.len() > 8 {
        &desc.id[0..8]
    } else {
        &desc.id
    };
    let _ = writeln!(out, "KernelDescriptor id={}", id_prefix);

    let _ = writeln!(out, "bindings:");
    for slot in &desc.bindings.slots {
        let count_str = match slot.element_count {
            Some(c) => format!("Some({})", c),
            None => "None".to_string(),
        };
        let access_str = match slot.visibility {
            BindingVisibility::ReadOnly => "ReadOnly",
            BindingVisibility::ReadWrite => "ReadWrite",
            BindingVisibility::WriteOnly => "WriteOnly",
        };
        // Just format it properly
        let _ = writeln!(
            out,
            "  slot={} name={} count={} access={} mc={:?}",
            slot.slot, slot.name, count_str, access_str, slot.memory_class
        );
    }

    let _ = writeln!(
        out,
        "dispatch: workgroup_size={:?}",
        desc.dispatch.workgroup_size
    );

    fn walk_body(
        body: &KernelBody,
        path: Vec<usize>,
        indent: usize,
        out: &mut String,
        counts: &mut BTreeMap<Vec<usize>, usize>,
        options: &DescriptorDumpOptions,
    ) {
        counts.insert(path.clone(), body.ops.len());

        let path_str = if path.is_empty() {
            "[]".to_string()
        } else {
            let s: Vec<_> = path.iter().map(|n| n.to_string()).collect();
            format!("[{}]", s.join(","))
        };

        let indent_str = " ".repeat(indent);
        let _ = writeln!(out, "{}body{}:", indent_str, path_str);

        let mut ops_shown = 0;
        for (i, op) in body.ops.iter().enumerate() {
            if ops_shown >= options.max_ops_per_body {
                let remaining = body.ops.len() - ops_shown;
                let _ = writeln!(out, "{}  ... <{} more ops>", indent_str, remaining);
                break;
            }

            let result_str = if options.show_result_ids {
                match op.result {
                    Some(r) => format!(" result=Some({})", r),
                    None => " result=None".to_string(),
                }
            } else {
                "".to_string()
            };

            let _ = writeln!(
                out,
                "{}  [{}] {:?} ops={:?}{}",
                indent_str, i, op.kind, op.operands, result_str
            );

            // If the op kind has child bodies, we need to iterate over them.
            // Wait, child_bodies is a flat array in KernelBody. The op just references them by index?
            // Let's assume ops reference child body indices in `op.operands` or something?
            // No, the dump says: "body[0,0,12,0]" for structured_if_then ops=[10, 0] -> actually vyre_lower has child bodies. Let me check KernelOp / KernelBody.
            ops_shown += 1;
        }

        for (i, child) in body.child_bodies.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(i);
            walk_body(child, child_path, indent + 2, out, counts, options);
        }
    }

    walk_body(&desc.body, vec![], 0, &mut out, &mut counts, options);

    if options.show_literals {
        let _ = writeln!(out, "literals:");
        fn walk_literals(body: &KernelBody, path: Vec<usize>, out: &mut String) {
            let path_str = if path.is_empty() {
                "[]".to_string()
            } else {
                let s: Vec<_> = path.iter().map(|n| n.to_string()).collect();
                format!("[{}]", s.join(","))
            };

            // Just formatting literals
            let _ = writeln!(out, "  body{}: {:?}", path_str, body.literals);

            for (i, child) in body.child_bodies.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(i);
                walk_literals(child, child_path, out);
            }
        }
        walk_literals(&desc.body, vec![], &mut out);
    }

    // Remove trailing newline if any, or just keep it? Dump usually expects exact matches.
    DescriptorDump {
        text: out,
        op_counts_by_path: counts,
    }
}
