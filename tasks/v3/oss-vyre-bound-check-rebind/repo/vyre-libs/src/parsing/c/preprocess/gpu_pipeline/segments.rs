use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::parsing::c::lex::tokens::{
    TOK_IDENTIFIER, TOK_LPAREN, TOK_RBRACE, TOK_RPAREN, TOK_SEMICOLON,
};

use super::buffers::checked_gpu_u32;
use super::{source_spans::checked_source_range, ClassifiedTokens, MacroDef};

pub(super) fn append_active_segment(
    segment: &mut Vec<u8>,
    segment_start: &mut Option<usize>,
    source: &[u8],
    start: usize,
    end: usize,
    label: &str,
) -> Result<(), String> {
    if segment.is_empty() {
        *segment_start = Some(start);
    }
    segment.extend_from_slice(checked_source_range(source, start, end, label)?);
    Ok(())
}

const MACRO_SEGMENT_SHARD_TOKEN_LIMIT: usize = 1024;

pub(super) fn macro_use_statement_ranges(
    classified: &ClassifiedTokens,
    macros: &[MacroDef],
) -> Result<Option<Vec<(usize, usize)>>, String> {
    if macros.is_empty() || classified.tok_types.len() <= 1 {
        return Ok(None);
    }
    let mut macro_lookup = HashMap::default();
    macro_lookup.try_reserve(macros.len()).map_err(|error| {
        format!(
            "vyre-libs::gpu_pipeline: could not reserve {} macro-use lookup entries: {error:?}. Fix: shard macro segment detection before GPU preprocessing.",
            macros.len()
        )
    })?;
    for mac in macros {
        macro_lookup.insert(mac.name.as_slice(), mac);
    }
    let next_boundaries = next_statement_boundaries(classified)?;
    let mut previous_boundary = 0_usize;
    let mut macro_ranges = Vec::<(usize, usize)>::new();
    for (idx, token_kind) in classified.tok_types.iter().enumerate() {
        if *token_kind == TOK_IDENTIFIER {
            let token = classified_token_bytes(classified, idx)?;
            if let Some(mac) = macro_lookup.get(token) {
                if mac.is_function_like {
                    let token_end = classified_token_end(classified, idx)?;
                    if next_non_ws_byte(&classified.source, token_end) != Some(b'(') {
                        if matches!(*token_kind, TOK_SEMICOLON | TOK_RBRACE) {
                            previous_boundary = classified_token_end(classified, idx)?;
                        }
                        continue;
                    }
                }
                let start = previous_boundary;
                let end = next_boundaries[idx];
                if end > start {
                    macro_ranges.push((start, end));
                }
            }
        }
        if matches!(*token_kind, TOK_SEMICOLON | TOK_RBRACE) {
            previous_boundary = classified_token_end(classified, idx)?;
        }
    }
    if macro_ranges.is_empty() {
        return Ok(None);
    }
    macro_ranges.sort_unstable_by_key(|range| range.0);
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in macro_ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    if merged.len() == 1 && merged[0] == (0, classified.source.len()) {
        return Ok(None);
    }
    let mut ranges = Vec::new();
    let mut cursor = 0_usize;
    for (start, end) in merged {
        if start > cursor {
            ranges.push((cursor, start));
        }
        ranges.push((start, end));
        cursor = end;
    }
    if cursor < classified.source.len() {
        ranges.push((cursor, classified.source.len()));
    }
    if ranges.len() <= 1 {
        return Ok(None);
    }
    Ok(Some(ranges))
}

pub(super) fn macro_segment_shard_ranges(
    classified: &ClassifiedTokens,
) -> Result<Option<Vec<(usize, usize)>>, String> {
    if classified.tok_types.len() <= MACRO_SEGMENT_SHARD_TOKEN_LIMIT {
        return Ok(None);
    }
    let mut ranges = Vec::new();
    let mut range_start = 0_usize;
    let mut range_start_token = 0_usize;
    let mut paren_depth = 0_u32;
    for (idx, token_kind) in classified.tok_types.iter().enumerate() {
        match *token_kind {
            TOK_LPAREN => paren_depth = paren_depth.saturating_add(1),
            TOK_RPAREN => paren_depth = paren_depth.saturating_sub(1),
            TOK_SEMICOLON | TOK_RBRACE if paren_depth == 0 => {
                let token_start = *classified.tok_starts.get(idx).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: shard token start missing. Fix: repair GPU lexer output column lengths.".to_string()
                })? as usize;
                let token_len = *classified.tok_lens.get(idx).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: shard token length missing. Fix: repair GPU lexer output column lengths.".to_string()
                })? as usize;
                let token_end = token_start.checked_add(token_len).ok_or_else(|| {
                    "vyre-libs::gpu_pipeline: shard token range overflow. Fix: repair GPU lexer spans before macro expansion.".to_string()
                })?;
                if idx + 1 - range_start_token >= MACRO_SEGMENT_SHARD_TOKEN_LIMIT
                    && token_end > range_start
                {
                    ranges.push((range_start, token_end));
                    range_start = token_end;
                    range_start_token = idx + 1;
                }
            }
            _ => {}
        }
    }
    if ranges.is_empty() {
        let mut hard_ranges = Vec::new();
        let mut start_byte = 0usize;
        let mut token_idx = MACRO_SEGMENT_SHARD_TOKEN_LIMIT;
        while token_idx < classified.tok_types.len() {
            let prev = token_idx - 1;
            let token_start = *classified.tok_starts.get(prev).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: hard shard token start missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let token_len = *classified.tok_lens.get(prev).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: hard shard token length missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let end_byte = token_start.checked_add(token_len).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: hard shard token range overflow. Fix: repair GPU lexer spans before macro expansion.".to_string()
            })?;
            if end_byte > start_byte {
                hard_ranges.push((start_byte, end_byte));
                start_byte = end_byte;
            }
            token_idx = token_idx.saturating_add(MACRO_SEGMENT_SHARD_TOKEN_LIMIT);
        }
        if start_byte < classified.source.len() {
            hard_ranges.push((start_byte, classified.source.len()));
        }
        return Ok((hard_ranges.len() > 1).then_some(hard_ranges));
    }
    if range_start < classified.source.len() {
        ranges.push((range_start, classified.source.len()));
    }
    if ranges.len() <= 1 {
        return Ok(None);
    }
    Ok(Some(ranges))
}

fn classified_token_bytes(classified: &ClassifiedTokens, idx: usize) -> Result<&[u8], String> {
    let start = *classified.tok_starts.get(idx).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: macro-use token start missing. Fix: repair GPU lexer output column lengths.".to_string()
    })? as usize;
    let end = classified_token_end(classified, idx)?;
    classified.source.get(start..end).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: macro-use token range outside source. Fix: repair GPU lexer spans before macro expansion.".to_string()
    })
}

fn classified_token_end(classified: &ClassifiedTokens, idx: usize) -> Result<usize, String> {
    let start = *classified.tok_starts.get(idx).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: macro-use token start missing. Fix: repair GPU lexer output column lengths.".to_string()
    })? as usize;
    let len = *classified.tok_lens.get(idx).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: macro-use token length missing. Fix: repair GPU lexer output column lengths.".to_string()
    })? as usize;
    start.checked_add(len).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: macro-use token range overflow. Fix: repair GPU lexer spans before macro expansion.".to_string()
    })
}

fn next_statement_boundaries(classified: &ClassifiedTokens) -> Result<Vec<usize>, String> {
    let mut next_boundaries = vec![classified.source.len(); classified.tok_types.len()];
    let mut next_boundary = classified.source.len();
    for idx in (0..classified.tok_types.len()).rev() {
        if matches!(classified.tok_types[idx], TOK_SEMICOLON | TOK_RBRACE) {
            next_boundary = classified_token_end(classified, idx)?;
        }
        next_boundaries[idx] = next_boundary;
    }
    Ok(next_boundaries)
}

pub(super) fn classified_segment(
    parent: &ClassifiedTokens,
    segment_start: usize,
    segment_len: usize,
) -> Result<ClassifiedTokens, String> {
    let segment_end = segment_start.checked_add(segment_len).ok_or_else(|| {
        "vyre-libs::gpu_pipeline: classified segment range overflow. Fix: shard preprocessing before segment flush.".to_string()
    })?;
    let source = parent
        .source
        .get(segment_start..segment_end)
        .ok_or_else(|| {
            "vyre-libs::gpu_pipeline: classified segment range outside parent source. Fix: preserve segment source offsets during directive walking.".to_string()
        })?
        .into();
    let mut tok_types = Vec::new();
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut directive_kinds = Vec::new();
    for idx in 0..parent.tok_types.len() {
        let start = *parent.tok_starts.get(idx).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: parent token start missing while slicing classified segment. Fix: repair GPU lexer output column lengths.".to_string()
        })? as usize;
        if start < segment_start {
            continue;
        }
        if start >= segment_end {
            break;
        }
        let len = *parent.tok_lens.get(idx).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: parent token length missing while slicing classified segment. Fix: repair GPU lexer output column lengths.".to_string()
        })? as usize;
        let end = start.checked_add(len).ok_or_else(|| {
            "vyre-libs::gpu_pipeline: parent token range overflow while slicing classified segment. Fix: repair GPU lexer spans before preprocessing.".to_string()
        })?;
        if end > segment_end {
            continue;
        }
        tok_types.push(parent.tok_types[idx]);
        tok_starts.push(checked_gpu_u32(
            "classified segment token start",
            start - segment_start,
        )?);
        tok_lens.push(parent.tok_lens[idx]);
        directive_kinds.push(parent.directive_kinds.get(idx).copied().unwrap_or(0));
    }
    Ok(ClassifiedTokens::from_parts(
        tok_types,
        tok_starts,
        tok_lens,
        directive_kinds,
        source,
    ))
}

#[derive(Default)]
pub(super) struct LiveMacroLookup {
    macro_len: usize,
    macro_ptr: usize,
    by_name: HashMap<Vec<u8>, usize>,
    used_flags: Vec<bool>,
    used_indexes: Vec<usize>,
    function_flags: Vec<bool>,
    prescan_flags: Vec<bool>,
    prescan_indexes: Vec<usize>,
}

impl LiveMacroLookup {
    pub(super) fn clear(&mut self) {
        self.macro_len = 0;
        self.macro_ptr = 0;
        self.by_name.clear();
        self.used_flags.clear();
        self.used_indexes.clear();
        self.function_flags.clear();
        self.prescan_flags.clear();
        self.prescan_indexes.clear();
    }

    fn refresh(&mut self, macros: &[MacroDef]) -> Result<(), String> {
        let macro_ptr = macros.as_ptr() as usize;
        if self.macro_len == macros.len() && self.macro_ptr == macro_ptr {
            self.used_flags.clear();
            self.used_flags.try_reserve_exact(macros.len()).map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} cached live macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    macros.len()
                )
            })?;
            self.used_flags.resize(macros.len(), false);
            self.used_indexes.clear();
            self.used_indexes.try_reserve_exact(macros.len()).map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} cached live macro indexes: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    macros.len()
                )
            })?;
            self.function_flags.clear();
            self.function_flags
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached function macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
            self.function_flags.resize(macros.len(), false);
            self.prescan_flags.clear();
            self.prescan_flags
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached prescan macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
            self.prescan_flags.resize(macros.len(), false);
            self.prescan_indexes.clear();
            self.prescan_indexes
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached prescan macro indexes: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
            return Ok(());
        }
        self.macro_len = macros.len();
        self.macro_ptr = macro_ptr;
        self.by_name.clear();
        self.by_name.try_reserve(macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} live macro lookup entries: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                macros.len()
            )
        })?;
        for (idx, mac) in macros.iter().enumerate() {
            self.by_name.insert(mac.name.clone(), idx);
        }
        self.used_flags.clear();
        self.used_flags.try_reserve_exact(macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} live macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                macros.len()
            )
        })?;
        self.used_flags.resize(macros.len(), false);
        self.used_indexes.clear();
        self.used_indexes.try_reserve_exact(macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} live macro indexes: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                macros.len()
            )
        })?;
        self.function_flags.clear();
        self.function_flags
            .try_reserve_exact(macros.len())
            .map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} function macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    macros.len()
                )
            })?;
        self.function_flags.resize(macros.len(), false);
        self.prescan_flags.clear();
        self.prescan_flags
            .try_reserve_exact(macros.len())
            .map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} prescan macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    macros.len()
                )
            })?;
        self.prescan_flags.resize(macros.len(), false);
        self.prescan_indexes.clear();
        self.prescan_indexes
            .try_reserve_exact(macros.len())
            .map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} prescan macro indexes: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    macros.len()
                )
            })?;
        Ok(())
    }

    fn live_macro_defs_for_segment(
        &mut self,
        macros: &[MacroDef],
        classified: &ClassifiedTokens,
    ) -> Result<Vec<MacroDef>, String> {
        self.refresh(macros)?;
        for (token_index, token_kind) in classified.tok_types.iter().enumerate() {
            if *token_kind != TOK_IDENTIFIER {
                continue;
            }
            let start = *classified.tok_starts.get(token_index).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token start missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let len = *classified.tok_lens.get(token_index).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token length missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let end = start.checked_add(len).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token range overflow. Fix: shard preprocessing before macro expansion.".to_string()
            })?;
            let token = classified.source.get(start..end).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token range outside source. Fix: repair GPU lexer spans before macro expansion.".to_string()
            })?;
            let Some(macro_index) = self.by_name.get(token).copied() else {
                continue;
            };
            let is_invocation = next_non_ws_byte(&classified.source, end) == Some(b'(');
            if macros[macro_index].is_function_like && !is_invocation {
                continue;
            }
            if !self.used_flags[macro_index] {
                self.used_flags[macro_index] = true;
                self.used_indexes.push(macro_index);
            }
        }
        let mut live = Vec::with_capacity(self.used_indexes.len());
        for &macro_index in &self.used_indexes {
            live.push(macros[macro_index].clone());
        }
        Ok(live)
    }

    fn has_live_macro_for_segment_excluding(
        &mut self,
        macros: &[MacroDef],
        classified: &ClassifiedTokens,
        excluded_names: &HashSet<&[u8]>,
    ) -> Result<bool, String> {
        if macros.is_empty() {
            return Ok(false);
        }
        self.refresh(macros)?;
        for (token_index, token_kind) in classified.tok_types.iter().enumerate() {
            if *token_kind != TOK_IDENTIFIER {
                continue;
            }
            let start = *classified.tok_starts.get(token_index).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token start missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let len = *classified.tok_lens.get(token_index).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token length missing. Fix: repair GPU lexer output column lengths.".to_string()
            })? as usize;
            let end = start.checked_add(len).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token range overflow. Fix: shard preprocessing before macro expansion.".to_string()
            })?;
            let token = classified.source.get(start..end).ok_or_else(|| {
                "vyre-libs::gpu_pipeline: macro-use prefilter token range outside source. Fix: repair GPU lexer spans before macro expansion.".to_string()
            })?;
            if excluded_names.contains(token) {
                continue;
            }
            let Some(macro_index) = self.by_name.get(token).copied() else {
                continue;
            };
            let is_invocation = next_non_ws_byte(&classified.source, end) == Some(b'(');
            if macros[macro_index].is_function_like && !is_invocation {
                continue;
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn function_argument_prescan_macros(
        &mut self,
        classified: &ClassifiedTokens,
        segment_macros: &[MacroDef],
        macros: &[MacroDef],
    ) -> Result<Option<Vec<MacroDef>>, String> {
        if !segment_macros.iter().any(|mac| mac.is_function_like) {
            return Ok(None);
        }
        let macro_ptr = macros.as_ptr() as usize;
        if self.macro_len != macros.len() || self.macro_ptr != macro_ptr {
            self.refresh(macros)?;
        } else {
            self.function_flags.clear();
            self.function_flags
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached function macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
            self.function_flags.resize(macros.len(), false);
            self.prescan_flags.clear();
            self.prescan_flags
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached prescan macro flags: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
            self.prescan_flags.resize(macros.len(), false);
            self.prescan_indexes.clear();
            self.prescan_indexes
                .try_reserve_exact(macros.len())
                .map_err(|error| {
                    format!(
                        "vyre-libs::gpu_pipeline: could not reserve {} cached prescan macro indexes: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                        macros.len()
                    )
                })?;
        }
        for mac in segment_macros.iter().filter(|mac| mac.is_function_like) {
            if let Some(index) = self.by_name.get(mac.name.as_slice()).copied() {
                self.function_flags[index] = true;
            }
        }
        for idx in 0..classified.tok_types.len().saturating_sub(1) {
            if classified.tok_types[idx] != TOK_IDENTIFIER
                || classified.tok_types[idx + 1] != TOK_LPAREN
            {
                continue;
            }
            let Some(name) = token_bytes(classified, idx) else {
                return Ok(None);
            };
            let Some(call_macro_index) = self.by_name.get(name).copied() else {
                continue;
            };
            if !self.function_flags[call_macro_index] {
                continue;
            }
            let Some(close) = matching_call_close(classified, idx + 1) else {
                return Ok(None);
            };
            for arg_idx in idx + 2..close {
                if classified.tok_types[arg_idx] != TOK_IDENTIFIER {
                    continue;
                }
                let Some(arg_name) = token_bytes(classified, arg_idx) else {
                    return Ok(None);
                };
                let Some(arg_macro_index) = self.by_name.get(arg_name).copied() else {
                    continue;
                };
                if arg_macro_index == call_macro_index || self.prescan_flags[arg_macro_index] {
                    continue;
                }
                self.prescan_flags[arg_macro_index] = true;
                self.prescan_indexes.push(arg_macro_index);
            }
        }
        if self.prescan_indexes.is_empty() {
            return Ok(None);
        }
        let mut prescan = Vec::new();
        prescan
            .try_reserve_exact(self.prescan_indexes.len())
            .map_err(|error| {
                format!(
                    "vyre-libs::gpu_pipeline: could not reserve {} function-argument prescan macro definitions: {error:?}. Fix: shard macro expansion before GPU preprocessing.",
                    self.prescan_indexes.len()
                )
            })?;
        for &macro_index in &self.prescan_indexes {
            prescan.push(macros[macro_index].clone());
        }
        Ok(Some(prescan))
    }
}

pub(super) fn live_macro_defs_for_segment(
    macros: &[MacroDef],
    classified: &ClassifiedTokens,
    lookup: &mut LiveMacroLookup,
) -> Result<Vec<MacroDef>, String> {
    lookup.live_macro_defs_for_segment(macros, classified)
}

pub(super) fn has_live_macro_for_segment_excluding(
    macros: &[MacroDef],
    classified: &ClassifiedTokens,
    excluded_names: &HashSet<&[u8]>,
    lookup: &mut LiveMacroLookup,
) -> Result<bool, String> {
    lookup.has_live_macro_for_segment_excluding(macros, classified, excluded_names)
}

fn next_non_ws_byte(source: &[u8], mut pos: usize) -> Option<u8> {
    loop {
        while source
            .get(pos)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            pos += 1;
        }
        if source.get(pos..pos.saturating_add(2)) == Some(&b"/*"[..]) {
            pos = pos.saturating_add(2);
            while source.get(pos..pos.saturating_add(2)) != Some(&b"*/"[..]) {
                pos = pos.saturating_add(1);
                if pos >= source.len() {
                    return None;
                }
            }
            pos = pos.saturating_add(2);
            continue;
        }
        if source.get(pos..pos.saturating_add(2)) == Some(&b"//"[..]) {
            pos = pos.saturating_add(2);
            while source.get(pos).is_some_and(|byte| *byte != b'\n') {
                pos = pos.saturating_add(1);
            }
            continue;
        }
        break;
    }
    source.get(pos).copied()
}

fn matching_call_close(classified: &ClassifiedTokens, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for idx in open_idx..classified.tok_types.len() {
        match classified.tok_types[idx] {
            TOK_LPAREN => depth = depth.checked_add(1)?,
            TOK_RPAREN => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn token_bytes(classified: &ClassifiedTokens, idx: usize) -> Option<&[u8]> {
    let start = *classified.tok_starts.get(idx)? as usize;
    let len = *classified.tok_lens.get(idx)? as usize;
    classified.source.get(start..start.checked_add(len)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macro_def(name: &[u8], is_function_like: bool) -> MacroDef {
        MacroDef {
            name: name.to_vec(),
            args: Vec::new(),
            body: b"1".to_vec(),
            is_function_like,
        }
    }

    #[test]
    fn macro_use_statement_ranges_indexes_names_and_precomputes_boundaries() {
        let source = b"a FOO;\nb BAR(1);\nc BAR;\n".to_vec();
        let classified = ClassifiedTokens {
            tok_types: vec![
                TOK_IDENTIFIER,
                TOK_SEMICOLON,
                TOK_IDENTIFIER,
                TOK_SEMICOLON,
                TOK_IDENTIFIER,
                TOK_SEMICOLON,
            ],
            tok_starts: vec![2, 5, 9, 15, 19, 22],
            tok_lens: vec![3, 1, 3, 1, 3, 1],
            directive_kinds: vec![0; 6],
            directive_count: 0,
            source: source.into(),
        };
        let macros = vec![macro_def(b"FOO", false), macro_def(b"BAR", true)];

        let ranges = macro_use_statement_ranges(&classified, &macros)
            .expect("Fix: macro use range discovery must not fail on valid token columns")
            .expect("Fix: macro use range discovery must shard mixed macro/plain source");

        assert_eq!(ranges, vec![(0, 16), (16, 24)]);
    }

    #[test]
    fn live_macro_defs_prefilter_distinguishes_object_and_function_uses() {
        let classified = ClassifiedTokens {
            tok_types: vec![TOK_IDENTIFIER, TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER],
            tok_starts: vec![0, 4, 6, 8],
            tok_lens: vec![3, 2, 1, 2],
            directive_kinds: vec![0; 4],
            directive_count: 0,
            source: std::sync::Arc::from(&b"OBJ FN( FN"[..]),
        };
        let macros = vec![macro_def(b"OBJ", false), macro_def(b"FN", true)];
        let mut lookup = LiveMacroLookup::default();

        let live = live_macro_defs_for_segment(&macros, &classified, &mut lookup)
            .expect("Fix: valid token spans must prefilter macro uses");
        let live = live
            .iter()
            .map(|mac| (mac.name.as_slice(), mac.is_function_like))
            .collect::<Vec<_>>();

        assert_eq!(
            live,
            vec![(b"OBJ".as_slice(), false), (b"FN".as_slice(), true)]
        );
    }

    #[test]
    fn live_macro_defs_prefilter_rejects_bare_function_like_identifier_mentions() {
        let classified = ClassifiedTokens {
            tok_types: vec![TOK_IDENTIFIER, TOK_IDENTIFIER],
            tok_starts: vec![0, 3],
            tok_lens: vec![2, 3],
            directive_kinds: vec![0; 2],
            directive_count: 0,
            source: std::sync::Arc::from(&b"FN OBJ"[..]),
        };
        let macros = vec![macro_def(b"FN", true), macro_def(b"OBJ", false)];
        let mut lookup = LiveMacroLookup::default();

        let live = live_macro_defs_for_segment(&macros, &classified, &mut lookup)
            .expect("Fix: valid token spans must prefilter bare function-like mentions");
        let live = live
            .iter()
            .map(|mac| (mac.name.as_slice(), mac.is_function_like))
            .collect::<Vec<_>>();

        assert_eq!(live, vec![(b"OBJ".as_slice(), false)]);
    }

    #[test]
    fn function_like_macro_prefilter_treats_comments_as_invocation_whitespace() {
        let classified = ClassifiedTokens {
            tok_types: vec![TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER],
            tok_starts: vec![0, 14, 16],
            tok_lens: vec![2, 1, 2],
            directive_kinds: vec![0; 3],
            directive_count: 0,
            source: std::sync::Arc::from(&b"FN /* comment */(x"[..]),
        };
        let macros = vec![macro_def(b"FN", true)];
        let mut lookup = LiveMacroLookup::default();

        let live = live_macro_defs_for_segment(&macros, &classified, &mut lookup)
            .expect("Fix: comment-separated function-like macro invocation must prefilter");

        assert_eq!(live.len(), 1);
        assert_eq!(live[0].name, b"FN");
    }

    #[test]
    fn classified_segment_stops_after_segment_end() {
        let classified = ClassifiedTokens {
            tok_types: vec![TOK_IDENTIFIER, TOK_SEMICOLON, TOK_IDENTIFIER],
            tok_starts: vec![0, 3, 10],
            tok_lens: vec![3, 1, u32::MAX],
            directive_kinds: vec![0, 0, 0],
            directive_count: 0,
            source: std::sync::Arc::from(&b"FOO;      BAD"[..]),
        };

        let segment = classified_segment(&classified, 0, 4)
            .expect("Fix: token metadata past the segment end must not be scanned");

        assert_eq!(segment.tok_types, vec![TOK_IDENTIFIER, TOK_SEMICOLON]);
        assert_eq!(segment.tok_starts, vec![0, 3]);
        assert_eq!(segment.tok_lens, vec![3, 1]);
    }
}
