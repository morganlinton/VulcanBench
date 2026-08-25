use clap::{Parser, Subcommand};
use std::path::Path;
use std::process::exit;
use vyre_debug::{
    bisect_rewrites, carrier_summary, diff_descriptors, dump_descriptor, dump_wgsl,
    find_dangling_refs, find_uncarriered_assigns, fixtures::loop_carry_smoke,
    DescriptorDumpOptions,
};
use vyre_foundation::ir::Expr;
use vyre_foundation::ir::Program;

#[derive(Parser)]
#[command(name = "vyre-dbg")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    DumpDescriptor {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
    },
    DumpWgsl {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
        #[arg(long)]
        lines: bool,
    },
    FindDangling {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    FindUncarriered {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    CarrierSummary {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    BisectRewrites {
        #[arg(long)]
        prog: String,
        #[arg(long)]
        num_tokens: Option<usize>,
    },
    DiffDescriptors {
        #[arg(long)]
        prog_a: String,
        #[arg(long)]
        prog_b: String,
    },
    FailureTrace {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        id: String,
    },
    EmitReplay {
        #[arg(long)]
        kdesc: String,
    },
    DiffEmit {
        #[arg(long)]
        kdesc_a: String,
        #[arg(long)]
        kdesc_b: String,
    },
    PipelineCacheClear,
}

fn get_program(name: &str, num_tokens: Option<usize>) -> Result<Program, String> {
    let tokens = num_tokens.unwrap_or(4);
    match name {
        "c11_lexer" => Ok(vyre_libs::parsing::c::lex::lexer::c11_lexer(
            "hs",
            "tt",
            "ts",
            "tl",
            "tc",
            tokens as u32,
        )),
        "c11_extract_calls" => Ok(vyre_libs::parsing::c::parse::structure::c11_extract_calls(
            "tt",
            "pp",
            "fns",
            Expr::u32(tokens as u32),
            Expr::u32(tokens as u32),
            "oc",
            "cn",
        )),
        "c11_build_vast_nodes" => Ok(vyre_libs::parsing::c::parse::vast::c11_build_vast_nodes(
            "tt",
            "ts",
            "tl",
            Expr::u32(tokens as u32),
            "vast",
            "count",
        )),
        "c_lower_ast_to_pg_semantic_graph" => Ok(
            vyre_libs::parsing::c::pipeline::stages::c_lower_ast_to_pg_semantic_graph(
                "vast",
                Expr::u32(tokens as u32),
                "out_pg_nodes",
                "out_pg_edges",
            ),
        ),
        "bracket_match" => Ok(vyre_primitives::matching::bracket_match::bracket_match(
            "k",
            "s",
            "mp",
            tokens as u32,
            tokens as u32,
        )),
        "loop_carry_smoke" => Ok(loop_carry_smoke()),
        _ => Err(format!("unknown program: {}", name)),
    }
}

fn print_json_or_exit<T: ?Sized + serde::Serialize>(value: &T, context: &str) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("Failed to serialize {context} as JSON: {e}");
            exit(2);
        }
    }
}

fn read_kdesc(path: &Path) -> Result<vyre_lower::KernelDescriptor, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open kdesc {}: {e}", path.display()))?;
    bincode::serde::decode_from_std_read(&mut file, bincode::config::standard())
        .map_err(|e| format!("Failed to decode kdesc {}: {e}", path.display()))
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::DumpDescriptor { prog, num_tokens } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let desc = match vyre_lower::lower(&p) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let dump = dump_descriptor(&desc, &DescriptorDumpOptions::default());
            println!("{}", dump.text);
            exit(0);
        }
        Commands::DumpWgsl {
            prog,
            num_tokens,
            lines,
        } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            // Note: Plan mentions WgslDump but we just implemented `dump_wgsl` returning `Result<String, String>`
            match dump_wgsl(&p) {
                Ok(wgsl) => {
                    if lines {
                        for (i, line) in wgsl.text.lines().enumerate() {
                            println!("{:5} | {}", i + 1, line);
                        }
                    } else {
                        println!("{}", wgsl.text);
                    }
                    exit(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    exit(2);
                }
            }
        }
        Commands::FindDangling {
            prog,
            num_tokens,
            json,
        } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let desc = match vyre_lower::lower(&p) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let dangling = find_dangling_refs(&desc);
            if json {
                print_json_or_exit(&dangling, "dangling reference report");
            } else {
                println!("{} dangling", dangling.len());
                for d in &dangling {
                    println!("{:?}", d);
                }
            }
            if dangling.is_empty() {
                exit(0);
            } else {
                exit(1);
            }
        }
        Commands::FindUncarriered {
            prog,
            num_tokens,
            json,
        } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let desc = match vyre_lower::lower(&p) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let assigns = find_uncarriered_assigns(&p, &desc);
            if json {
                print_json_or_exit(&assigns, "uncarriered assignment report");
            } else {
                println!("{} uncarriered", assigns.len());
                for a in &assigns {
                    println!("{:?}", a);
                }
            }
            if assigns.is_empty() {
                exit(0);
            } else {
                exit(1);
            }
        }
        Commands::CarrierSummary {
            prog,
            num_tokens,
            json,
        } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let desc = match vyre_lower::lower(&p) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let summary = carrier_summary(&desc);
            if json {
                print_json_or_exit(&summary, "carrier summary");
            } else {
                println!("{:?}", summary);
            }
            exit(0); // This command is informational
        }
        Commands::BisectRewrites { prog, num_tokens } => {
            let p = match get_program(&prog, num_tokens) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            match bisect_rewrites(&p) {
                Ok(res) => {
                    println!("{:?}", res.first_failing_rewrite);
                    if res.first_failing_rewrite.is_none() {
                        exit(0);
                    } else {
                        exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Bisect failed: {:?}", e);
                    exit(2);
                }
            }
        }
        Commands::DiffDescriptors { prog_a, prog_b } => {
            let p_a = match get_program(&prog_a, None) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let p_b = match get_program(&prog_b, None) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    exit(3);
                }
            };
            let desc_a = match vyre_lower::lower(&p_a) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let desc_b = match vyre_lower::lower(&p_b) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Lowering failed: {:?}", e);
                    exit(2);
                }
            };
            let diff = diff_descriptors(&desc_a, &desc_b);
            println!("Bindings dropped: {:?}", diff.bindings_dropped);
            println!("Bindings added: {:?}", diff.bindings_added);
            println!("Op count delta: {:?}", diff.op_count_delta);
            println!("Root shape changed: {}", diff.root_shape_changed);
            exit(0);
        }
        Commands::FailureTrace { dir, id } => {
            let path = std::path::Path::new(&dir);
            let kdesc_path = path.join(format!("{id}.kdesc.bin"));
            let desc = match read_kdesc(&kdesc_path) {
                Ok(desc) => desc,
                Err(e) => {
                    eprintln!("{e}");
                    exit(1);
                }
            };

            match vyre_emit_naga::emit(&desc) {
                Ok(_) => {
                    println!("Replay succeeded without error.");
                    exit(0);
                }
                Err(e) => {
                    println!("Failure trace: {e}");
                    exit(0);
                }
            }
        }
        Commands::EmitReplay { kdesc } => {
            let desc = match read_kdesc(Path::new(&kdesc)) {
                Ok(desc) => desc,
                Err(e) => {
                    eprintln!("{e}");
                    exit(1);
                }
            };
            match vyre_emit_naga::emit(&desc) {
                Ok(m) => {
                    println!("Emission succeeded. Module dump:");
                    let dump = vyre_debug::dump_naga_module(&m);
                    println!("{}", dump.text);
                }
                Err(e) => {
                    eprintln!("Emission failed: {e}");
                    exit(1);
                }
            }
            exit(0);
        }
        Commands::DiffEmit { kdesc_a, kdesc_b } => {
            let desc_a = match read_kdesc(Path::new(&kdesc_a)) {
                Ok(desc) => desc,
                Err(e) => {
                    eprintln!("{e}");
                    exit(1);
                }
            };
            let desc_b = match read_kdesc(Path::new(&kdesc_b)) {
                Ok(desc) => desc,
                Err(e) => {
                    eprintln!("{e}");
                    exit(1);
                }
            };

            let module_a = match vyre_emit_naga::emit(&desc_a) {
                Ok(module) => module,
                Err(e) => {
                    eprintln!("Emission failed for {}: {e}", kdesc_a);
                    exit(1);
                }
            };
            let module_b = match vyre_emit_naga::emit(&desc_b) {
                Ok(module) => module,
                Err(e) => {
                    eprintln!("Emission failed for {}: {e}", kdesc_b);
                    exit(1);
                }
            };

            let dump_a = vyre_debug::dump_naga_module(&module_a);
            let dump_b = vyre_debug::dump_naga_module(&module_b);

            println!("--- {} ({} bytes)", kdesc_a, dump_a.text.len());
            println!("+++ {} ({} bytes)", kdesc_b, dump_b.text.len());
            exit(0);
        }
        Commands::PipelineCacheClear => {
            if let Ok(home) = std::env::var("HOME") {
                let cache_dir = std::path::Path::new(&home).join(".cache/vyre/pipeline");
                match std::fs::remove_dir_all(&cache_dir) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        eprintln!("failed to clear pipeline cache {:?}: {error}", cache_dir);
                        exit(1);
                    }
                }
                println!("Cleared {:?}", cache_dir);
            }
            exit(0);
        }
    }
}
