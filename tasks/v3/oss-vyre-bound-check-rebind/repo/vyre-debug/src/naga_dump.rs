use naga::{Block, Module, Statement};
use std::collections::HashMap;

pub struct NagaDump {
    pub text: String,
    // Maps handle index -> enclosing block path
    block_paths: HashMap<u32, Vec<String>>,
}

impl NagaDump {
    pub fn find(&self, handle: u32) -> Option<&Vec<String>> {
        self.block_paths.get(&handle)
    }
}

pub fn dump_naga_module(module: &Module) -> NagaDump {
    let mut out = String::new();
    let mut block_paths = HashMap::new();

    out.push_str("=== globals ===\n");
    for (handle, global) in module.global_variables.iter() {
        let name = global.name.as_deref().unwrap_or("");
        out.push_str(&format!(
            "[{}] {} : {:?}, space={:?}\n",
            handle.index(),
            name,
            global.ty,
            global.space
        ));
    }
    out.push_str("\n");

    // We mainly care about entry points or the main function body.
    // In vyre, there is typically one entry point.
    if let Some(ep) = module.entry_points.first() {
        let func = &ep.function;
        out.push_str("=== local_variables ===\n");
        for (handle, local) in func.local_variables.iter() {
            let name = local.name.as_deref().unwrap_or("");
            out.push_str(&format!(
                "[{}] {:?} : {:?}\n",
                handle.index(),
                name,
                local.ty
            ));
        }
        out.push_str("\n");

        out.push_str("=== expressions ===\n");
        for (handle, expr) in func.expressions.iter() {
            // Very simplified rendering
            out.push_str(&format!("[{}] {:?}\n", handle.index(), expr));
        }
        out.push_str("\n");

        out.push_str("=== body ===\n");
        fn walk_block(
            block: &Block,
            path: &mut Vec<String>,
            out: &mut String,
            indent: usize,
            block_paths: &mut HashMap<u32, Vec<String>>,
            func: &naga::Function,
        ) {
            let ind = "  ".repeat(indent);
            out.push_str(&format!("{}{{\n", ind));
            for stmt in block.iter() {
                out.push_str(&format!("{}  {:?}\n", ind, stmt));
                match stmt {
                    Statement::Emit(range) => {
                        for h in range.clone() {
                            block_paths.insert(h.index() as u32, path.clone());
                        }
                    }
                    Statement::Block(b) => {
                        path.push("Block".to_string());
                        walk_block(b, path, out, indent + 1, block_paths, func);
                        path.pop();
                    }
                    Statement::If {
                        condition: _,
                        accept,
                        reject,
                    } => {
                        path.push("If.accept".to_string());
                        walk_block(accept, path, out, indent + 1, block_paths, func);
                        path.pop();
                        path.push("If.reject".to_string());
                        walk_block(reject, path, out, indent + 1, block_paths, func);
                        path.pop();
                    }
                    Statement::Loop {
                        body,
                        continuing,
                        break_if: _,
                    } => {
                        path.push("Loop.body".to_string());
                        walk_block(body, path, out, indent + 1, block_paths, func);
                        path.pop();
                        path.push("Loop.continuing".to_string());
                        walk_block(continuing, path, out, indent + 1, block_paths, func);
                        path.pop();
                    }
                    _ => {}
                }
            }
            out.push_str(&format!("{}}}\n", ind));
        }

        let mut root_path = vec!["root".to_string()];
        walk_block(
            &func.body,
            &mut root_path,
            &mut out,
            0,
            &mut block_paths,
            func,
        );
    }

    NagaDump {
        text: out,
        block_paths,
    }
}
