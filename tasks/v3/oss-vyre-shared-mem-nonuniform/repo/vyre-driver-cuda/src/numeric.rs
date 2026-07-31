use vyre_driver::numeric::BackendNumericPolicy;

pub(crate) const CUDA_NUMERIC: BackendNumericPolicy = BackendNumericPolicy::new("CUDA");

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("numeric.rs");

    #[test]
    fn cuda_numeric_module_is_policy_binding_not_helper_fork() {
        assert_eq!(CUDA_NUMERIC.backend(), "CUDA");
        assert!(
            SOURCE.contains("BackendNumericPolicy::new(\"CUDA\")"),
            "CUDA numeric ownership must stay in vyre-driver::numeric"
        );
        let forbidden_wrapper = concat!("pub(crate) ", "fn");
        assert!(
            !SOURCE.contains(forbidden_wrapper),
            "CUDA must not reintroduce per-helper numeric wrappers"
        );
    }
}
