use crate::repo_boundary;

pub(crate) const PUBLISH_ACTION: &str = "cargo_full publish approved crates in dependency order";
pub(crate) const GIT_PUSH_ACTION: &str = "git push release branch and tags";

pub(crate) fn required_external_actions() -> [&'static str; 3] {
    [
        PUBLISH_ACTION,
        repo_boundary::verify_public_repo_action(),
        GIT_PUSH_ACTION,
    ]
}
