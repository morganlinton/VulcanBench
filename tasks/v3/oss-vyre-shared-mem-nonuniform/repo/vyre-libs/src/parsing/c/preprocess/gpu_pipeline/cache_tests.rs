use std::path::PathBuf;

use super::cache::{
    cache_key_stem, classified_cache_key, decode_classified, decode_payloads, encode_classified,
    encode_payloads, macro_fingerprint, payloads_cache_key, production_payloads_cache_key,
    read_disk_cache_file_bounded_with_limit, ClassifiedCacheKey, DecodeError, PayloadCache,
    PayloadsCacheKey, CLASSIFIED_DISK_MAGIC,
};
use super::live_conditional_cache::{LiveConditionalCache, LiveConditionalCacheKey};
use super::{ClassifiedTokens, DirectivePayload};

#[test]
fn classified_round_trip_through_encode_decode() {
    let key = ClassifiedCacheKey {
        path: PathBuf::from("/tmp/vyre-test-fixture-decode/header.h"),
        source_len: 42,
        source_hash: [0xde; 16],
    };
    let original = ClassifiedTokens {
        tok_types: vec![1, 2, 3, 4],
        tok_starts: vec![0, 5, 10, 15],
        tok_lens: vec![5, 5, 5, 5],
        directive_kinds: vec![0, 0, 7, 0],
        directive_count: 1,
        source: std::sync::Arc::from(&b"int main(void){return 0;}"[..]),
    };
    let encoded = encode_classified(&key, &original)
        .expect("Fix: classified cache encoding must reserve exactly.");
    let decoded = decode_classified(&encoded, &key)
        .expect("Fix: classified cache encoding must round-trip for a matching key.");
    assert_eq!(decoded, original);
}

#[test]
fn decode_rejects_mismatched_key() {
    let key = ClassifiedCacheKey {
        path: PathBuf::from("/tmp/vyre-test-fixture-mismatch/a.h"),
        source_len: 1,
        source_hash: [1; 16],
    };
    let other = ClassifiedCacheKey {
        path: PathBuf::from("/tmp/vyre-test-fixture-mismatch/b.h"),
        source_len: 1,
        source_hash: [1; 16],
    };
    let classified = ClassifiedTokens {
        tok_types: Vec::new(),
        tok_starts: Vec::new(),
        tok_lens: Vec::new(),
        directive_kinds: Vec::new(),
        directive_count: 0,
        source: std::sync::Arc::from([]),
    };
    let encoded = encode_classified(&key, &classified)
        .expect("Fix: classified cache encoding must reserve exactly.");
    assert!(matches!(
        decode_classified(&encoded, &other),
        Err(DecodeError::KeyMismatch)
    ));
}

#[test]
fn decode_rejects_bad_magic() {
    let key = ClassifiedCacheKey {
        path: PathBuf::from("/x"),
        source_len: 0,
        source_hash: [0; 16],
    };
    let mut bytes = b"NOTVYRE_".to_vec();
    bytes.extend_from_slice(&[0u8; 64]);
    assert!(matches!(
        decode_classified(&bytes, &key),
        Err(DecodeError::BadMagic)
    ));
}

#[test]
fn decode_rejects_truncated() {
    let key = ClassifiedCacheKey {
        path: PathBuf::from("/x"),
        source_len: 0,
        source_hash: [0; 16],
    };
    let mut bytes = CLASSIFIED_DISK_MAGIC.to_vec();
    bytes.push(0);
    assert!(matches!(
        decode_classified(&bytes, &key),
        Err(DecodeError::Truncated)
    ));
}

#[test]
fn source_bearing_cache_keys_use_128_bit_fingerprints() {
    let key = classified_cache_key(std::path::Path::new("/tmp/a.h"), b"#define A 1\n");
    assert_eq!(key.source_hash.len(), 16);
    let stem = cache_key_stem(
        key.path.as_os_str().as_encoded_bytes(),
        key.source_len,
        key.source_hash,
        None,
    );
    assert_eq!(stem.len(), 32);
}

fn sample_payload_key() -> PayloadsCacheKey {
    PayloadsCacheKey {
        path: PathBuf::from("/tmp/vyre-payloads-fixture/h.h"),
        source_len: 200,
        source_hash: [0xfe; 16],
        macro_fingerprint: [0xa1; 16],
    }
}

fn sample_payloads() -> Vec<DirectivePayload> {
    vec![
        DirectivePayload::None,
        DirectivePayload::Define {
            name: b"FOO".to_vec(),
            name_start: 8,
            name_len: 3,
            args: b"x,y".to_vec(),
            args_start: 12,
            args_len: 3,
            body: b"((x)+(y))".to_vec(),
            body_start: 17,
            body_len: 9,
            is_function_like: true,
        },
        DirectivePayload::Undef {
            name: b"OLD".to_vec(),
        },
        DirectivePayload::Include {
            path: b"stdio.h".to_vec(),
            is_system: true,
            is_next: false,
        },
        DirectivePayload::Ifdef {
            value: 1,
            negated: false,
        },
        DirectivePayload::IfExpr {
            value: 0,
            is_elif: true,
        },
        DirectivePayload::Else,
        DirectivePayload::Endif,
        DirectivePayload::Other,
    ]
}

#[test]
fn payloads_round_trip_through_encode_decode() {
    let key = sample_payload_key();
    let payloads = sample_payloads();
    let encoded = encode_payloads(&key, &payloads)
        .expect("Fix: payload cache encoding must reserve exactly.");
    let decoded = decode_payloads(&encoded, &key)
        .expect("Fix: payload cache encoding must round-trip for a matching key.");
    assert_eq!(decoded, payloads);
}

#[test]
fn payloads_decode_rejects_macro_fingerprint_change() {
    let key = sample_payload_key();
    let mut other = sample_payload_key();
    other.macro_fingerprint[0] ^= 1;
    let encoded = encode_payloads(&key, &sample_payloads())
        .expect("Fix: payload cache encoding must reserve exactly.");
    assert!(matches!(
        decode_payloads(&encoded, &other),
        Err(DecodeError::KeyMismatch)
    ));
}

#[test]
fn payloads_decode_rejects_path_change() {
    let key = sample_payload_key();
    let mut other = sample_payload_key();
    other.path = PathBuf::from("/tmp/vyre-payloads-fixture/other.h");
    let encoded = encode_payloads(&key, &sample_payloads())
        .expect("Fix: payload cache encoding must reserve exactly.");
    assert!(matches!(
        decode_payloads(&encoded, &other),
        Err(DecodeError::KeyMismatch)
    ));
}

#[test]
fn payloads_decode_rejects_bad_magic() {
    let key = sample_payload_key();
    let mut bytes = b"NOTPL___".to_vec();
    bytes.extend_from_slice(&[0u8; 32]);
    assert!(matches!(
        decode_payloads(&bytes, &key),
        Err(DecodeError::BadMagic)
    ));
}

#[test]
fn macro_fingerprint_changes_with_macro_set() {
    let a = macro_fingerprint(&[b"FOO".as_slice(), b"BAR".as_slice()]);
    let b = macro_fingerprint(&[b"FOO".as_slice()]);
    let c = macro_fingerprint(&[b"FOO".as_slice(), b"BAR".as_slice()]);
    assert_ne!(a, b, "removing a macro must change the fingerprint");
    assert_eq!(a, c, "same macro set must produce the same fingerprint");
}

#[test]
fn production_payload_cache_key_is_macro_independent() {
    let path = std::path::Path::new("/tmp/vyre-payloads-fixture/h.h");
    let source = b"#if F\nint x;\n#endif\n";
    let production = production_payloads_cache_key(path, source);
    let compatibility = payloads_cache_key(path, source, &[b"F".as_slice()]);
    assert_eq!(
        production,
        payloads_cache_key(path, source, &[]),
        "production payload extraction must key only on path and source"
    );
    assert_ne!(
        production, compatibility,
        "compatibility snapshot extraction may still key on the supplied macro set"
    );
}

fn temp_disk_cache_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "vyre_gpu_preprocess_disk_cache_{}_{}_{:?}",
        name,
        std::process::id(),
        std::thread::current().id()
    ));
    path
}

#[test]
fn bounded_disk_cache_read_accepts_entry_at_limit() {
    let path = temp_disk_cache_path("at_limit");
    std::fs::write(&path, [0xA7u8; 8]).expect("Fix: temp cache fixture must be writable");

    let bytes = read_disk_cache_file_bounded_with_limit(&path, "test", 8)
        .expect("Fix: cache entry at byte cap must not report an I/O error")
        .expect("Fix: cache entry at byte cap must be readable");

    assert_eq!(bytes.len(), 8);
    std::fs::remove_file(path).ok();
}

#[test]
fn bounded_disk_cache_read_removes_entry_over_limit() {
    let path = temp_disk_cache_path("over_limit");
    std::fs::write(&path, [0x5Cu8; 9]).expect("Fix: temp cache fixture must be writable");

    let bytes = read_disk_cache_file_bounded_with_limit(&path, "test", 8);

    assert!(bytes
        .expect("Fix: oversized cache entry should be removable")
        .is_none());
    assert!(!path.exists());
}

fn payload_cache_key(id: u8) -> PayloadsCacheKey {
    PayloadsCacheKey {
        path: PathBuf::from(format!("/tmp/vyre-payload-cache-{id}.h")),
        source_len: id as usize,
        source_hash: [id; 16],
        macro_fingerprint: [0; 16],
    }
}

fn cached_payloads(id: u8) -> std::sync::Arc<[DirectivePayload]> {
    std::sync::Arc::from(
        vec![DirectivePayload::Define {
            name: vec![b'A' + id],
            name_start: 0,
            name_len: 1,
            args: Vec::new(),
            args_start: 0,
            args_len: 0,
            body: vec![id],
            body_start: 2,
            body_len: 1,
            is_function_like: false,
        }]
        .into_boxed_slice(),
    )
}

#[test]
fn payload_memory_cache_reuses_arc_entries_and_evicts_lru() {
    let mut cache = PayloadCache::with_limit(2);
    let a = payload_cache_key(1);
    let b = payload_cache_key(2);
    let c = payload_cache_key(3);
    let a_payloads = cached_payloads(1);
    cache.insert(a.clone(), std::sync::Arc::clone(&a_payloads));
    cache.insert(b.clone(), cached_payloads(2));
    let hit = cache.lookup(&a).expect("Fix: payload cache should hit");
    assert!(std::sync::Arc::ptr_eq(&a_payloads, &hit));
    cache.insert(c.clone(), cached_payloads(3));
    assert!(cache.contains_key(&a));
    assert!(!cache.contains_key(&b));
    assert!(cache.contains_key(&c));
    assert_eq!(cache.len(), 2);
}

fn live_conditional_key(id: u8) -> LiveConditionalCacheKey {
    LiveConditionalCacheKey {
        evaluator: id,
        directive_kind: id as u32,
        negated: false,
        row_fingerprint: [id; 16],
        row_len: 1,
        macro_fingerprint: [id; 16],
        macro_names_len: id as u32,
        num_macros: id as u32,
    }
}

#[test]
fn live_conditional_memory_cache_evicts_least_recently_used_entry() {
    let mut cache = LiveConditionalCache::with_limit(2);
    let a = live_conditional_key(1);
    let b = live_conditional_key(2);
    let c = live_conditional_key(3);
    cache.insert(a.clone(), true);
    cache.insert(b.clone(), false);
    assert_eq!(cache.lookup(&a), Some(true));
    cache.insert(c.clone(), true);
    assert!(cache.contains_key(&a));
    assert!(!cache.contains_key(&b));
    assert!(cache.contains_key(&c));
    assert_eq!(cache.len(), 2);
}
