use std::collections::HashMap;

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::lex::tokens::{
    ANDAND, EQ, GE, GT, LE, LT, MINUS, NE, OROR, PERCENT, PLUS, SLASH, STAR,
};
use vyre_libs::parsing::rust::lower::{lower, lower_batched};
use vyre_libs::parsing::rust::parse::{parse, Expr, Module, Stmt};
use vyre_libs::parsing::rust::sema::{resolve, typeck, BindingId, Resolution};
use vyre_reference::value::Value;

fn frontend(src: &str) -> (Module, Resolution) {
    let bytes = src.as_bytes();
    let tokens = lex(bytes).expect("lex");
    let module = parse(bytes, &tokens).expect("parse");
    let resolution = resolve(&module, bytes).expect("resolve");
    typeck(&module, bytes, &resolution).expect("typeck");
    (module, resolution)
}

fn value_to_i32(v: &Value) -> i32 {
    match v {
        Value::I32(x) => *x,
        Value::U32(x) => *x as i32,
        Value::Bool(b) => i32::from(*b),
        Value::Bytes(bytes) => i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")),
        other => panic!("unexpected output value {other:?}"),
    }
}

/// Lower `src`'s entry function and run it on the reference interpreter.
pub(crate) fn ir_exec(src: &str, inputs: &[i32]) -> i32 {
    let (module, resolution) = frontend(src);
    let program = lower(&module, &resolution).expect("lower");
    let values: Vec<Value> = inputs.iter().map(|&x| Value::I32(x)).collect();
    let out = vyre_reference::reference_eval(&program, &values).expect("reference_eval");
    assert_eq!(out.len(), 1, "entry must produce exactly one output");
    value_to_i32(&out[0])
}

fn i32_vec_to_bytes(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn bytes_to_i32_vec(bytes: &[u8]) -> Vec<i32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes(chunk.try_into().expect("i32 chunk")))
        .collect()
}

/// Lower `src`'s entry function as a data-parallel batch kernel and run it on
/// the reference interpreter. Each inner vector is one parameter buffer.
pub(crate) fn ir_exec_batched(src: &str, columns: &[Vec<i32>]) -> Vec<i32> {
    let lane_count = columns.first().map_or(0, Vec::len);
    assert!(lane_count > 0, "batched execution needs at least one lane");
    assert!(
        columns.iter().all(|column| column.len() == lane_count),
        "batched parameter columns must have equal lengths"
    );
    let (module, resolution) = frontend(src);
    let program = lower_batched(&module, &resolution, lane_count as u32).expect("lower_batched");
    let values: Vec<Value> = columns
        .iter()
        .map(|column| Value::from(i32_vec_to_bytes(column)))
        .collect();
    let out = vyre_reference::reference_eval(&program, &values).expect("reference_eval");
    assert_eq!(out.len(), 1, "entry must produce exactly one output");
    match &out[0] {
        Value::Bytes(bytes) => bytes_to_i32_vec(bytes),
        other => vec![value_to_i32(other)],
    }
}

fn global_def_to_id(resolution: &Resolution) -> HashMap<u32, BindingId> {
    resolution
        .bindings
        .iter()
        .enumerate()
        .map(|(id, b)| (b.def_offset, id))
        .collect()
}

enum Flow {
    Return(i32),
    Fall,
}

struct Ev<'a> {
    module: &'a Module,
    resolution: &'a Resolution,
    def_to_id: &'a HashMap<u32, BindingId>,
}

impl Ev<'_> {
    fn run_fn(&self, idx: usize, args: &[i32]) -> i32 {
        let func = &self.module.functions[idx];
        let mut env: HashMap<BindingId, i32> = HashMap::new();
        for (i, (offset, _)) in func.params.iter().enumerate() {
            env.insert(self.def_to_id[offset], args[i]);
        }
        match self.exec(&func.body, &mut env) {
            Flow::Return(v) => v,
            Flow::Fall => 0,
        }
    }

    fn exec(&self, stmts: &[Stmt], env: &mut HashMap<BindingId, i32>) -> Flow {
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, init, .. } => {
                    let v = self.eval_int(init, env);
                    env.insert(self.def_to_id[name], v);
                }
                Stmt::Return(Some(e)) => return Flow::Return(self.eval_int(e, env)),
                Stmt::Return(None) => return Flow::Return(0),
                Stmt::Assign { name, value } => {
                    let v = self.eval_int(value, env);
                    env.insert(self.resolution.uses[name], v);
                }
                Stmt::While { cond, body } => {
                    let mut guard = 0u32;
                    while self.eval_bool(cond, env) {
                        if let Flow::Return(v) = self.exec(body, env) {
                            return Flow::Return(v);
                        }
                        guard += 1;
                        assert!(guard < 1_000_000, "oracle while loop did not terminate");
                    }
                }
                Stmt::For {
                    name,
                    start,
                    end,
                    body,
                } => {
                    let binding = self.def_to_id[name];
                    let start = self.eval_int(start, env);
                    let end = self.eval_int(end, env);
                    for value in start..end {
                        env.insert(binding, value);
                        if let Flow::Return(v) = self.exec(body, env) {
                            return Flow::Return(v);
                        }
                    }
                }
                Stmt::Expr(Expr::If {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    let taken = if self.eval_bool(cond, env) {
                        Some(then_block.as_ref())
                    } else {
                        else_block.as_deref()
                    };
                    if let Some(Expr::Block(body)) = taken {
                        if let Flow::Return(v) = self.exec(body, env) {
                            return Flow::Return(v);
                        }
                    }
                }
                Stmt::Expr(_) => {}
            }
        }
        Flow::Fall
    }

    fn eval_int(&self, e: &Expr, env: &HashMap<BindingId, i32>) -> i32 {
        match e {
            Expr::LiteralInt(_, v) => *v as i32,
            Expr::Var(off) => env[&self.resolution.uses[off]],
            Expr::Binary { op, lhs, rhs } => {
                let (l, r) = (self.eval_int(lhs, env), self.eval_int(rhs, env));
                match *op {
                    PLUS => l.wrapping_add(r),
                    MINUS => l.wrapping_sub(r),
                    STAR => l.wrapping_mul(r),
                    SLASH => l / r,
                    PERCENT => l % r,
                    other => panic!("non-arithmetic op {other} in integer position"),
                }
            }
            Expr::Call { name, args } => {
                let idx = self.resolution.calls[name];
                let a: Vec<i32> = args.iter().map(|x| self.eval_int(x, env)).collect();
                self.run_fn(idx, &a)
            }
            Expr::Borrow { expr, .. } => self.eval_int(expr, env),
            Expr::Deref(inner) => self.eval_int(inner, env),
            Expr::Neg(inner) => self.eval_int(inner, env).wrapping_neg(),
            other => panic!("unexpected integer expr {other:?}"),
        }
    }

    fn eval_bool(&self, e: &Expr, env: &HashMap<BindingId, i32>) -> bool {
        match e {
            Expr::LiteralBool(_, b) => *b,
            Expr::Not(inner) => !self.eval_bool(inner, env),
            Expr::Binary { op, lhs, rhs } => {
                if *op == ANDAND {
                    return self.eval_bool(lhs, env) && self.eval_bool(rhs, env);
                }
                if *op == OROR {
                    return self.eval_bool(lhs, env) || self.eval_bool(rhs, env);
                }
                let (l, r) = (self.eval_int(lhs, env), self.eval_int(rhs, env));
                match *op {
                    LT => l < r,
                    GT => l > r,
                    LE => l <= r,
                    GE => l >= r,
                    EQ => l == r,
                    NE => l != r,
                    other => panic!("non-comparison op {other} in bool position"),
                }
            }
            other => panic!("unexpected bool expr {other:?}"),
        }
    }
}

pub(crate) fn ast_interp(src: &str, inputs: &[i32]) -> i32 {
    let (module, resolution) = frontend(src);
    let def_to_id = global_def_to_id(&resolution);
    let ev = Ev {
        module: &module,
        resolution: &resolution,
        def_to_id: &def_to_id,
    };
    ev.run_fn(module.functions.len() - 1, inputs)
}

struct Gen {
    state: u64,
}

impl Gen {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x517C_C1B7_2722_0A95,
        }
    }

    fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 33) as u32
    }

    fn expr(&mut self, nvars: usize, depth: u32, calls: bool, refs: bool) -> String {
        if depth > 0 {
            if calls && self.next() % 5 == 0 {
                return format!(
                    "h({}, {})",
                    self.expr(nvars, depth - 1, calls, refs),
                    self.expr(nvars, depth - 1, calls, refs)
                );
            }
            if refs && self.next() % 5 == 0 {
                let inner = self.expr(nvars, depth - 1, calls, refs);
                return if self.next() % 2 == 0 {
                    format!("*(&({inner}))")
                } else {
                    format!("d(&({inner}))")
                };
            }
            if self.next() % 6 == 0 {
                let op = if self.next() % 2 == 0 { "/" } else { "%" };
                return format!(
                    "({} {} {})",
                    self.expr(nvars, depth - 1, calls, refs),
                    op,
                    self.next() % 5 + 1
                );
            }
        }
        if depth == 0 || self.next() % 3 == 0 {
            if self.next() % 2 == 0 {
                format!("v{}", (self.next() as usize) % nvars)
            } else {
                format!("{}", self.next() % 6)
            }
        } else {
            let op = ["+", "-", "*"][(self.next() % 3) as usize];
            format!(
                "({} {} {})",
                self.expr(nvars, depth - 1, calls, refs),
                op,
                self.expr(nvars, depth - 1, calls, refs)
            )
        }
    }

    fn cond(&mut self, nvars: usize) -> String {
        self.cond_depth(nvars, 1)
    }

    fn cond_depth(&mut self, nvars: usize, depth: u32) -> String {
        if depth > 0 && self.next() % 4 == 0 {
            return format!("!({})", self.cond_depth(nvars, depth - 1));
        }
        if depth > 0 && self.next() % 3 == 0 {
            let op = if self.next() % 2 == 0 { "&&" } else { "||" };
            return format!(
                "({}) {} ({})",
                self.cond_depth(nvars, depth - 1),
                op,
                self.cond_depth(nvars, depth - 1)
            );
        }
        let op = ["<", ">", "<=", ">=", "==", "!="][(self.next() % 6) as usize];
        format!(
            "{} {} {}",
            self.expr(nvars, 1, false, false),
            op,
            self.expr(nvars, 1, false, false)
        )
    }
}

pub(crate) fn gen_program(seed: u64) -> (String, usize) {
    let mut g = Gen::new(seed);
    let calls = g.next() % 2 == 0;
    let refs = g.next() % 2 == 0;
    let mut module = String::new();
    if calls {
        module.push_str(&format!(
            "fn h(v0: i32, v1: i32) -> i32 {{ return {}; }}\n",
            g.expr(2, 2, false, false)
        ));
    }
    if refs {
        module.push_str("fn d(v0: &i32) -> i32 { return *v0; }\n");
    }
    let nparams = 1 + (g.next() % 3) as usize;
    let mut nvars = nparams;
    let params: Vec<String> = (0..nparams).map(|i| format!("v{i}: i32")).collect();
    module.push_str(&format!("fn f({}) -> i32 {{", params.join(", ")));
    let nlets = (g.next() % 3) as usize;
    for _ in 0..nlets {
        module.push_str(&format!(
            " let mut v{}: i32 = {};",
            nvars,
            g.expr(nvars, 2, calls, refs)
        ));
        nvars += 1;
    }
    if nvars > nparams {
        for _ in 0..(g.next() % 3) {
            let k = nparams + (g.next() as usize) % (nvars - nparams);
            module.push_str(&format!(" v{k} = {};", g.expr(nvars, 2, calls, refs)));
        }
    }
    if g.next() % 2 == 0 {
        module.push_str(&format!(" return {}; }}", g.expr(nvars, 2, calls, refs)));
    } else {
        module.push_str(&format!(
            " if {} {{ return {}; }} else {{ return {}; }} }}",
            g.cond(nvars),
            g.expr(nvars, 2, calls, refs),
            g.expr(nvars, 2, calls, refs)
        ));
    }
    (module, nparams)
}

pub(crate) fn gen_inputs(seed: u64, n: usize) -> Vec<i32> {
    let mut g = Gen::new(seed ^ 0xABCD_1234);
    (0..n).map(|_| (g.next() % 19) as i32 - 9).collect()
}

pub(crate) fn gen_while_program(seed: u64) -> (String, usize) {
    let mut g = Gen::new(seed ^ 0x5DEE_CE66_1357_9BDF);
    let nparams = 1 + (g.next() % 2) as usize;
    let i = nparams;
    let acc = nparams + 1;
    let bound = g.next() % 6 + 1;
    let params: Vec<String> = (0..nparams).map(|p| format!("v{p}: i32")).collect();
    let acc_init = g.expr(nparams, 1, false, false);
    let body = g.expr(nparams + 1, 1, false, false);
    (
        format!(
            "fn f({}) -> i32 {{ let mut v{i}: i32 = 0; let mut v{acc}: i32 = {acc_init}; \
             while v{i} < {bound} {{ v{acc} = v{acc} + {body}; v{i} = v{i} + 1; }} return v{acc}; }}",
            params.join(", ")
        ),
        nparams,
    )
}

pub(crate) fn gen_for_program(seed: u64) -> (String, usize) {
    let mut g = Gen::new(seed ^ 0xA11C_E5F0_2BCD_8891);
    let nparams = 1 + (g.next() % 2) as usize;
    let acc = nparams;
    let start = (g.next() % 7) as i32 - 3;
    let span = g.next() % 7;
    let end = start + span as i32;
    let params: Vec<String> = (0..nparams).map(|p| format!("v{p}: i32")).collect();
    let acc_init = g.expr(nparams, 1, false, false);
    let body = g.expr(nparams + 2, 1, false, false);
    (
        format!(
            "fn f({}) -> i32 {{ let mut v{acc}: i32 = {acc_init}; \
             for v{} in {start}..{end} {{ v{acc} += {body}; }} return v{acc}; }}",
            params.join(", "),
            acc + 1
        ),
        nparams,
    )
}

pub(crate) fn rustc_run(src: &str, inputs: &[i32]) -> Option<i32> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("vyre_lower_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let args = inputs
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let main = format!("\nfn main() {{ println!(\"{{}}\", f({args})); }}\n");
    let rs = dir.join("m.rs");
    std::fs::write(&rs, format!("{src}{main}")).expect("write");
    let exe = dir.join("m");
    let build = std::process::Command::new("rustc")
        .args(["--edition", "2021", "-O", "--cap-lints", "allow", "-o"])
        .arg(&exe)
        .arg(&rs)
        .output()
        .expect("rustc on PATH");
    let result = if build.status.success() {
        std::process::Command::new(&exe)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<i32>()
                    .ok()
            })
    } else {
        None
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}
