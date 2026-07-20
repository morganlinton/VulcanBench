use super::{
    default_buffers, optimize_megakernel_program, persistent_body_with_io, wrap_megakernel_program,
    wrap_persistent_megakernel_program,
};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::sync::Arc;
use vyre_foundation::ir::Program;

const EMPTY_TEMPLATE_CACHE_CAP: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EmptyTemplateKey {
    workgroup_size_x: u32,
    slot_count: u32,
    include_io_polling: bool,
    finite_once: bool,
    control_report_only: bool,
}

struct EmptyTemplateCache {
    entries: FxHashMap<EmptyTemplateKey, EmptyTemplateEntry>,
    clock: u64,
}

struct EmptyTemplateEntry {
    program: Arc<Program>,
    last_seen: u64,
}

impl EmptyTemplateCache {
    fn get(&mut self, key: &EmptyTemplateKey) -> Option<Arc<Program>> {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.entries.values_mut() {
                entry.last_seen = 0;
            }
        }
        let entry = self.entries.get_mut(key)?;
        self.clock += 1;
        entry.last_seen = self.clock;
        Some(Arc::clone(&entry.program))
    }

    fn insert(&mut self, key: EmptyTemplateKey, program: Arc<Program>) {
        let tick = self.next_tick();
        self.entries.insert(
            key,
            EmptyTemplateEntry {
                program,
                last_seen: tick,
            },
        );
        while self.entries.len() > EMPTY_TEMPLATE_CACHE_CAP {
            let Some(evicted) = self
                .entries
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(candidate, _)| *candidate)
            else {
                break;
            };
            self.entries.remove(&evicted);
        }
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
        self.clock = 0;
    }

    fn next_tick(&mut self) -> u64 {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.entries.values_mut() {
                entry.last_seen = 0;
            }
        }
        self.clock += 1;
        self.clock
    }
}

impl Default for EmptyTemplateCache {
    fn default() -> Self {
        Self {
            entries: FxHashMap::with_capacity_and_hasher(
                EMPTY_TEMPLATE_CACHE_CAP,
                Default::default(),
            ),
            clock: 0,
        }
    }
}

thread_local! {
    static EMPTY_TEMPLATE_CACHE: RefCell<EmptyTemplateCache> =
        RefCell::new(EmptyTemplateCache::default());
}

pub(super) fn cached_empty_sharded_program(
    workgroup_size_x: u32,
    slot_count: u32,
    include_io_polling: bool,
) -> Program {
    cached_empty_sharded_program_shared(workgroup_size_x, slot_count, include_io_polling)
        .as_ref()
        .clone()
}

pub(super) fn cached_empty_sharded_program_shared(
    workgroup_size_x: u32,
    slot_count: u32,
    include_io_polling: bool,
) -> Arc<Program> {
    let key = EmptyTemplateKey {
        workgroup_size_x,
        slot_count,
        include_io_polling,
        finite_once: false,
        control_report_only: false,
    };
    if let Some(program) = EMPTY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().get(&key)) {
        return program;
    }

    let program = wrap_persistent_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_with_io(workgroup_size_x, &[], include_io_polling),
    );
    let program = Arc::new(program);
    EMPTY_TEMPLATE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, Arc::clone(&program));
    });
    program
}

pub(super) fn cached_empty_sharded_once_program(workgroup_size_x: u32, slot_count: u32) -> Program {
    cached_empty_sharded_once_program_shared(workgroup_size_x, slot_count)
        .as_ref()
        .clone()
}

pub(super) fn cached_empty_sharded_once_program_shared(
    workgroup_size_x: u32,
    slot_count: u32,
) -> Arc<Program> {
    let key = EmptyTemplateKey {
        workgroup_size_x,
        slot_count,
        include_io_polling: false,
        finite_once: true,
        control_report_only: false,
    };
    if let Some(program) = EMPTY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().get(&key)) {
        return program;
    }

    let program = wrap_megakernel_program(
        workgroup_size_x,
        slot_count,
        persistent_body_with_io(workgroup_size_x, &[], false),
    );
    let program = Arc::new(program);
    EMPTY_TEMPLATE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, Arc::clone(&program));
    });
    program
}

pub(super) fn cached_empty_sharded_once_control_report_program_shared(
    workgroup_size_x: u32,
    slot_count: u32,
) -> Arc<Program> {
    let key = EmptyTemplateKey {
        workgroup_size_x,
        slot_count,
        include_io_polling: false,
        finite_once: true,
        control_report_only: true,
    };
    if let Some(program) = EMPTY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().get(&key)) {
        return program;
    }

    let mut buffers = default_buffers(slot_count);
    for buffer in buffers.iter_mut().skip(1) {
        buffer.output_byte_range = Some(0..0);
    }
    let program = Arc::new(optimize_megakernel_program(Program::wrapped(
        buffers,
        [workgroup_size_x, 1, 1],
        persistent_body_with_io(workgroup_size_x, &[], false),
    )));
    EMPTY_TEMPLATE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, Arc::clone(&program));
    });
    program
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_template_cache_refreshes_hot_template_on_hit() {
        EMPTY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().clear());
        let hot = cached_empty_sharded_program_shared(1, 1, false);
        for slot_count in 2..=EMPTY_TEMPLATE_CACHE_CAP as u32 {
            let _ = cached_empty_sharded_program_shared(1, slot_count, false);
        }
        let hot_after_hit = cached_empty_sharded_program_shared(1, 1, false);
        assert!(Arc::ptr_eq(&hot, &hot_after_hit));
        let _ =
            cached_empty_sharded_program_shared(1, (EMPTY_TEMPLATE_CACHE_CAP + 1) as u32, false);
        let hot_after_eviction = cached_empty_sharded_program_shared(1, 1, false);
        assert!(Arc::ptr_eq(&hot, &hot_after_eviction));
    }

    #[test]
    fn empty_control_report_template_is_cached_by_arc() {
        EMPTY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().clear());
        let first = cached_empty_sharded_once_control_report_program_shared(64, 128);
        let second = cached_empty_sharded_once_control_report_program_shared(64, 128);

        assert!(Arc::ptr_eq(&first, &second));
    }
}
