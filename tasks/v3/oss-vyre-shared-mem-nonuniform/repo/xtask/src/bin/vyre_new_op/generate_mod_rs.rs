#![allow(missing_docs)]
pub(crate) fn generate_mod_rs() -> String {
    r#"//! Operation scaffold module.

pub mod lowering {
    pub mod wgsl;
}
"#
    .to_string()
}
