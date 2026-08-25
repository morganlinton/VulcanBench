//! CLI entry point for vyre-grammar-gen.
//!
//! Emits the C11 DFA lexer table as a binary blob
//! suitable for uploading to the GPU as ReadOnly storage buffers.

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use vyre_grammar_gen::{
    c11_lexer::build_c11_lexer_dfa, dfa::DfaBuilder, DfaTable, LrTable, PackedBlob,
};

/// Command-line interface.
#[derive(Parser)]
#[command(
    name = "vyre-grammar-gen",
    version,
    about = "Compile C11 lexer grammar into a GPU-ready table."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Emit the C11 lexer table to disk.
    Emit {
        /// Output directory.
        #[arg(long, default_value = "./rules/c11")]
        out_dir: PathBuf,
        /// Use a tiny synthetic lexer DFA for CLI smoke tests.
        #[arg(long, default_value_t = false)]
        smoke_lexer: bool,
        /// `bin` (default) or `json` sidecar metadata next to `.bin` files.
        #[arg(long, value_enum, default_value_t = EmitFormat::Bin)]
        format: EmitFormat,
        /// Optional JSON-encoded `LrTable` to serialize as `c11_lr_tables.bin`.
        #[arg(long)]
        lr_json: Option<PathBuf>,
    },
    /// Print a hex dump of the lexer DFA blob to stdout.
    DumpLexer {
        /// Same as emit: use the tiny synthetic DFA instead of full C11 table.
        #[arg(long, default_value_t = false)]
        smoke_lexer: bool,
    },
    /// Print a hex dump of a caller-supplied LR table blob to stdout.
    DumpLr {
        /// JSON-encoded `LrTable` to serialize and dump.
        #[arg(long)]
        lr_json: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum EmitFormat {
    /// Only `.bin` files.
    #[default]
    Bin,
    /// `.bin` plus `.json` sidecars (metadata, not a second wire format).
    Json,
}

fn lexer_dfa_table(smoke: bool) -> Result<DfaTable, Box<dyn std::error::Error>> {
    if smoke {
        Ok(DfaBuilder::new(4, 32).build().map_err(|e| {
            format!("Fix: smoke DFA build failed: {e}")
        })?)
    } else {
        Ok(build_c11_lexer_dfa())
    }
}

fn write_json_sidecar(path: &PathBuf, label: &str, blob: &PackedBlob) {
    let j = json!({
        "format": "vyre-grammar-gen-sidecar-v0",
        "label": label,
        "kind": format!("{:?}", blob.kind),
        "byte_length": blob.bytes.len(),
    });
    if let Ok(s) = serde_json::to_string_pretty(&j) {
        let _ = fs::write(path, s);
    }
}

fn lr_blob_from_json(path: &PathBuf) -> Result<PackedBlob, Box<dyn std::error::Error>> {
    let bytes = read_file_bounded(path, 64 * 1024 * 1024)?;
    let table: LrTable = serde_json::from_slice(&bytes)?;
    Ok(PackedBlob::from_lr(&table).map_err(|e| {
        format!(
            "Fix: LR table from '{}' failed validation before packing: {e}",
            path.display()
        )
    })?)
}

fn read_file_bounded(
    path: &PathBuf,
    max_bytes: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes as u64 {
        return Err(format!(
            "{} exceeds {max_bytes} byte grammar input cap",
            path.display()
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(len as usize);
    let mut buf = [0u8; 8192];
    while bytes.len() <= max_bytes {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..read]);
        if bytes.len() > max_bytes {
            return Err(format!(
                "{} exceeded {max_bytes} byte grammar input cap while reading",
                path.display()
            )
            .into());
        }
    }
    Ok(bytes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Emit {
            out_dir,
            smoke_lexer,
            format,
            lr_json,
        } => {
            let dfa = lexer_dfa_table(smoke_lexer)?;

            fs::create_dir_all(&out_dir)?;

            let lexer_blob = PackedBlob::from_dfa(&dfa);
            let lr_blob = match lr_json {
                Some(path) => Some(lr_blob_from_json(&path)?),
                None => None,
            };

            let lexer_path = out_dir.join("c11_lexer_dfa.bin");
            fs::write(&lexer_path, &lexer_blob.bytes)?;
            if let Some(blob) = &lr_blob {
                fs::write(out_dir.join("c11_lr_tables.bin"), &blob.bytes)?;
            }

            if format == EmitFormat::Json {
                write_json_sidecar(
                    &out_dir.join("c11_lexer_dfa.json"),
                    "lexer_dfa",
                    &lexer_blob,
                );
                if let Some(blob) = &lr_blob {
                    write_json_sidecar(&out_dir.join("c11_lr_tables.json"), "lr_tables", blob);
                }
            }

            match &lr_blob {
                Some(blob) => writeln!(
                    std::io::stdout().lock(),
                    "wrote {} bytes {} + {} bytes c11_lr_tables.bin to {} (smoke_lexer={})",
                    lexer_blob.bytes.len(),
                    lexer_path.display(),
                    blob.bytes.len(),
                    out_dir.display(),
                    smoke_lexer
                )?,
                None => writeln!(
                    std::io::stdout().lock(),
                    "wrote {} bytes {} to {} (smoke_lexer={})",
                    lexer_blob.bytes.len(),
                    lexer_path.display(),
                    out_dir.display(),
                    smoke_lexer
                )?,
            }
        }
        Command::DumpLexer { smoke_lexer } => {
            let dfa = lexer_dfa_table(smoke_lexer)?;
            let blob = PackedBlob::from_dfa(&dfa);
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            for (i, chunk) in blob.bytes.chunks(16).enumerate() {
                write!(out, "{:08x}  ", i * 16)?;
                for b in chunk {
                    write!(out, "{b:02x} ")?;
                }
                writeln!(out)?;
            }
        }
        Command::DumpLr { lr_json } => {
            let blob = lr_blob_from_json(&lr_json)?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            for (i, chunk) in blob.bytes.chunks(16).enumerate() {
                write!(out, "{:08x}  ", i * 16)?;
                for b in chunk {
                    write!(out, "{b:02x} ")?;
                }
                writeln!(out)?;
            }
        }
    }

    Ok(())
}
