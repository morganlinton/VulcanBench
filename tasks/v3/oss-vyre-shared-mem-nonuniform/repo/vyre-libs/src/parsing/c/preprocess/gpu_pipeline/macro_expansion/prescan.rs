use super::*;

pub(crate) fn function_argument_prescan_macros(
    classified: &ClassifiedTokens,
    segment_macros: &[MacroDef],
    macros: &[MacroDef],
    lookup: &mut LiveMacroLookup,
) -> Result<Option<Vec<MacroDef>>, String> {
    lookup.function_argument_prescan_macros(classified, segment_macros, macros)
}
