//! Agent-harness smoke: drive `vyre_primitives::wire` from a CLI.
//!
//! Reads a line from stdin in one of:
//!   `pack-u32 <comma-separated u32 list>`
//!   `pack-f32 <comma-separated f32 list>`
//!   `unpack-u32 <hex byte string> <count>`
//!   `unpack-f32 <hex byte string> <count>`
//!
//! Emits the packed bytes (hex) or decoded values (comma-separated) on
//! stdout. Exit code is `0` on success and `1` with a one-line error
//! message on stderr otherwise.
//!
//! Designed for an agent harness to invoke as a deterministic
//! subprocess: no stdout chatter unless asked, no network, no GPU, no
//! environment dependencies. Re-running with identical stdin must
//! produce byte-identical stdout.
//!
//! Build:
//!   `cargo build --release --example wire_harness_smoke -p vyre-primitives`
//!
//! Example:
//!   `echo "pack-u32 1,2,3" | wire_harness_smoke
//!     -> "01000000020000000300000000"  (hex)`

#![allow(missing_docs)]

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use vyre_primitives::wire::{
    decode_f32_le_bytes_all, decode_u32_le_bytes_all, pack_f32_slice, pack_u32_slice,
};

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("hex string must have even length".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks_exact(2) {
        let high = char_to_nibble(chunk[0])?;
        let low = char_to_nibble(chunk[1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn char_to_nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("non-hex character: {:?}", c as char)),
    }
}

fn parse_u32_list(s: &str) -> Result<Vec<u32>, String> {
    s.split(',')
        .map(|tok| tok.trim().parse::<u32>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_f32_list(s: &str) -> Result<Vec<f32>, String> {
    s.split(',')
        .map(|tok| tok.trim().parse::<f32>().map_err(|e| e.to_string()))
        .collect()
}

fn render_u32_list(values: &[u32]) -> String {
    values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn render_f32_list(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| format!("{v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn run_line(line: &str) -> Result<String, String> {
    let mut parts = line.split_whitespace();
    let cmd = parts.next().ok_or_else(|| "empty input line".to_string())?;
    match cmd {
        "pack-u32" => {
            let payload = parts
                .next()
                .ok_or_else(|| "pack-u32 needs a payload".to_string())?;
            let values = parse_u32_list(payload)?;
            Ok(hex_encode(&pack_u32_slice(&values)))
        }
        "pack-f32" => {
            let payload = parts
                .next()
                .ok_or_else(|| "pack-f32 needs a payload".to_string())?;
            let values = parse_f32_list(payload)?;
            Ok(hex_encode(&pack_f32_slice(&values)))
        }
        "unpack-u32" => {
            let payload = parts
                .next()
                .ok_or_else(|| "unpack-u32 needs hex payload".to_string())?;
            let bytes = hex_decode(payload)?;
            let _count: usize = parts
                .next()
                .ok_or_else(|| "unpack-u32 needs count".to_string())?
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
            // count is informational; decode_u32_le_bytes_all consumes the whole buffer.
            Ok(render_u32_list(&decode_u32_le_bytes_all(&bytes)))
        }
        "unpack-f32" => {
            let payload = parts
                .next()
                .ok_or_else(|| "unpack-f32 needs hex payload".to_string())?;
            let bytes = hex_decode(payload)?;
            let _count: usize = parts
                .next()
                .ok_or_else(|| "unpack-f32 needs count".to_string())?
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?;
            Ok(render_f32_list(&decode_f32_le_bytes_all(&bytes)))
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn main() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("stdin read error: {err}");
                return ExitCode::from(1);
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match run_line(line) {
            Ok(reply) => {
                if writeln!(out, "{reply}").is_err() {
                    return ExitCode::from(1);
                }
            }
            Err(err) => {
                had_error = true;
                eprintln!("{err}");
                if writeln!(out, "ERR").is_err() {
                    return ExitCode::from(1);
                }
            }
        }
    }
    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
