use vyre_foundation::ir::{Node, Program};

#[derive(Clone)]
struct Scope {
    is_loop_boundary: bool,
    defs: std::collections::HashSet<String>,
}

pub fn walk_source_assigns<F>(program: &Program, mut callback: F)
where
    F: FnMut(&str, Vec<String>),
{
    let mut env = vec![Scope {
        is_loop_boundary: false,
        defs: std::collections::HashSet::new(),
    }];
    let mut current_loop_path = Vec::new();

    fn walk<F>(
        nodes: &[Node],
        env: &mut Vec<Scope>,
        current_loop_path: &mut Vec<String>,
        callback: &mut F,
    ) where
        F: FnMut(&str, Vec<String>),
    {
        for node in nodes {
            match node {
                Node::Let { name, .. } => {
                    if let Some(scope) = env.last_mut() {
                        scope.defs.insert(name.as_ref().to_string());
                    }
                }
                Node::Assign { name, .. } => {
                    let name_str = name.as_ref().to_string();
                    if current_loop_path.is_empty() {
                        if let Some(scope) = env.last_mut() {
                            scope.defs.insert(name_str);
                        }
                        continue;
                    }

                    let mut defined_outside_loop = false;
                    let mut loop_boundaries_crossed = 0;

                    for scope in env.iter().rev() {
                        if scope.is_loop_boundary {
                            loop_boundaries_crossed += 1;
                        }

                        if scope.defs.contains(&name_str) {
                            if loop_boundaries_crossed > 0 {
                                defined_outside_loop = true;
                            }
                            break;
                        }
                    }

                    if defined_outside_loop {
                        callback(&name_str, current_loop_path.clone());
                    }

                    if let Some(scope) = env.last_mut() {
                        scope.defs.insert(name_str);
                    }
                }
                Node::Block(stmts) => {
                    env.push(Scope {
                        is_loop_boundary: false,
                        defs: std::collections::HashSet::new(),
                    });
                    walk(stmts, env, current_loop_path, callback);
                    env.pop();
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    env.push(Scope {
                        is_loop_boundary: false,
                        defs: std::collections::HashSet::new(),
                    });
                    walk(then, env, current_loop_path, callback);
                    env.pop();

                    env.push(Scope {
                        is_loop_boundary: false,
                        defs: std::collections::HashSet::new(),
                    });
                    walk(otherwise, env, current_loop_path, callback);
                    env.pop();
                }
                Node::Loop { var, body, .. } => {
                    let loop_name_str = var.as_ref().to_string();
                    current_loop_path.push(loop_name_str.clone());

                    env.push(Scope {
                        is_loop_boundary: true,
                        defs: std::collections::HashSet::new(),
                    });
                    if let Some(scope) = env.last_mut() {
                        scope.defs.insert(loop_name_str);
                    }

                    walk(body, env, current_loop_path, callback);

                    env.pop();
                    current_loop_path.pop();
                }
                Node::Region { body, .. } => {
                    env.push(Scope {
                        is_loop_boundary: false,
                        defs: std::collections::HashSet::new(),
                    });
                    walk(body, env, current_loop_path, callback);
                    env.pop();
                }
                _ => {}
            }
        }
    }

    walk(
        &program.entry,
        &mut env,
        &mut current_loop_path,
        &mut callback,
    );
}
