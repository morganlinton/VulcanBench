//! Buffer storage for the HashMap interpreter.
//!
//! Storage buffers persist across workgroups; workgroup buffers are rebuilt for
//! each workgroup dispatch. The helpers here centralize that distinction so the
//! executor does not duplicate storage/workgroup lookup plumbing.

use crate::{oob::Buffer, value::Value, workgroup::MAX_WORKGROUP_BYTES};
use rustc_hash::FxHashMap;
use vyre::ir::{BufferAccess, BufferDecl, Program};
use vyre::Error;

pub(crate) struct HashmapMemory {
    pub(crate) storage: FxHashMap<String, Buffer>,
    pub(crate) workgroup: FxHashMap<String, Buffer>,
}

impl HashmapMemory {
    pub(crate) fn new(storage: FxHashMap<String, Buffer>) -> Self {
        Self {
            storage,
            workgroup: FxHashMap::default(),
        }
    }

    pub(crate) fn reset_workgroup(&mut self, program: &Program) -> Result<(), Error> {
        if zero_existing_workgroup(&self.workgroup, program)? {
            return Ok(());
        }
        self.workgroup = workgroup_memory(program)?;
        Ok(())
    }
}

pub(crate) fn output_value(buffer: Buffer, decl: &BufferDecl) -> Value {
    let mut bytes = buffer.into_bytes();
    if let Some(range) = decl.output_byte_range() {
        if range.start <= range.end && range.end <= bytes.len() {
            bytes.truncate(range.end);
            bytes.drain(..range.start);
        }
    }
    Value::from(bytes)
}

pub(crate) fn workgroup_memory(program: &Program) -> Result<FxHashMap<String, Buffer>, Error> {
    let mut workgroup = FxHashMap::default();
    let mut allocated = 0usize;
    for decl in program
        .buffers()
        .iter()
        .filter(|decl| decl.access() == BufferAccess::Workgroup)
    {
        let len = workgroup_byte_len(decl)?;
        allocated = allocated . checked_add (len) . ok_or_else (| | { Error :: interp ("total workgroup memory byte size overflows usize. Fix: reduce workgroup buffer declarations." ,) }) ? ;
        if allocated > MAX_WORKGROUP_BYTES {
            return Err(Error::interp(format!(
                "workgroup memory requires {allocated} bytes, exceeding the {MAX_WORKGROUP_BYTES}-byte reference budget. Fix: reduce workgroup buffer counts."
            )));
        }
        workgroup.insert(
            decl.name().to_string(),
            Buffer::new(vec![0; len], decl.element().clone()),
        );
    }
    Ok(workgroup)
}

fn zero_existing_workgroup(
    workgroup: &FxHashMap<String, Buffer>,
    program: &Program,
) -> Result<bool, Error> {
    let mut decl_count = 0usize;
    for decl in program
        .buffers()
        .iter()
        .filter(|decl| decl.access() == BufferAccess::Workgroup)
    {
        decl_count += 1;
        let Some(buffer) = workgroup.get(decl.name()) else {
            return Ok(false);
        };
        let len = workgroup_byte_len(decl)?;
        if buffer.element() != &decl.element() || buffer.byte_len() != len {
            return Ok(false);
        }
    }
    if workgroup.len() != decl_count {
        return Ok(false);
    }
    for buffer in workgroup.values() {
        buffer.zero_fill();
    }
    Ok(true)
}

fn workgroup_byte_len(decl: &BufferDecl) -> Result<usize, Error> {
    match decl.static_byte_len() {
        Ok(Some(byte_len)) => Ok(byte_len),
        Ok(None) if decl.count() == 0 => Ok(0),
        Ok(None) => Err(Error::interp(format!(
            "workgroup buffer `{}` has unsized element type {}. Fix: use a fixed-width workgroup element type.",
            decl.name(),
            decl.element()
        ))),
        Err(error) => Err(Error::interp(error)),
    }
}

pub(crate) fn resolve_buffer<'a>(
    memory: &'a HashmapMemory,
    name: &str,
) -> Result<&'a Buffer, Error> {
    memory
        .storage
        .get(name)
        .or_else(|| memory.workgroup.get(name))
        .ok_or_else(|| {
            Error::interp(format!(
                "missing buffer `{name}`. Fix: initialize all declared buffers."
            ))
        })
}

pub(crate) fn buffer_mut<'a>(
    memory: &'a mut HashmapMemory,
    name: &str,
) -> Result<&'a mut Buffer, Error> {
    memory
        .storage
        .get_mut(name)
        .or_else(|| memory.workgroup.get_mut(name))
        .ok_or_else(|| {
            Error::interp(format!(
                "missing buffer `{name}`. Fix: initialize all declared buffers."
            ))
        })
}

pub(crate) fn atomic_buffer_mut<'a>(
    memory: &'a mut HashmapMemory,
    name: &str,
) -> Result<&'a mut Buffer, Error> {
    memory . storage . get_mut (name) . ok_or_else (| | { Error :: interp (format ! ("atomic target `{name}` is workgroup memory or missing. Fix: atomics only support ReadWrite storage buffers.")) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oob;
    use crate::value::Value;
    use vyre::ir::{DataType, Node};

    fn workgroup_program(count: u32) -> Program {
        Program::wrapped(
            vec![BufferDecl::workgroup("scratch", count, DataType::U32)],
            [64, 1, 1],
            Vec::<Node>::new(),
        )
    }

    #[test]
    fn reset_workgroup_reuses_matching_buffers_and_zeroes_in_place() {
        let program = workgroup_program(4);
        let mut memory = HashmapMemory::new(FxHashMap::default());
        memory
            .reset_workgroup(&program)
            .expect("Fix: first workgroup allocation must succeed.");
        let before = memory
            .workgroup
            .get("scratch")
            .expect("Fix: scratch must be allocated.")
            .bytes
            .clone();
        oob::store(
            memory.workgroup.get_mut("scratch").unwrap(),
            0,
            &Value::U32(0xfeed_beef),
        );

        memory
            .reset_workgroup(&program)
            .expect("Fix: matching reset must reuse and zero the workgroup buffer.");
        let after = memory.workgroup.get("scratch").unwrap().bytes.clone();
        assert!(
            std::sync::Arc::ptr_eq(&before, &after),
            "Fix: matching workgroup layout must not allocate a replacement buffer."
        );
        assert_eq!(
            oob::load(memory.workgroup.get("scratch").unwrap(), 0),
            Value::U32(0),
            "Fix: reused workgroup buffers must be zero-filled before the next workgroup."
        );
    }

    #[test]
    fn reset_workgroup_reallocates_when_layout_changes() {
        let mut memory = HashmapMemory::new(FxHashMap::default());
        memory.reset_workgroup(&workgroup_program(4)).unwrap();
        let before = memory.workgroup.get("scratch").unwrap().bytes.clone();
        memory.reset_workgroup(&workgroup_program(8)).unwrap();
        let after = memory.workgroup.get("scratch").unwrap().bytes.clone();
        assert!(
            !std::sync::Arc::ptr_eq(&before, &after),
            "Fix: changed workgroup byte length must allocate a correctly-sized buffer."
        );
    }

    #[test]
    fn workgroup_memory_uses_packed_static_byte_len_for_i4() {
        let program = Program::wrapped(
            vec![BufferDecl::workgroup("scratch", 3, DataType::I4)],
            [1, 1, 1],
            Vec::<Node>::new(),
        );
        let memory =
            workgroup_memory(&program).expect("Fix: packed I4 workgroup allocation must succeed.");

        assert_eq!(
            memory.get("scratch").expect("Fix: scratch must exist.").byte_len(),
            2,
            "Fix: three I4 workgroup elements must allocate two packed bytes, not three one-byte lanes."
        );
    }

    #[test]
    fn output_value_slices_declared_byte_range_from_buffer_bytes() {
        let decl = BufferDecl::output("out", 0, DataType::U32)
            .with_count(4)
            .with_output_byte_range(4..12);
        let buffer = Buffer::new((0u8..16).collect(), DataType::U32);

        assert_eq!(
            output_value(buffer, &decl).to_bytes(),
            vec![4, 5, 6, 7, 8, 9, 10, 11],
            "Fix: output byte ranges must slice the buffer payload without changing bytes."
        );
    }
}
