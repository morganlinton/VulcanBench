use crate::allocation::{reserve_hash_map_to_capacity, reserve_smallvec_to_capacity};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use vyre_driver::BackendError;

pub(crate) struct BindingLookup {
    entries: SmallVec<[(u32, usize); 16]>,
    index: Option<FxHashMap<u32, usize>>,
}

impl BindingLookup {
    const INLINE_ENTRIES: usize = 16;

    pub(crate) fn new() -> Self {
        Self {
            entries: SmallVec::new(),
            index: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        if let Some(index) = self.index.as_mut() {
            index.clear();
        }
    }

    pub(crate) fn push(&mut self, binding: u32, value: usize) -> Result<(), BackendError> {
        let next_len = self.entries.len().checked_add(1).ok_or_else(|| {
            BackendError::new(
                "record-and-readback binding lookup length overflowed usize. Fix: split the bind-group binding set before dispatch.",
            )
        })?;
        reserve_smallvec_to_capacity(
            &mut self.entries,
            next_len,
            "record-and-readback binding lookup",
            "inline binding entry",
            "split the bind-group binding set before dispatch",
        )?;
        self.entries.push((binding, value));
        if next_len > Self::INLINE_ENTRIES {
            if let Some(index) = self.index.as_mut() {
                reserve_hash_map_to_capacity(
                    index,
                    next_len,
                    "record-and-readback binding lookup",
                    "binding index entry",
                    "split the bind-group binding set before dispatch",
                )?;
                index.insert(binding, value);
            } else {
                let mut index = FxHashMap::default();
                reserve_hash_map_to_capacity(
                    &mut index,
                    next_len,
                    "record-and-readback binding lookup",
                    "binding index entry",
                    "split the bind-group binding set before dispatch",
                )?;
                for (existing_binding, existing_value) in self.entries.iter().copied() {
                    index.insert(existing_binding, existing_value);
                }
                self.index = Some(index);
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, binding: u32) -> Option<usize> {
        if self.entries.len() > Self::INLINE_ENTRIES {
            let index = self.index.as_ref()?;
            return index.get(&binding).copied();
        }
        self.entries
            .iter()
            .find_map(|(candidate, value)| (*candidate == binding).then_some(*value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_inline_at_inline_capacity() {
        let mut lookup = BindingLookup::new();
        for binding in 0..BindingLookup::INLINE_ENTRIES as u32 {
            lookup
                .push(binding, binding as usize)
                .expect("Fix: inline binding lookup push must reserve.");
        }

        assert!(lookup.index.is_none());
        assert_eq!(lookup.get(7), Some(7));
    }

    #[test]
    fn reuses_hash_capacity_only_after_inline_capacity() {
        let mut lookup = BindingLookup::new();
        for binding in 0..(BindingLookup::INLINE_ENTRIES as u32 + 1) {
            lookup
                .push(binding, binding as usize)
                .expect("Fix: indexed binding lookup push must reserve.");
        }
        assert!(lookup.index.is_some());
        assert_eq!(lookup.get(16), Some(16));

        lookup.clear();
        lookup
            .push(99, 7)
            .expect("Fix: reused binding lookup push must reserve.");

        assert!(
            lookup.index.as_ref().is_some_and(|index| index.is_empty()),
            "Fix: clear must retain the allocated hash table but not force small lookups through it."
        );
        assert_eq!(lookup.get(99), Some(7));
    }
}
