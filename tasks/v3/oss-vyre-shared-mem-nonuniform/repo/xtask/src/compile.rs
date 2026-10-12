//! `cargo_full run --bin xtask -- compile`  -  multi-target emitter harness.
//!
//! P7.4 contract: one IR → backend artifacts + a
//! byte-proof-equivalence certificate. The WGSL path is wired through
//! `vyre-driver-wgpu`; targets without an installed emitter fail with
//! an actionable error instead of writing synthetic artifacts.
//!
//! Usage:
//!
//! ```sh
//! cargo_full run --bin xtask -- compile <program.vir> \
//!     [--to wgsl] [--to spirv] [--to secondary_text] [--to native_module] [--to hlsl] \
//!     [--output-dir <dir>]
//! ```
//!
//! Every `--to TARGET` writes `<dir>/<fp>.<ext>` where `<fp>` is the
//! blake3 of the canonicalized IR (8 chars prefix for readability;
//! full 64-char form lives in the companion JSON manifest).

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

const MAX_XTASK_COMPILE_INPUT_BYTES: u64 = 64 * 1024 * 1024;

/// Supported compile targets. Each implies a file extension +
/// emitter path. The emitters themselves live in backend crates;
/// this enum is the frozen taxonomy consumers pin against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    PrimaryText,
    PrimaryBinary,
    SecondaryText,
    Metal,
    Hlsl,
}

impl Target {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "wgsl" => Some(Self::PrimaryText),
            "spirv" => Some(Self::PrimaryBinary),
            "secondary_text" => Some(Self::SecondaryText),
            "native_module" => Some(Self::Metal),
            "hlsl" => Some(Self::Hlsl),
            _ => None,
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Self::PrimaryText => "wgsl",
            Self::PrimaryBinary => "spv",
            Self::SecondaryText => "secondary_text",
            Self::Metal => "native_module",
            Self::Hlsl => "hlsl",
        }
    }
}

pub(crate) fn run(args: &[String]) {
    // Parse: compile <input> --to <t1> [--to <t2>] ... [--output-dir <d>]
    let mut idx = 2; // skip binary + "compile"
    if idx >= args.len() {
        eprintln!(
            "Fix: missing input wire file. Usage: cargo_full run --bin xtask -- compile <program.vir> --to <target>"
        );
        process::exit(2);
    }
    let input_path = PathBuf::from(&args[idx]);
    idx += 1;

    let mut targets: Vec<Target> = Vec::new();
    let mut out_dir = PathBuf::from("target/vyre-compile");

    while idx < args.len() {
        match args[idx].as_str() {
            "--to" => {
                idx += 1;
                if idx >= args.len() {
                    eprintln!("Fix: --to requires a target name");
                    process::exit(2);
                }
                match Target::parse(&args[idx]) {
                    Some(t) => targets.push(t),
                    None => {
                        eprintln!(
                            "Fix: unknown target '{}'. Supported: wgsl, spirv, secondary_text, native_module, hlsl",
                            args[idx]
                        );
                        process::exit(2);
                    }
                }
                idx += 1;
            }
            "--output-dir" => {
                idx += 1;
                if idx >= args.len() {
                    eprintln!("Fix: --output-dir requires a path");
                    process::exit(2);
                }
                out_dir = PathBuf::from(&args[idx]);
                idx += 1;
            }
            other => {
                eprintln!("Fix: unknown arg '{other}'");
                process::exit(2);
            }
        }
    }

    if targets.is_empty() {
        eprintln!(
            "Fix: no --to targets specified. Must pass at least one (wgsl, spirv, secondary_text, native_module, hlsl)."
        );
        process::exit(2);
    }

    let wire = match read_bytes_bounded(&input_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Fix: can't read {}: {e}", input_path.display());
            process::exit(1);
        }
    };

    // Decode + canonicalize (content-addressed fingerprint basis).
    let program = match vyre_foundation::ir::Program::from_wire(&wire) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Fix: wire decode failed: {e}");
            process::exit(1);
        }
    };
    let canonical =
        vyre_foundation::optimizer::passes::algebraic::canonicalize_engine::run(program);
    let canonical_wire = match canonical.to_wire() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Fix: canonical re-encode failed: {e}");
            process::exit(1);
        }
    };
    let fp = *blake3::hash(&canonical_wire).as_bytes();
    let mut fp_hex = String::with_capacity(fp.len() * 2);
    for b in &fp {
        use std::fmt::Write;
        write!(&mut fp_hex, "{b:02x}")
            .expect("Fix: format to String never fails; restore this invariant before continuing.");
    }
    let fp_prefix = &fp_hex[..16];

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("Fix: can't create output dir {}: {e}", out_dir.display());
        process::exit(1);
    }

    // Emit each target through the owning backend crate. Targets
    // without an installed emitter fail before writing a misleading
    // artifact.
    for target in &targets {
        let artifact_path = out_dir.join(format!("{fp_prefix}.{}", target.ext()));
        let artifact = match emit_target(*target, &canonical) {
            Ok(bytes) => bytes,
            Err(message) => {
                eprintln!("{message}");
                process::exit(1);
            }
        };
        if let Err(e) = fs::write(&artifact_path, artifact) {
            eprintln!("Fix: can't write {}: {e}", artifact_path.display());
            process::exit(1);
        }
        println!("emitted: {}", artifact_path.display());
    }

    // Manifest: full fingerprint + target list for proof-of-equivalence.
    let manifest_path = out_dir.join(format!("{fp_prefix}.manifest.json"));
    let manifest = serde_json_manifest(&fp_hex, &targets);
    if let Err(e) = fs::write(&manifest_path, manifest) {
        eprintln!("Fix: can't write manifest: {e}");
        process::exit(1);
    }
    println!("manifest: {}", manifest_path.display());
}

fn emit_target(
    target: Target,
    canonical: &vyre_foundation::ir::Program,
) -> Result<Vec<u8>, String> {
    match target {
        Target::PrimaryText => {
            let wgsl = vyre_driver_wgpu::emit::lower(canonical)
                .map_err(|error| format!("Fix: WGSL lowering failed: {error}"))?;
            Ok(wgsl.into_bytes())
        }
        Target::PrimaryBinary => Err(
            "Fix: SPIR-V artifact emission has no installed xtask emitter; use vyre-driver-spirv directly or add its emitter here before requesting --to spirv."
                .to_string(),
        ),
        Target::SecondaryText => Err(
            "Fix: PTX artifact emission requires the CUDA backend crate to provide an emitter before --to secondary_text can be used."
                .to_string(),
        ),
        Target::Metal => vyre_emit_metal::emit_program_artifact_bytes(canonical)
            .map_err(|error| format!("Fix: Metal native_module emission failed: {error}")),
        Target::Hlsl => Err(
            "Fix: HLSL artifact emission requires a DXC backend emitter before --to hlsl can be used."
                .to_string(),
        ),
    }
}

fn serde_json_manifest(fp_hex: &str, targets: &[Target]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"fingerprint\": \"{fp_hex}\",\n"));
    out.push_str("  \"targets\": [\n");
    for (i, t) in targets.iter().enumerate() {
        let comma = if i + 1 < targets.len() { "," } else { "" };
        out.push_str(&format!("    \"{}\"{comma}\n", t.ext()));
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

fn read_bytes_bounded(path: &PathBuf) -> io::Result<Vec<u8>> {
    let mut reader = fs::File::open(path)?.take(MAX_XTASK_COMPILE_INPUT_BYTES.saturating_add(1));
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_XTASK_COMPILE_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_XTASK_COMPILE_INPUT_BYTES} byte xtask compile input cap",
                path.display()
            ),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{emit_target, Target};
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

    #[test]
    fn native_module_target_emits_metal_artifact_json() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(16),
            ],
            [64, 1, 1],
            vec![Node::Store {
                buffer: Ident::from("out"),
                index: Expr::InvocationId { axis: 0 },
                value: Expr::LitU32(7),
            }],
        );

        let bytes = emit_target(Target::Metal, &program).expect(
            "Fix: xtask native_module target must route through vyre-emit-metal instead of the historical placeholder error.",
        );
        let json: serde_json::Value = serde_json::from_slice(&bytes)
            .expect("Fix: native_module target must emit structured JSON artifact bytes.");

        assert_eq!(json["target"], "native_module");
        let entry_point = json["entry_point"]
            .as_str()
            .expect("Fix: native_module artifact must record the emitted Metal function name.");
        assert_eq!(json["workgroup_size"], serde_json::json!([64, 1, 1]));
        assert_eq!(json["schema"], 3);
        assert!(
            json["msl"]
                .as_str()
                .is_some_and(|source| !source.is_empty() && source.contains(entry_point)),
            "native_module artifact must carry generated MSL source containing the emitted entry point"
        );
        assert_eq!(json["bindings"][0]["name"], "out");
        assert_eq!(json["bindings"][0]["metal_buffer_index"], 0);
        assert_eq!(json["sizes_buffer_index"], 1);
    }
}
