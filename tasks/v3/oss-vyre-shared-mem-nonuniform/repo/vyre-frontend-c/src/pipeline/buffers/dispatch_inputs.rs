use super::*;

/// Borrowed-input variant of [`pad_dispatch_inputs`].
///
/// The returned references are valid as long as `supplied` and `padding` stay
/// alive. This avoids cloning large resident token buffers just to satisfy
/// strict CUDA input-count validation.
pub(crate) fn pad_dispatch_input_refs<'a>(
    program: &vyre_foundation::ir::Program,
    supplied: Vec<&'a [u8]>,
    padding: &'a mut Vec<Vec<u8>>,
) -> Vec<&'a [u8]> {
    let supplied_len = supplied.len();
    let input_count = program
        .buffers
        .iter()
        .filter(|buffer| is_input_buffer(buffer))
        .count();
    assert!(
        supplied_len <= input_count,
        "pad_dispatch_input_refs received {supplied_len} supplied inputs for a program with {input_count} input buffers. Fix: remove output buffers from the dispatch input list."
    );
    let missing = input_count - supplied_len;
    if padding.len() < missing {
        padding.resize_with(missing, Vec::new);
    } else {
        padding.truncate(missing);
    }
    for (slot, buf) in padding.iter_mut().zip(
        program
            .buffers
            .iter()
            .filter(|buffer| is_input_buffer(buffer))
            .skip(supplied_len),
    ) {
        slot.clear();
        slot.resize(missing_input_pad_bytes(buf), 0);
    }
    let mut refs = supplied;
    refs.extend(padding.iter().map(Vec::as_slice));
    refs
}

#[inline]
fn missing_input_pad_bytes(buffer: &vyre_foundation::ir::BufferDecl) -> usize {
    if buffer.count == 0 {
        return 4;
    }
    usize::try_from(buffer.count)
        .unwrap_or(usize::MAX)
        .saturating_mul(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferDecl, DataType, Program};

    fn input(name: &str, binding: u32, count: u32) -> BufferDecl {
        BufferDecl::read(name, binding, DataType::U32).with_count(count)
    }

    #[test]
    fn pad_dispatch_input_refs_reuses_existing_padding_slots() {
        let program = Program::wrapped(
            vec![
                input("present", 0, 1),
                input("pad_a", 1, 2),
                input("pad_b", 2, 1),
            ],
            [1, 1, 1],
            Vec::new(),
        );
        let supplied = [1u8, 2, 3, 4];
        let mut padding = vec![Vec::with_capacity(8), Vec::with_capacity(4)];
        let first_ptr = padding[0].as_ptr();
        let second_ptr = padding[1].as_ptr();

        let refs = pad_dispatch_input_refs(&program, vec![&supplied], &mut padding);

        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], supplied);
        assert_eq!(refs[1], &[0u8; 8]);
        assert_eq!(refs[2], &[0u8; 4]);
        assert_eq!(padding[0].as_ptr(), first_ptr);
        assert_eq!(padding[1].as_ptr(), second_ptr);
    }
}
