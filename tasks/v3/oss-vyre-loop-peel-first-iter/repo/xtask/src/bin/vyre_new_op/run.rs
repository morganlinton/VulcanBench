#![allow(missing_docs)]
use super::generate_mod_rs::generate_mod_rs;
use super::generate_readme::generate_readme;
use super::generate_required_impl_rs::{generate_required_impl_rs, RequiredImplKind};
use super::generate_spec_toml::generate_spec_toml;
use super::id_to_title_case::id_to_title_case;
use super::print_help::print_help;
use super::split_id_into_path::split_id_into_path;
use super::validate_archetype::validate_archetype;
use super::validate_id::validate_id;
use super::write_scaffold_file::write_scaffold_file;
use std::env;
use std::path::PathBuf;

pub(crate) fn run(args: &mut impl Iterator<Item = String>) -> Result<(), String> {
    let command = args.next().ok_or_else(|| {
        "Fix: missing command.\nUsage:\n  vyre new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]"
            .to_string()
    })?;
    if command == "--help" || command == "-h" {
        print_help();
        return Ok(());
    }
    if command != "new-op" {
        return Err(format!(
            "Fix: unknown command '{command}'. Use 'new-op'.\nUsage:\n  vyre new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]"
        ));
    }

    let mut id = None;
    let mut archetype = None;
    let mut display_name = None;
    let mut summary = None;
    let mut category = "C".to_string();

    let mut iter = args.peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--archetype" => {
                archetype = iter
                    .next()
                    .map(|value| {
                        if value.is_empty() {
                            Err("Fix: --archetype requires a non-empty value.".to_string())
                        } else {
                            Ok(value)
                        }
                    })
                    .transpose()?;
            }
            "--display-name" => {
                display_name = iter
                    .next()
                    .map(|value| {
                        if value.is_empty() {
                            Err("Fix: --display-name requires a non-empty value.".to_string())
                        } else {
                            Ok(value)
                        }
                    })
                    .transpose()?;
            }
            "--summary" => {
                summary = iter
                    .next()
                    .map(|value| {
                        if value.is_empty() {
                            Err("Fix: --summary requires a non-empty value.".to_string())
                        } else {
                            Ok(value)
                        }
                    })
                    .transpose()?;
            }
            "--category" => {
                category = iter
                    .next()
                    .map(|value| {
                        if value == "A" || value == "C" {
                            Ok(value)
                        } else {
                            Err(format!(
                                "Fix: invalid --category {value:?}. Supported values are A or C."
                            ))
                        }
                    })
                    .transpose()?
                    .unwrap_or_default();
            }
            _ => {
                if id.is_none() {
                    id = Some(arg);
                } else {
                    return Err(format!(
                        "Fix: unknown argument '{arg}'. Use --help for valid options."
                    ));
                }
            }
        }
    }

    let id = id.ok_or_else(|| {
        "Fix: missing <id>.\nUsage:\n  vyre new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]".to_string()
    })?;
    let archetype = archetype.ok_or_else(|| {
        "Fix: missing --archetype.\nUsage:\n  vyre new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]".to_string()
    })?;
    if category != "A" && category != "C" {
        return Err(format!(
            "Fix: invalid category {category:?}. Supported values are A and C."
        ));
    }

    validate_id(id.as_str())?;
    validate_archetype(archetype.as_str())?;
    let (family, subfamily, name) = split_id_into_path(id.as_str())?;
    let display_name = display_name.unwrap_or_else(|| id_to_title_case(id.as_str()));
    // LAW 9: no incomplete-work markers in generated files. If no summary was
    // provided on the CLI, derive one from the display name so the
    // generated spec.toml never ships incomplete-work text.
    let summary = summary.unwrap_or_else(|| {
        format!("One-line {archetype} operation `{display_name}`; populate spec.toml before certification.")
    });

    let spec_toml = generate_spec_toml(
        id.as_str(),
        archetype.as_str(),
        display_name.as_str(),
        summary.as_str(),
        category.as_str(),
    );
    let kernel_rs = generate_required_impl_rs(RequiredImplKind::Kernel);
    let lowering_wgsl_rs = generate_required_impl_rs(RequiredImplKind::WgslLowering);
    let mod_rs = generate_mod_rs();
    let readme = generate_readme(id.as_str(), archetype.as_str(), summary.as_str());

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let op_dir = manifest_dir
        .join("src")
        .join("ops")
        .join(family)
        .join(subfamily)
        .join(name);

    write_scaffold_file(op_dir.join("spec.toml"), &spec_toml)?;
    write_scaffold_file(op_dir.join("kernel.rs"), &kernel_rs)?;
    write_scaffold_file(op_dir.join("mod.rs"), &mod_rs)?;
    write_scaffold_file(op_dir.join("README.md"), &readme)?;
    write_scaffold_file(op_dir.join("lowering").join("wgsl.rs"), &lowering_wgsl_rs)?;

    println!("created operation files at {}", op_dir.display());
    println!("Implement kernel.rs, run cargo_full build, then cargo_full run -p vyre certify {id}");
    Ok(())
}
