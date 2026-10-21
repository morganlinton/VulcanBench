use vyre_macros::vyre_pass;

#[vyre_pass(name = "bad_item_kind", requires = [], invalidates = [])]
pub enum BadItemKind {
    Variant,
}

fn main() {}
