use crate::parsing::c::preprocess::gpu_pipeline::DirectivePayload;

#[derive(Default)]
pub(super) struct IncludeGuardIfndefNames {
    guard: Option<IncludeGuardIfndefName>,
}

struct IncludeGuardIfndefName {
    ifndef_row: usize,
    name: Vec<u8>,
}

impl IncludeGuardIfndefNames {
    pub(super) fn name_at(&self, row: usize) -> Option<&[u8]> {
        self.guard
            .as_ref()
            .filter(|guard| guard.ifndef_row == row)
            .map(|guard| guard.name.as_slice())
    }
}

pub(super) fn include_guard_ifndef_names(payloads: &[DirectivePayload]) -> IncludeGuardIfndefNames {
    let mut opening_ifndef = None;
    for (idx, payload) in payloads
        .iter()
        .enumerate()
        .filter(|(_, payload)| !matches!(payload, DirectivePayload::None))
        .take(32)
    {
        match (opening_ifndef, payload) {
            (None, DirectivePayload::Other) => {}
            (
                None,
                DirectivePayload::Ifdef {
                    value: _,
                    negated: true,
                },
            ) => opening_ifndef = Some(idx),
            (Some(_), DirectivePayload::Other) => {}
            (Some(ifndef_idx), DirectivePayload::Define { name, .. }) if !name.is_empty() => {
                return IncludeGuardIfndefNames {
                    guard: Some(IncludeGuardIfndefName {
                        ifndef_row: ifndef_idx,
                        name: name.clone(),
                    }),
                };
            }
            _ => break,
        }
    }
    IncludeGuardIfndefNames::default()
}
