#![allow(missing_docs)]
pub(crate) fn generate_spec_toml(
    id: &str,
    archetype: &str,
    display_name: &str,
    summary: &str,
    category: &str,
) -> String {
    let composition = if category == "A" {
        "\n# Composition-backed ops must list composed dependencies.\ncomposition_of = []\n"
    } else {
        ""
    };

    // A Category-C op must declare backend-intrinsic spellings. Leave
    // them blank (empty strings) by default  -  the spec loader rejects
    // empty intrinsic strings with a `Fix:`-prefixed error, so the
    // contributor has to supply them before the op lands, and the
    // generated file never ships with incomplete-work text that the
    // release hygiene enforcer would flag on contributor PRs.
    let intrinsic = if category == "C" {
        "\n[intrinsic]\nwgsl = \"\"\nspirv = \"\"\ncuda = \"\"\nmetal = \"\"\n"
    } else {
        ""
    };

    format!(
        r##"schema_version = 1

id = "{id}"
archetype = "{archetype}"
display_name = "{display_name}"
summary = "{summary}"
category = "{category}"{composition}
{intrinsic}
[signature]
inputs = ["U32", "U32"]
output = "U32"

# Concrete laws from `vyre_spec::AlgebraicLaw` go here (Commutative,
# Associative, Identity {{ element = ... }}, …). An empty list is
# valid for ops that declare no algebraic claims; the conform gate
# then only checks parity, not law verification.
laws = []

# Adversarial equivalence classes for this op. Empty is valid; the
# adversarial gauntlet widens coverage later. Populate once mutation
# testing surfaces a defect the default classes miss.
equivalence_classes = []

workgroup_size = [64, 1, 1]

tags = []
"##
    )
}
