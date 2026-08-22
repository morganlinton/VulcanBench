fn main() {
    let versions = toml_test_data::versions();
    println!("{versions:#?}");

    let valid = toml_test_data::valid().collect::<Vec<_>>();
    println!("{valid:#?}");

    let invalid = toml_test_data::invalid().collect::<Vec<_>>();
    println!("{invalid:#?}");
}
