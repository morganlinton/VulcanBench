pub(crate) fn checked_batched_frontier_words(words: u32, query_count: u32) -> Result<u32, String> {
    words.checked_mul(query_count).ok_or_else(|| {
        format!(
            "Fix: batched CSR frontier words overflow u32: words={words}, query_count={query_count}."
        )
    })
}
