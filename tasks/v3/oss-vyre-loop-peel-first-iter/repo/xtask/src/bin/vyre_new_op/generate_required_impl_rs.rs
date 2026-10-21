#![allow(missing_docs)]

pub(crate) enum RequiredImplKind {
    Kernel,
    WgslLowering,
}

pub(crate) fn generate_required_impl_rs(kind: RequiredImplKind) -> String {
    match kind {
        RequiredImplKind::Kernel => {
            r#"pub fn execute(_inputs: &[u8]) -> Vec<u8> {
    compile_error!("new operations must define a concrete kernel before this module is added to a workspace build");
}
"#
        }
        RequiredImplKind::WgslLowering => {
            r#"compile_error!("new operations must define a concrete WGSL lowering before this module is added to a workspace build");
"#
        }
    }
    .to_string()
}
