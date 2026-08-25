use super::DirectivePayload;

pub(super) fn directive_payloads_bytes(payloads: &[DirectivePayload]) -> usize {
    let mut bytes = payloads
        .len()
        .checked_mul(std::mem::size_of::<DirectivePayload>())
        .unwrap_or(usize::MAX);
    for payload in payloads {
        let dynamic = match payload {
            DirectivePayload::None
            | DirectivePayload::Ifdef { .. }
            | DirectivePayload::IfExpr { .. }
            | DirectivePayload::Else
            | DirectivePayload::Endif
            | DirectivePayload::Other => 0,
            DirectivePayload::Define {
                name, args, body, ..
            } => name
                .len()
                .checked_add(args.len())
                .and_then(|value| value.checked_add(body.len()))
                .unwrap_or(usize::MAX),
            DirectivePayload::Undef { name } => name.len(),
            DirectivePayload::Include { path, .. } => path.len(),
        };
        bytes = bytes.checked_add(dynamic).unwrap_or(usize::MAX);
    }
    bytes
}
