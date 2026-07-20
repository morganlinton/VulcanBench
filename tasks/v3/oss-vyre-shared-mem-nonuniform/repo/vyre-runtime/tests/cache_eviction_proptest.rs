//! Property tests for layered pipeline-cache fallthrough behavior.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use proptest::prelude::*;
use vyre_runtime::pipeline_cache::{LayeredPipelineCache, PipelineCacheStore, PipelineFingerprint};

#[derive(Debug, Default)]
struct TestStore {
    inner: Mutex<HashMap<PipelineFingerprint, Arc<Vec<u8>>>>,
}

impl TestStore {
    fn snapshot(&self) -> HashMap<PipelineFingerprint, Vec<u8>> {
        self.inner
            .lock()
            .expect("Fix: test store mutex must not be poisoned")
            .iter()
            .map(|(key, value)| (*key, (**value).clone()))
            .collect()
    }

    fn direct_put(&self, key: PipelineFingerprint, payload: Vec<u8>) {
        self.inner
            .lock()
            .expect("Fix: test store mutex must not be poisoned")
            .insert(key, Arc::new(payload));
    }
}

impl PipelineCacheStore for TestStore {
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>> {
        self.get_arc(fp).map(|arc| (*arc).clone())
    }

    fn get_arc(&self, fp: &PipelineFingerprint) -> Option<Arc<Vec<u8>>> {
        self.inner
            .lock()
            .expect("Fix: test store mutex must not be poisoned")
            .get(fp)
            .cloned()
    }

    fn put(&self, fp: PipelineFingerprint, artifact: Vec<u8>) {
        self.direct_put(fp, artifact);
    }
}

#[derive(Debug, Clone)]
enum Op {
    SeedLower {
        layer: usize,
        key: [u8; 32],
        payload: Vec<u8>,
    },
    PutTop {
        key: [u8; 32],
        payload: Vec<u8>,
    },
    Get {
        key: [u8; 32],
    },
}

fn fingerprint(bytes: [u8; 32]) -> PipelineFingerprint {
    PipelineFingerprint(bytes)
}

fn payload_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..33)
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (1usize..3, any::<[u8; 32]>(), payload_strategy()).prop_map(|(layer, key, payload)| {
            Op::SeedLower {
                layer,
                key,
                payload,
            }
        }),
        (any::<[u8; 32]>(), payload_strategy())
            .prop_map(|(key, payload)| Op::PutTop { key, payload }),
        any::<[u8; 32]>().prop_map(|key| Op::Get { key }),
    ]
}

fn first_hit(
    model: &[HashMap<PipelineFingerprint, Vec<u8>>],
    key: &PipelineFingerprint,
) -> Option<Vec<u8>> {
    model.iter().find_map(|layer| layer.get(key).cloned())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        ..ProptestConfig::default()
    })]

    #[test]
    fn layered_cache_preserves_fallthrough_invariants(ops in prop::collection::vec(op_strategy(), 1..256)) {
        let layers = [Arc::new(TestStore::default()),
            Arc::new(TestStore::default()),
            Arc::new(TestStore::default())];
        let cache = LayeredPipelineCache::new(vec![layers[0].clone(), layers[1].clone(), layers[2].clone()]);
        let mut model = vec![HashMap::new(), HashMap::new(), HashMap::new()];

        for op in ops {
            let op_debug = format!("{op:?}");
            match op {
                Op::SeedLower { layer, key, payload } => {
                    let key = fingerprint(key);
                    layers[layer].direct_put(key, payload.clone());
                    model[layer].insert(key, payload);
                }
                Op::PutTop { key, payload } => {
                    let key = fingerprint(key);
                    cache.put(key, payload.clone());
                    model[0].insert(key, payload);
                }
                Op::Get { key } => {
                    let key = fingerprint(key);
                    let expected = first_hit(&model, &key);
                    let actual = cache.get_arc(&key).map(|bytes| (*bytes).clone());
                    prop_assert_eq!(actual, expected, "Fix: layered cache must return the first hit across its layers");
                }
            }

            for (index, store) in layers.iter().enumerate() {
                prop_assert_eq!(
                    store.snapshot(),
                    model[index].clone(),
                    "Fix: cache layer {} diverged from the modeled state after {:?}",
                    index,
                    op_debug
                );
            }
        }
    }
}
