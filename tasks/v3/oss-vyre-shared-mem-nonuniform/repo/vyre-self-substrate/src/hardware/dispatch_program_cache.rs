/// Two-slot primitive dispatch program cache.
///
/// Self-substrate wrappers specialize primitive `Program`s for launch shape,
/// backend feature key, or layout. Rebuilding those programs in hot loops adds
/// host allocation and descriptor work to paths that should be dominated by GPU
/// execution. This cache keeps the common single-shape fast path cheap and the
/// common two-shape alternating path resident without heap allocation across
/// graph, math, optimizer, and data wrappers.
#[derive(Debug)]
pub(crate) struct ProgramCache<K, V> {
    hot: Option<ProgramCacheEntry<K, V>>,
    warm: Option<ProgramCacheEntry<K, V>>,
    #[cfg(test)]
    builds: usize,
}

#[derive(Debug)]
struct ProgramCacheEntry<K, V> {
    key: K,
    value: V,
}

impl<K, V> Default for ProgramCache<K, V> {
    fn default() -> Self {
        Self {
            hot: None,
            warm: None,
            #[cfg(test)]
            builds: 0,
        }
    }
}

impl<K: Eq, V> ProgramCache<K, V> {
    pub(crate) fn get_or_insert_with(&mut self, key: K, build: impl FnOnce() -> V) -> &V {
        if self.hot_matches(&key) {
            return self.hot_value();
        }
        if self.warm_matches(&key) {
            self.promote_warm();
            return self.hot_value();
        }

        self.insert_hot(key, build());
        self.hot_value()
    }

    pub(crate) fn get_or_try_insert_with<E>(
        &mut self,
        key: K,
        build: impl FnOnce() -> Result<V, E>,
    ) -> Result<&V, E> {
        if self.hot_matches(&key) {
            return Ok(self.hot_value());
        }
        if self.warm_matches(&key) {
            self.promote_warm();
            return Ok(self.hot_value());
        }

        let value = build()?;
        self.insert_hot(key, value);
        Ok(self.hot_value())
    }

    fn hot_matches(&self, key: &K) -> bool {
        self.hot.as_ref().is_some_and(|entry| entry.key == *key)
    }

    fn warm_matches(&self, key: &K) -> bool {
        self.warm.as_ref().is_some_and(|entry| entry.key == *key)
    }

    fn promote_warm(&mut self) {
        core::mem::swap(&mut self.hot, &mut self.warm);
    }

    fn insert_hot(&mut self, key: K, value: V) {
        self.warm = self.hot.take();
        self.hot = Some(ProgramCacheEntry { key, value });
        #[cfg(test)]
        {
            self.builds += 1;
        }
    }

    fn hot_value(&self) -> &V {
        match self.hot.as_ref() {
            Some(entry) => &entry.value,
            None => {
                unreachable!("Fix: dispatch program cache must contain a hot entry after insertion")
            }
        }
    }

    #[cfg(test)]
    pub(crate) const fn builds(&self) -> usize {
        self.builds
    }
}

#[cfg(test)]
mod tests {
    use super::ProgramCache;

    #[test]
    fn alternating_two_keys_do_not_rebuild() {
        let mut cache = ProgramCache::<u32, u32>::default();

        assert_eq!(*cache.get_or_insert_with(1, || 10), 10);
        assert_eq!(*cache.get_or_insert_with(2, || 20), 20);
        assert_eq!(*cache.get_or_insert_with(1, || 99), 10);
        assert_eq!(*cache.get_or_insert_with(2, || 99), 20);

        assert_eq!(cache.builds(), 2);
    }

    #[test]
    fn third_key_evicts_only_the_cold_slot() {
        let mut cache = ProgramCache::<u32, u32>::default();

        assert_eq!(*cache.get_or_insert_with(1, || 10), 10);
        assert_eq!(*cache.get_or_insert_with(2, || 20), 20);
        assert_eq!(*cache.get_or_insert_with(1, || 99), 10);
        assert_eq!(*cache.get_or_insert_with(3, || 30), 30);
        assert_eq!(*cache.get_or_insert_with(1, || 99), 10);
        assert_eq!(*cache.get_or_insert_with(2, || 22), 22);

        assert_eq!(cache.builds(), 4);
    }

    #[test]
    fn fallible_builder_does_not_poison_existing_entries_on_error() {
        let mut cache = ProgramCache::<u32, u32>::default();

        assert_eq!(
            *cache
                .get_or_try_insert_with::<&'static str>(1, || Ok(10))
                .expect("Fix: cache warm-up must succeed for valid keys; return Err on corrupt cache config - first build succeeds"),
            10
        );
        assert_eq!(
            cache.get_or_try_insert_with(2, || Err("failed build")),
            Err("failed build")
        );
        assert_eq!(
            *cache
                .get_or_try_insert_with::<&'static str>(1, || Ok(99))
                .expect("Fix: failed miss must not evict valid hot entries; return cache miss Err - existing hot entry survives failed miss"),
            10
        );

        assert_eq!(cache.builds(), 1);
    }

    #[test]
    fn generated_access_stream_matches_two_slot_lru_model_for_8192_cases() {
        let mut cache = ProgramCache::<u32, u32>::default();
        let mut hot = None::<(u32, u32)>;
        let mut warm = None::<(u32, u32)>;
        let mut expected_builds = 0usize;

        for index in 0..8192u32 {
            let key = generated_key(index);
            let expected = match (hot, warm) {
                (Some((hot_key, hot_value)), _) if hot_key == key => hot_value,
                (_, Some((warm_key, warm_value))) if warm_key == key => {
                    core::mem::swap(&mut hot, &mut warm);
                    warm_value
                }
                _ => {
                    let value = key.wrapping_mul(17).wrapping_add(3);
                    warm = hot;
                    hot = Some((key, value));
                    expected_builds += 1;
                    value
                }
            };

            let actual = *cache.get_or_insert_with(key, || key.wrapping_mul(17).wrapping_add(3));
            assert_eq!(actual, expected, "index {index} key {key}");
            assert_eq!(cache.builds(), expected_builds, "index {index}");
        }
    }

    fn generated_key(index: u32) -> u32 {
        match index % 11 {
            0 | 1 | 2 | 3 => index % 2,
            4 | 5 | 6 => 2 + (index % 2),
            7 | 8 => index % 4,
            9 => index.wrapping_mul(2_654_435_761).rotate_left(7) % 16,
            _ => index % 3,
        }
    }
}
