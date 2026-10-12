use super::*;
/// Decode semantic scope records from a VYRECOB2 object byte stream.
pub fn decode_object_sema_scope(object_bytes: &[u8]) -> Result<CObjectSemaScope, String> {
    decode_embedded_object(object_bytes, decode_object_sema_scope_from_container)
}

pub(crate) fn decode_object_sema_scope_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectSemaScope, String> {
    let scope_section = container.section(SectionTag::SemaScope).ok_or_else(|| {
        "vyre-frontend-c object is missing SemaScope. Fix: compile with C semantic scope extraction enabled."
            .to_string()
    })?;
    let records = decode_c_sema_scope_records(scope_section)?;
    validate_sema_scope_records(&records)?;
    let declaration_rows = checked_count_u64(
        records
            .iter()
            .filter(|record| record.has_declaration())
            .count(),
        "SemaScope declaration row count",
    )?;
    let identifier_rows = checked_count_u64(
        records
            .iter()
            .filter(|record| record.has_identifier())
            .count(),
        "SemaScope identifier row count",
    )?;
    Ok(CObjectSemaScope {
        vyrecob2_version: container.version,
        records,
        declaration_rows,
        identifier_rows,
    })
}

pub(super) fn validate_sema_scope_records(records: &[CSemaScopeRecord]) -> Result<(), String> {
    let scope_ids: HashSet<u32> = records.iter().map(|record| record.scope_id).collect();
    let mut root_scope_ids = HashSet::with_capacity(scope_ids.len());
    for (idx, record) in records.iter().enumerate() {
        if !is_known_decl_kind(record.decl_kind) {
            return Err(format!(
                "vyre-frontend-c SemaScope row {idx} has unknown declaration kind {}. Fix: regenerate the object with supported semantic declaration metadata.",
                record.decl_kind
            ));
        }
        if record.has_declaration() && !record.has_identifier() {
            return Err(format!(
                "vyre-frontend-c SemaScope row {idx} declares `{}` without an identifier hash. Fix: regenerate the object with semantic identifier interning enabled.",
                record.decl_kind_name()
            ));
        }
        if record.has_identifier() && record.token_len == 0 {
            return Err(format!(
                "vyre-frontend-c SemaScope row {idx} has identifier hash {} but zero source length. Fix: preserve GPU lexer token spans when emitting semantic scope evidence.",
                record.identifier_id
            ));
        }
        if record.parent_scope_id == u32::MAX
            || (record.scope_id == 0 && record.parent_scope_id == 0)
        {
            root_scope_ids.insert(record.scope_id);
            continue;
        }
        if !scope_ids.contains(&record.parent_scope_id) {
            return Err(format!(
                "vyre-frontend-c SemaScope row {idx} references missing parent scope {}. Fix: regenerate the object; every non-root parent scope must be present in SemaScope.",
                record.parent_scope_id
            ));
        }
    }
    let root_scope_count = root_scope_ids.len();
    if root_scope_count != 1 {
        return Err(format!(
            "vyre-frontend-c SemaScope has {root_scope_count} root scope rows; expected exactly one. Fix: regenerate the object with a single translation-unit scope root."
        ));
    }
    Ok(())
}

/// Decode semantic scope records from a VYRECOB2 object file.
pub fn decode_object_sema_scope_file(path: &Path) -> Result<CObjectSemaScope, String> {
    read_object_file(path, decode_object_sema_scope)
}

pub(super) fn decode_c_sema_scope_records(bytes: &[u8]) -> Result<Vec<CSemaScopeRecord>, String> {
    const STRIDE: usize = 6;
    let words = decode_u32_words(bytes)?;
    if words.is_empty() {
        return Err(
            "vyre-frontend-c SemaScope section is empty. Fix: regenerate the object; semantic analysis must emit at least the root scope row."
                .to_string(),
        );
    }
    if words.len() % STRIDE != 0 {
        return Err(format!(
            "vyre-frontend-c SemaScope section has {} u32 words, not a multiple of stride {STRIDE}. Fix: regenerate the object.",
            words.len()
        ));
    }
    Ok(words
        .chunks_exact(STRIDE)
        .map(|row| CSemaScopeRecord {
            scope_id: row[0],
            parent_scope_id: row[1],
            decl_kind: row[2],
            identifier_id: row[3],
            token_start: row[4],
            token_len: row[5],
        })
        .collect())
}
