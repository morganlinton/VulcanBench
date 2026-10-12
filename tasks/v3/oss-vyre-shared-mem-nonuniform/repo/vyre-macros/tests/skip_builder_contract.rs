#![allow(missing_docs)]

use vyre_macros::skip_builder;

#[skip_builder]
pub struct PassthroughField {
    value: u32,
}

#[test]
fn skip_builder_attribute_preserves_item_shape() {
    let field = PassthroughField { value: 42 };
    assert_eq!(field.value, 42);
}
