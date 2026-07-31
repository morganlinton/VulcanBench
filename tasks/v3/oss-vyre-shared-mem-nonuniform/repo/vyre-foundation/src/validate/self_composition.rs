//! Validation for self-exclusive regions.

#[cfg(test)]
use crate::composition::duplicate_self_exclusive_regions;
#[cfg(test)]
use crate::ir::Node;
#[cfg(test)]
use crate::validate::{err, ValidationError};

/// Reject programs that compose the same self-exclusive region twice.
#[cfg(test)]
pub(crate) fn validate_self_composition(nodes: &[Node], errors: &mut Vec<ValidationError>) {
    for generator in duplicate_self_exclusive_regions(nodes) {
        errors.push(err(format!(
            "region `{generator}` is marked non-composable with itself but appears multiple times in one fused program. Fix: split the parser into separate dispatches, or give each instance distinct scratch storage before fusion."
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::mark_self_exclusive_region;
    use crate::ir::{Node, Program};
    use std::sync::Arc;

    #[test]
    fn duplicate_self_exclusive_generator_is_rejected() {
        let generator = mark_self_exclusive_region("vyre.parser.core_delimiter_match");
        let program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![
                Node::Region {
                    generator: generator.clone().into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
                Node::Region {
                    generator: generator.into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
            ],
        );

        let mut errors = Vec::new();
        validate_self_composition(program.entry(), &mut errors);
        assert!(errors
            .iter()
            .any(|error| { error.message.contains("marked non-composable with itself") }));
    }
}
