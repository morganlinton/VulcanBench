use super::classified_memory::{ClassifiedCacheKey, ClassifiedTokenCache};
use super::payload_keys::PayloadsCacheKey;
use super::payload_memory::PayloadCache;
use crate::parsing::c::preprocess::gpu_pipeline::payload_size::directive_payloads_bytes;
use crate::parsing::c::preprocess::gpu_pipeline::ClassifiedTokens;
use crate::parsing::c::preprocess::gpu_pipeline::DirectivePayload;
use std::sync::Arc;

fn key(id: u8) -> ClassifiedCacheKey {
    ClassifiedCacheKey {
        path: std::path::PathBuf::from(format!("/tmp/vyre-classified-cache-{id}.h")),
        source_len: id as usize,
        source_hash: [id; 16],
    }
}

fn classified(id: u8) -> ClassifiedTokens {
    classified_with_source(id, 1)
}

fn classified_with_source(id: u8, source_len: usize) -> ClassifiedTokens {
    ClassifiedTokens {
        tok_types: vec![id as u32],
        tok_starts: vec![0],
        tok_lens: vec![source_len as u32],
        directive_kinds: vec![0],
        directive_count: 0,
        source: Arc::from(vec![id; source_len].into_boxed_slice()),
    }
}

#[test]
fn classified_token_cache_evicts_least_recently_used_entry() {
    let mut cache = ClassifiedTokenCache::with_limit(2);
    let a = key(1);
    let b = key(2);
    let c = key(3);
    cache.insert(a.clone(), std::sync::Arc::new(classified(1)));
    cache.insert(b.clone(), std::sync::Arc::new(classified(2)));
    assert!(cache.lookup(&a).is_some());
    cache.insert(c.clone(), std::sync::Arc::new(classified(3)));
    assert!(cache.contains_key(&a));
    assert!(!cache.contains_key(&b));
    assert!(cache.contains_key(&c));
    assert_eq!(cache.len(), 2);
}

#[test]
fn classified_token_cache_evicts_to_byte_budget() {
    let a = key(11);
    let b = key(12);
    let c = key(13);
    let a_value = Arc::new(classified_with_source(11, 16));
    let b_value = Arc::new(classified_with_source(12, 16));
    let c_value = Arc::new(classified_with_source(13, 96));
    let budget =
        crate::parsing::c::preprocess::gpu_pipeline::classified_size::classified_tokens_bytes(
            &a_value,
        )
        .checked_add(
            crate::parsing::c::preprocess::gpu_pipeline::classified_size::classified_tokens_bytes(
                &c_value,
            ),
        )
        .expect("Fix: classified cache test budget must fit usize");
    let mut cache = ClassifiedTokenCache::with_limits(8, budget);

    cache.insert(a.clone(), a_value);
    cache.insert(b.clone(), b_value);
    assert!(cache.lookup(&a).is_some());
    cache.insert(c.clone(), c_value);

    assert!(cache.contains_key(&a));
    assert!(!cache.contains_key(&b));
    assert!(cache.contains_key(&c));
    assert!(cache.byte_len() <= budget);
}

#[test]
fn classified_token_cache_lru_index_stays_capacity_scale() {
    let mut cache = ClassifiedTokenCache::with_limit(4);

    for id in 0..96u8 {
        let cache_key = key(id);
        cache.insert(cache_key.clone(), Arc::new(classified(id)));
        assert!(cache.lookup(&cache_key).is_some());
    }

    assert_eq!(cache.len(), 4);
    assert!(
        cache.lru_index_len() <= cache.len().saturating_mul(4).max(8),
        "Fix: classified token cache LRU index must compact stale touches to cache-capacity scale"
    );
}

fn payload_key(id: u8) -> PayloadsCacheKey {
    PayloadsCacheKey {
        path: std::path::PathBuf::from(format!("/tmp/vyre-payload-cache-{id}.h")),
        source_len: id as usize,
        source_hash: [id; 16],
        macro_fingerprint: [id.wrapping_add(1); 16],
    }
}

fn payloads(id: u8, body_len: usize) -> Arc<[DirectivePayload]> {
    Arc::from(
        vec![DirectivePayload::Define {
            name: vec![id],
            name_start: 0,
            name_len: 1,
            args: Vec::new(),
            args_start: 0,
            args_len: 0,
            body: vec![id; body_len],
            body_start: 1,
            body_len: body_len as u32,
            is_function_like: false,
        }]
        .into_boxed_slice(),
    )
}

#[test]
fn payload_cache_evicts_to_byte_budget() {
    let a = payload_key(21);
    let b = payload_key(22);
    let c = payload_key(23);
    let a_value = payloads(21, 16);
    let b_value = payloads(22, 16);
    let c_value = payloads(23, 96);
    let budget = directive_payloads_bytes(&a_value)
        .checked_add(directive_payloads_bytes(&c_value))
        .expect("Fix: payload cache test budget must fit usize");
    let mut cache = PayloadCache::with_limits(8, budget);

    cache.insert(a.clone(), a_value);
    cache.insert(b.clone(), b_value);
    assert!(cache.lookup(&a).is_some());
    cache.insert(c.clone(), c_value);

    assert!(cache.contains_key(&a));
    assert!(!cache.contains_key(&b));
    assert!(cache.contains_key(&c));
    assert!(cache.byte_len() <= budget);
}

#[test]
fn payload_cache_lru_index_stays_capacity_scale() {
    let mut cache = PayloadCache::with_limit(4);

    for id in 0..96u8 {
        let cache_key = payload_key(id);
        cache.insert(cache_key.clone(), payloads(id, 1));
        assert!(cache.lookup(&cache_key).is_some());
    }

    assert_eq!(cache.len(), 4);
    assert!(
        cache.lru_index_len() <= cache.len().saturating_mul(4).max(8),
        "Fix: payload cache LRU index must compact stale touches to cache-capacity scale"
    );
}
