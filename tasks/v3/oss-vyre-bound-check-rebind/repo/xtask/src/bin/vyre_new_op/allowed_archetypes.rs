#![allow(missing_docs)]
pub(crate) const ALLOWED_ARCHETYPES: &[&str] = &[
    "binary-bitwise",
    "unary-bitwise",
    "binary-arithmetic",
    "unary-arithmetic",
    "binary-comparison",
    "binary-logical",
    "unary-logical",
    "hash-bytes-to-u32",
    "hash-bytes-to-u64",
    "decode-bytes-to-bytes",
    "compression-bytes-to-bytes",
    "match-bytes-pattern",
    "graph-reachability",
    "tokenize-bytes",
    "rule-bytes-to-bool",
];
