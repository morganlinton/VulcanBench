//! HTTPS-backed read-through cache. Feature-gated on `remote-cache` so library
//! users who only want disk caching don't pull in `ureq`.

use super::disk::read_verified_cache_blob;
use super::fingerprint::PipelineFingerprint;
use super::store::PipelineCacheStore;

const HEADER_FINGERPRINT: &str = "x-vyre-cache-fingerprint";
const HEADER_SOURCE_PROVENANCE: &str = "x-vyre-cache-source-provenance";
const HEADER_DEVICE_COMPATIBILITY: &str = "x-vyre-cache-device-compatibility";

/// HTTPS-backed cache that reads pre-compiled artifacts from a
/// base URL. Feature-gated on `remote-cache` so library users who only
/// want disk caching don't pull in `ureq`.
///
/// Writes are **no-ops**  -  `RemoteCache` is a read-through layer.
/// Publishing to a remote registry is a separate `vyre publish-cache`
/// xtask, not part of this runtime.
pub struct RemoteCache {
    base_url: String,
    agent: ureq::Agent,
    expected_source_provenance: Option<String>,
    expected_device_compatibility: Option<String>,
}

impl RemoteCache {
    /// Construct from a base URL. The cache fetches
    /// `<base_url>/<fp_hex>.bin` for each lookup.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            agent: ureq::Agent::new_with_defaults(),
            expected_source_provenance: None,
            expected_device_compatibility: None,
        }
    }

    /// Require exact source provenance and device compatibility metadata on
    /// every remote hit.
    #[must_use]
    pub fn with_required_metadata(
        mut self,
        source_provenance: impl Into<String>,
        device_compatibility: impl Into<String>,
    ) -> Self {
        self.expected_source_provenance = Some(source_provenance.into());
        self.expected_device_compatibility = Some(device_compatibility.into());
        self
    }

    fn metadata_expectation(&self) -> RemoteMetadataExpectation<'_> {
        RemoteMetadataExpectation {
            expected_source_provenance: self.expected_source_provenance.as_deref(),
            expected_device_compatibility: self.expected_device_compatibility.as_deref(),
        }
    }
}

impl PipelineCacheStore for RemoteCache {
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>> {
        let url = format!("{}/{}.bin", self.base_url.trim_end_matches('/'), fp.hex());
        let mut resp = self.agent.get(&url).call().ok()?;
        if !remote_metadata_allows(
            fp,
            header_value(&resp, HEADER_FINGERPRINT),
            header_value(&resp, HEADER_SOURCE_PROVENANCE),
            header_value(&resp, HEADER_DEVICE_COMPATIBILITY),
            self.metadata_expectation(),
        ) {
            return None;
        }
        read_verified_cache_blob(resp.body_mut().as_reader())
    }

    fn put(&self, _fp: PipelineFingerprint, _artifact: Vec<u8>) {
        // Remote cache is read-through; publishing is a separate flow.
    }
}

fn header_value<'a>(resp: &'a ureq::http::Response<ureq::Body>, name: &str) -> Option<&'a str> {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

#[derive(Debug, Clone, Copy)]
struct RemoteMetadataExpectation<'a> {
    expected_source_provenance: Option<&'a str>,
    expected_device_compatibility: Option<&'a str>,
}

fn remote_metadata_allows(
    fp: &PipelineFingerprint,
    fingerprint: Option<&str>,
    source_provenance: Option<&str>,
    device_compatibility: Option<&str>,
    expectation: RemoteMetadataExpectation<'_>,
) -> bool {
    let expected_fingerprint = fp.hex();
    let Some(fingerprint) = fingerprint else {
        return false;
    };
    if fingerprint != expected_fingerprint {
        return false;
    }
    let Some(source_provenance) = source_provenance.filter(|value| !value.is_empty()) else {
        return false;
    };
    let Some(device_compatibility) = device_compatibility.filter(|value| !value.is_empty()) else {
        return false;
    };
    if let Some(expected) = expectation.expected_source_provenance {
        if source_provenance != expected {
            return false;
        }
    }
    if let Some(expected) = expectation.expected_device_compatibility {
        if device_compatibility != expected {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_cache::test_helpers::tiny_program;

    #[test]
    fn remote_cache_owns_reusable_http_agent() {
        let cache = RemoteCache::new("https://cache.invalid/root/");

        assert_eq!(cache.base_url, "https://cache.invalid/root/");
        let _shared_agent = cache.agent.clone();
    }

    #[test]
    fn remote_cache_metadata_accepts_matching_fingerprint_source_and_device() {
        let fp = PipelineFingerprint::of(&tiny_program());
        let fp_hex = fp.hex();
        let expectation = RemoteMetadataExpectation {
            expected_source_provenance: Some("git:abc123"),
            expected_device_compatibility: Some("cuda-sm90"),
        };

        assert!(remote_metadata_allows(
            &fp,
            Some(&fp_hex),
            Some("git:abc123"),
            Some("cuda-sm90"),
            expectation,
        ));
    }

    #[test]
    fn remote_cache_metadata_rejects_stale_source_device_and_fingerprint() {
        let fp = PipelineFingerprint::of(&tiny_program());
        let fp_hex = fp.hex();
        let mut other = fp;
        other.0[0] ^= 0xFF;
        let other_hex = other.hex();
        let expectation = RemoteMetadataExpectation {
            expected_source_provenance: Some("git:abc123"),
            expected_device_compatibility: Some("cuda-sm90"),
        };

        assert!(!remote_metadata_allows(
            &fp,
            Some(&other_hex),
            Some("git:abc123"),
            Some("cuda-sm90"),
            expectation,
        ));
        assert!(!remote_metadata_allows(
            &fp,
            Some(&fp_hex),
            Some("git:stale"),
            Some("cuda-sm90"),
            expectation,
        ));
        assert!(!remote_metadata_allows(
            &fp,
            Some(&fp_hex),
            Some("git:abc123"),
            Some("metal-apple7"),
            expectation,
        ));
        assert!(!remote_metadata_allows(
            &fp,
            Some(&fp_hex),
            None,
            Some("cuda-sm90"),
            expectation,
        ));
    }
}
