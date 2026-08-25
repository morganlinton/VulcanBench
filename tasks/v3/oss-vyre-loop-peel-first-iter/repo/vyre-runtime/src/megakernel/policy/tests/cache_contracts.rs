use super::*;

#[test]
fn launch_cache_update_does_not_duplicate_entries() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest::direct(128, 64, 256);
    let key = LaunchRecommendationCacheKey { policy, request };
    let rec = policy
        .recommend(request)
        .expect("Fix: policy should accept non-zero adapter limits");
    let mut cache = LaunchRecommendationCache::default();

    cache.insert(key, rec);
    cache.insert(key, rec);

    assert_eq!(cache.entries.len(), 1);
}

#[test]

fn launch_cache_get_promotes_hot_key_before_eviction() {
    let policy = MegakernelLaunchPolicy::standard();
    let hot_request = MegakernelLaunchRequest::direct(1, 64, 256);
    let hot_key = LaunchRecommendationCacheKey {
        policy,
        request: hot_request,
    };
    let hot_rec = policy
        .recommend(hot_request)
        .expect("Fix: policy should accept non-zero adapter limits");
    let mut cache = LaunchRecommendationCache::default();

    cache.insert(hot_key, hot_rec);
    for queue_len in 2..=128 {
        let request = MegakernelLaunchRequest::direct(queue_len, 64, 256);
        let rec = policy
            .recommend(request)
            .expect("Fix: policy should accept non-zero adapter limits");
        cache.insert(LaunchRecommendationCacheKey { policy, request }, rec);
    }
    assert!(cache.get(&hot_key).is_some());
    assert_eq!(cache.hits, 1);
    assert_eq!(cache.misses, 0);

    let cold_request = MegakernelLaunchRequest::direct(129, 64, 256);
    let cold_rec = policy
        .recommend(cold_request)
        .expect("Fix: policy should accept non-zero adapter limits");
    cache.insert(
        LaunchRecommendationCacheKey {
            policy,
            request: cold_request,
        },
        cold_rec,
    );

    assert!(cache.get(&hot_key).is_some());
    assert_eq!(cache.hits, 2);
    assert_eq!(cache.entries.len(), 128);
}

#[test]
fn launch_cache_records_misses_without_mutating_capacity() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest::direct(128, 64, 256);
    let missing = LaunchRecommendationCacheKey { policy, request };
    let mut cache = LaunchRecommendationCache::default();

    assert!(cache.get(&missing).is_none());

    assert_eq!(cache.hits, 0);
    assert_eq!(cache.misses, 1);
    assert!(cache.entries.is_empty());
}

#[test]
fn launch_policy_exposes_thread_local_cache_stats() {
    MegakernelLaunchPolicy::reset_launch_cache_for_thread();
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest::direct(512, 64, 256);

    let initial = MegakernelLaunchPolicy::launch_cache_stats();
    assert_eq!(initial.entries, 0);
    assert_eq!(initial.hits, 0);
    assert_eq!(initial.misses, 0);

    let first = policy
        .recommend(request)
        .expect("Fix: valid policy request must recommend");
    let after_miss = MegakernelLaunchPolicy::launch_cache_stats();
    assert_eq!(after_miss.entries, 1);
    assert_eq!(after_miss.hits, 0);
    assert_eq!(after_miss.misses, 1);

    let second = policy
        .recommend(request)
        .expect("Fix: cached policy request must recommend");
    let after_hit = MegakernelLaunchPolicy::launch_cache_stats();
    assert_eq!(first, second);
    assert_eq!(after_hit.entries, 1);
    assert_eq!(after_hit.hits, 1);
    assert_eq!(after_hit.misses, 1);

    MegakernelLaunchPolicy::reset_launch_cache_for_thread();
}
