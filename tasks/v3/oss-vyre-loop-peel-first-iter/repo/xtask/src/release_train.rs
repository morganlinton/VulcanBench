use std::sync::OnceLock;

use serde::Deserialize;

pub(crate) const RELEASE_TRAIN_TOML_PATH: &str = "release/release-train.toml";
const RELEASE_TRAIN_TOML: &str = include_str!("../../release/release-train.toml");

#[derive(Debug, Deserialize)]
struct ReleaseTrainData {
    versions: Versions,
    tags: Tags,
    required_release_note_tokens: Vec<String>,
    required_packaging_steps: Vec<String>,
    package_verify_passed: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Versions {
    vyre: String,
    weir: String,
    vyrec: String,
    vyrec_train: String,
    vyre_frontend_c: String,
}

#[derive(Debug, Deserialize)]
struct Tags {
    vyre_rc: String,
    weir_rc: String,
    combined_release_train_rc: String,
    vyre: String,
    weir: String,
    combined_release_train: String,
    policy: String,
}

static RELEASE_TRAIN: OnceLock<Result<ReleaseTrainData, String>> = OnceLock::new();

fn data() -> &'static ReleaseTrainData {
    crate::toml_config::data_or_exit(RELEASE_TRAIN.get_or_init(|| {
        crate::toml_config::parse_embedded_toml(RELEASE_TRAIN_TOML_PATH, RELEASE_TRAIN_TOML)
    }))
}

pub(crate) fn vyre_version() -> &'static str {
    data().versions.vyre.as_str()
}

pub(crate) fn weir_version() -> &'static str {
    data().versions.weir.as_str()
}

pub(crate) fn vyrec_version() -> &'static str {
    data().versions.vyrec.as_str()
}

pub(crate) fn vyrec_train_version() -> &'static str {
    data().versions.vyrec_train.as_str()
}

pub(crate) fn vyre_frontend_c_version() -> &'static str {
    data().versions.vyre_frontend_c.as_str()
}

pub(crate) fn vyre_rc_tag() -> &'static str {
    data().tags.vyre_rc.as_str()
}

pub(crate) fn weir_rc_tag() -> &'static str {
    data().tags.weir_rc.as_str()
}

pub(crate) fn combined_release_train_rc_tag() -> &'static str {
    data().tags.combined_release_train_rc.as_str()
}

pub(crate) fn vyre_tag() -> &'static str {
    data().tags.vyre.as_str()
}

pub(crate) fn weir_tag() -> &'static str {
    data().tags.weir.as_str()
}

pub(crate) fn combined_release_train_tag() -> &'static str {
    data().tags.combined_release_train.as_str()
}

pub(crate) fn tag_story_fields() -> [(&'static str, &'static str); 6] {
    [
        ("vyre_rc_tag", vyre_rc_tag()),
        ("weir_rc_tag", weir_rc_tag()),
        ("combined_release_train_rc_tag", combined_release_train_rc_tag()),
        ("vyre_tag", vyre_tag()),
        ("weir_tag", weir_tag()),
        ("combined_release_train_tag", combined_release_train_tag()),
    ]
}

pub(crate) fn tag_creation_order() -> [&'static str; 6] {
    [
        vyre_rc_tag(),
        weir_rc_tag(),
        combined_release_train_rc_tag(),
        vyre_tag(),
        weir_tag(),
        combined_release_train_tag(),
    ]
}

pub(crate) fn rc_to_final_tags() -> [(&'static str, &'static str); 3] {
    [
        (vyre_rc_tag(), vyre_tag()),
        (weir_rc_tag(), weir_tag()),
        (combined_release_train_rc_tag(), combined_release_train_tag()),
    ]
}

pub(crate) fn tag_policy() -> &'static str {
    data().tags.policy.as_str()
}

pub(crate) fn required_release_note_tokens() -> Vec<&'static str> {
    data()
        .required_release_note_tokens
        .iter()
        .map(String::as_str)
        .collect()
}

pub(crate) fn required_packaging_steps() -> Vec<&'static str> {
    data()
        .required_packaging_steps
        .iter()
        .map(String::as_str)
        .collect()
}

pub(crate) fn package_verify_passed() -> Vec<&'static str> {
    data()
        .package_verify_passed
        .iter()
        .map(String::as_str)
        .collect()
}

pub(crate) fn required_release_packages() -> [(&'static str, &'static str, &'static str); 6] {
    [
        ("vyre", vyre_version(), "vyre"),
        ("vyre-driver-cuda", vyre_version(), "vyre"),
        ("vyre-driver-wgpu", vyre_version(), "vyre"),
        ("weir", weir_version(), "weir"),
        ("vyrec", vyrec_version(), "vyre"),
        ("vyre-frontend-c", vyre_frontend_c_version(), "vyre"),
    ]
}

pub(crate) fn release_group_version(group: &str) -> Option<&'static str> {
    match group {
        "vyre" => Some(vyre_version()),
        "weir" => Some(weir_version()),
        _ => None,
    }
}
