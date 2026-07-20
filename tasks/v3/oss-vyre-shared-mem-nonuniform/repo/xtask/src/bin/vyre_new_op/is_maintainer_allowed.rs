#![allow(missing_docs)]
use super::reserved_id_env::RESERVED_ID_ENV;
use std::env;

pub(crate) fn is_maintainer_allowed() -> bool {
    env::var(RESERVED_ID_ENV)
        .ok()
        .is_some_and(|value| value == "1")
}
