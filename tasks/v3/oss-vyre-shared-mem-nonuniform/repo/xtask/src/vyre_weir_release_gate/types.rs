use serde::Deserialize;

pub(super) const MAX_RELEASE_GATE_TEXT_BYTES: u64 = 16_777_216;

#[derive(Debug, Deserialize)]
pub(super) struct EvidenceManifest {
    pub(super) schema_version: u32,
    pub(super) plan_path: String,
    pub(super) release: ReleaseNames,
    pub(super) requirements: Vec<Requirement>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseNames {
    pub(super) vyre: String,
    pub(super) weir: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Requirement {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) evidence: Vec<String>,
    pub(super) minimum_evidence: usize,
}
