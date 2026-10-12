use super::*;

#[test]
fn self_substrate_root_contains_no_flat_domain_modules() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");
    let mut flat_modules = Vec::new();

    for entry in std::fs::read_dir(&source_root)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", source_root.display()))
    {
        let path = entry
            .unwrap_or_else(|err| panic!("src/ entry must be readable: {err}"))
            .path();
        if path.is_file() && path.file_name().is_some_and(|name| name != "lib.rs") {
            flat_modules.push(
                path.file_name()
                    .expect("root file must have a name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    assert!(
        flat_modules.is_empty(),
        "vyre-self-substrate must not regress to flat src/ modules; move files into domain directories: {flat_modules:?}"
    );
}

#[test]
fn every_domain_module_is_declared_by_its_domain_mod_file() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest.join("src");

    for (domain, modules) in DOMAIN_MODULES {
        let mod_path = source_root.join(domain).join("mod.rs");
        let mod_source = std::fs::read_to_string(&mod_path)
            .unwrap_or_else(|err| panic!("{} must be readable: {err}", mod_path.display()));

        for module_file in *modules {
            let module_path = source_root.join(domain).join(module_file);
            let directory_module_path = module_file
                .strip_suffix(".rs")
                .map(|stem| source_root.join(domain).join(stem).join("mod.rs"));
            assert!(
                module_path.exists()
                    || directory_module_path
                        .as_ref()
                        .is_some_and(|path| path.exists()),
                "{domain}/{module_file} must exist because it is part of the self-substrate organization contract"
            );
            let stem = module_file
                .strip_suffix(".rs")
                .expect("organization module entries must be Rust source files");
            assert!(
                mod_source.contains(&format!("mod {stem};")),
                "{domain}/mod.rs must declare mod {stem}; so imports cross the domain boundary through one file"
            );
        }
    }
}

