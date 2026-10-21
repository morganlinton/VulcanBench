pub(super) fn global_typedef_hash_count(
    global_typedef_hashes: Option<&[u8]>,
) -> Result<u32, String> {
    let Some(bytes) = global_typedef_hashes else {
        return Ok(0);
    };
    if bytes.is_empty() {
        return Err(
            "global typedef fast path received an empty hash table. Fix: pass None when no global typedef hashes are available."
                .to_string(),
        );
    }
    if bytes.len() % 4 != 0 {
        return Err(format!(
            "global typedef hash table has {} bytes, not a whole number of u32 hashes. Fix: pass vec_u32_le_bytes output.",
            bytes.len()
        ));
    }
    u32::try_from(bytes.len() / 4).map_err(|_| {
        format!(
            "global typedef hash table has {} hashes, exceeding u32 dispatch capacity. Fix: split the translation unit or shrink typedef prehash input.",
            bytes.len() / 4
        )
    })
}

#[cfg(test)]
mod tests {
    use super::global_typedef_hash_count;

    #[test]
    fn global_typedef_hash_count_rejects_empty_present_table() {
        let err = global_typedef_hash_count(Some(&[]))
            .expect_err("empty present table must not be coerced to one hash");
        assert!(
            err.contains("pass None"),
            "error must tell callers to use None for absent global typedef hashes"
        );
    }

    #[test]
    fn global_typedef_hash_count_rejects_misaligned_bytes() {
        let err = global_typedef_hash_count(Some(&[1, 2, 3]))
            .expect_err("misaligned hash bytes must fail loudly");
        assert!(
            err.contains("whole number of u32 hashes"),
            "error must explain the u32 hash-table byte contract"
        );
    }

    #[test]
    fn global_typedef_hash_count_accepts_valid_hash_bytes() {
        assert_eq!(global_typedef_hash_count(None).unwrap(), 0);
        assert_eq!(
            global_typedef_hash_count(Some(&[1, 2, 3, 4, 5, 6, 7, 8])).unwrap(),
            2
        );
    }
}
