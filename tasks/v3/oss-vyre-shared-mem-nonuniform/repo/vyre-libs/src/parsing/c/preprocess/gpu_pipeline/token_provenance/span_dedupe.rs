use super::*;
use std::hash::Hash;

const INLINE_SPANS: usize = 8;

pub(crate) struct SpanDedupe<T>
where
    T: Copy + Eq + Hash,
{
    inline: SmallVec<[(T, T); 8]>,
    overflow: Option<HashSet<(T, T)>>,
}

impl<T> SpanDedupe<T>
where
    T: Copy + Eq + Hash,
{
    pub(crate) fn try_from_iter(spans: impl IntoIterator<Item = (T, T)>) -> Result<Self, String> {
        let mut dedupe = Self {
            inline: SmallVec::new(),
            overflow: None,
        };
        for span in spans {
            dedupe.insert(span)?;
        }
        Ok(dedupe)
    }

    pub(crate) fn insert(&mut self, span: (T, T)) -> Result<bool, String> {
        if let Some(overflow) = &mut self.overflow {
            return Ok(overflow.insert(span));
        }
        if self.inline.contains(&span) {
            return Ok(false);
        }
        if self.inline.len() < INLINE_SPANS {
            self.inline.push(span);
            return Ok(true);
        }
        let mut overflow = HashSet::default();
        let reserve_slots = self.inline.len().checked_mul(2).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: token provenance span dedupe reserve size overflowed. Fix: shard preprocessing before provenance export.".to_string()
        })?;
        overflow.try_reserve(reserve_slots).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {reserve_slots} token provenance span dedupe slots: {error:?}. Fix: shard preprocessing before provenance export."
            )
        })?;
        overflow.extend(self.inline.iter().copied());
        let inserted = overflow.insert(span);
        self.overflow = Some(overflow);
        Ok(inserted)
    }
}
