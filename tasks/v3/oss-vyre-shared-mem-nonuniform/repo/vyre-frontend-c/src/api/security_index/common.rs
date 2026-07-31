pub(super) fn count_u64(count: usize, label: &str) -> u64 {
    let _ = label;
    u64::try_from(count).unwrap_or(u64::MAX)
}
