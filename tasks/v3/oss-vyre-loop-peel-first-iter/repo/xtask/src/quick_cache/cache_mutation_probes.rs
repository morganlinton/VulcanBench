#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick::{QuickOp, QuickStatus};
use crate::quick_cache::{
    atomic_write_new, cached_outcome, encode_cache_component, evaluate_mutation,
    mutation_cache_json, mutations_for,
};
use crate::{hash, paths};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Instant;

const MAX_CACHE_MUTATION_SOURCE_BYTES: u64 = 2_097_152;

pub(crate) fn cache_mutation_probes(op: &QuickOp, source_file: &Path) -> (QuickStatus, String) {
    let source = match read_text_bounded(source_file) {
        Ok(source) => source,
        Err(err) => {
            return (
                QuickStatus::Fail,
                format!("could not read {}: {err}", source_file.display()),
            );
        }
    };

    let cache_dir = paths::quick_cache_dir();
    if let Err(err) = fs::create_dir_all(&cache_dir) {
        return (
            QuickStatus::Fail,
            format!("could not create {}: {err}", cache_dir.display()),
        );
    }

    let source_hash = hash::sha256_hex(source.as_bytes());
    let test_name = format!("quick-check::{op_id}", op_id = op.id);
    let test_hash = hash::sha256_hex(test_name.as_bytes());
    let mut hits = 0usize;
    let mut writes = 0usize;
    let mut survived = Vec::new();
    let mutations = mutations_for(op);

    for mutation in &mutations {
        let cache_file = cache_dir.join(format!(
            "{}_{}_{}.json",
            source_hash,
            test_hash,
            encode_cache_component(mutation.id)
        ));

        match cached_outcome(&cache_file) {
            Ok(Some(outcome)) => {
                hits += 1;
                if outcome == "survived" {
                    survived.push(mutation.id.to_string());
                }
                continue;
            }
            Ok(None) => {}
            Err(err) => return (QuickStatus::Fail, err),
        }

        let start = Instant::now();
        let outcome = evaluate_mutation(op, &source, *mutation);
        if outcome == "survived" {
            survived.push(mutation.id.to_string());
        }

        let payload = mutation_cache_json(
            &source_hash,
            &test_hash,
            mutation.id,
            outcome,
            start.elapsed(),
        );
        if let Err(err) = atomic_write_new(&cache_file, payload.as_bytes()) {
            return (
                QuickStatus::Fail,
                format!("could not write {}: {err}", cache_file.display()),
            );
        }
        writes += 1;
    }

    if !survived.is_empty() {
        return (
            QuickStatus::Fail,
            format!("survived mutations: {}", survived.join(", ")),
        );
    }

    (
        QuickStatus::Pass,
        format!(
            "{} probes cached ({} hits, {} writes)",
            mutations.len(),
            hits,
            writes
        ),
    )
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_CACHE_MUTATION_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_CACHE_MUTATION_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_CACHE_MUTATION_SOURCE_BYTES} byte cache mutation source read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
