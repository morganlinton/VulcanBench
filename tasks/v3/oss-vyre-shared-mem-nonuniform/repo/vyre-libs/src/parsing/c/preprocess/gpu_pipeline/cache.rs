#[path = "cache/classified_codec.rs"]
mod classified_codec;
#[path = "cache/classified_disk.rs"]
mod classified_disk;
#[path = "cache/classified_memory.rs"]
mod classified_memory;
#[path = "cache/disk_common.rs"]
mod disk_common;
#[path = "cache/payload_codec.rs"]
mod payload_codec;
#[path = "cache/payload_disk.rs"]
mod payload_disk;
#[path = "cache/payload_keys.rs"]
mod payload_keys;
#[path = "cache/payload_memory.rs"]
mod payload_memory;
#[cfg(test)]
#[path = "cache/tests/mod.rs"]
mod tests;

pub(super) use classified_disk::{load_classified_from_disk, store_classified_to_disk};
pub(super) use classified_memory::{
    cached_classified_tokens, classified_cache_key_from_hash, insert_classified_tokens,
};
pub(super) use disk_common::source_hash128;
pub(super) use payload_disk::{load_payloads_from_disk, store_payloads_to_disk};
pub(super) use payload_keys::production_payloads_cache_key_from_hash;
pub(super) use payload_memory::{cached_payloads, insert_payloads};

#[cfg(test)]
pub(super) use classified_codec::{decode_classified, encode_classified, DecodeError};
#[cfg(test)]
pub(super) use classified_memory::classified_cache_key;
#[cfg(test)]
pub(super) use classified_memory::ClassifiedCacheKey;
#[cfg(test)]
pub(super) use disk_common::{
    cache_key_stem, read_disk_cache_file_bounded_with_limit, CLASSIFIED_DISK_MAGIC,
};
#[cfg(test)]
pub(super) use payload_codec::{decode_payloads, encode_payloads};
#[cfg(test)]
pub(super) use payload_keys::{
    macro_fingerprint, payloads_cache_key, production_payloads_cache_key, PayloadsCacheKey,
};
#[cfg(test)]
pub(super) use payload_memory::PayloadCache;
