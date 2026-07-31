//! Dependency-free SHA-256 for cache keys.

mod sha256;
mod sha256_hex;
#[cfg(test)]
mod tests;

pub(crate) use sha256::sha256;
pub(crate) use sha256_hex::sha256_hex;
