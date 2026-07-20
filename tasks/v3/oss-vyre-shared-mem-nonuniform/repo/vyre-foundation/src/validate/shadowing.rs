use crate::validate::{err, Binding, ValidationError, ValidationOptions};
use rustc_hash::FxHashMap;

#[inline]
pub(crate) fn check_local(
    name: &str,
    scope: &FxHashMap<crate::ir::Ident, Binding>,
    options: ValidationOptions<'_>,
    errors: &mut Vec<ValidationError>,
) {
    if !options.allow_shadowing && scope.contains_key(name) {
        errors.push(err(format!(
            "V008: duplicate local binding `{name}` shadows an outer scope. Fix: choose a unique local name, or opt into nested shadowing with ValidationOptions::with_shadowing(true)."
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::DataType;

    fn scope_with(name: &str) -> FxHashMap<crate::ir::Ident, Binding> {
        let mut scope = FxHashMap::default();
        scope.insert(
            crate::ir::Ident::from(name),
            Binding {
                ty: DataType::U32,
                ty_known: true,
                mutable: true,
                uniform: true,
            },
        );
        scope
    }

    #[test]
    fn shadowing_detected_by_default() {
        let scope = scope_with("x");
        let mut errors = Vec::new();
        check_local("x", &scope, ValidationOptions::default(), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("V008"));
    }

    #[test]
    fn shadowing_allowed_with_opt_in() {
        let scope = scope_with("x");
        let mut errors = Vec::new();
        let options = ValidationOptions::default().with_shadowing(true);
        check_local("x", &scope, options, &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn new_name_never_errors() {
        let scope = scope_with("x");
        let mut errors = Vec::new();
        check_local("y", &scope, ValidationOptions::default(), &mut errors);
        assert!(errors.is_empty());
    }
}
