use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CandidateKind {
    OptimizerRule,
    PassOrder,
    FusionPolicy,
    VectorizationPolicy,
    WorkgroupPolicy,
    MegakernelBatchPolicy,
    CacheRetentionPolicy,
    BackendLoweringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateManifest {
    pub name: String,
    pub description: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub kind: CandidateKind,
    pub manifest: CandidateManifest,
    pub patch_digest: Option<[u8; 32]>,
}
