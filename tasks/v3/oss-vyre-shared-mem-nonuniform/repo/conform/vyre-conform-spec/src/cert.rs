//! Certification helpers shared by backend conformance tools.

/// Deterministic backend capability fingerprints.
pub mod fingerprint;

#[cfg(test)]
pub(crate) mod tests {
    use super::fingerprint::{BackendFingerprint, ProbeObservation};

    #[test]
    fn fingerprint_stable_across_runs() {
        let observation = ProbeObservation::new("wgpu", "nvidia", 32, 0, 1);

        let first = BackendFingerprint::from_observation(&observation);
        let second = BackendFingerprint::from_observation(&observation);

        assert_eq!(first, second);
    }

    #[test]
    fn fingerprint_diverges_on_simulated_driver_change() {
        let old = ProbeObservation::new("wgpu", "nvidia", 32, 0, 1);
        let new = ProbeObservation::new("wgpu", "nvidia", 32, 1, 1);

        assert_ne!(
            BackendFingerprint::from_observation(&old),
            BackendFingerprint::from_observation(&new)
        );
    }
}
