use vyre_macros::skip_builder;

#[skip_builder]
struct Example {
    value: u32,
}

fn main() {
    let example = Example { value: 7 };
    assert_eq!(example.value, 7);
}
