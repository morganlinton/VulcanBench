//! Hidden fail-to-pass tests: the man-page SYNOPSIS must render the usage set
//! with `Command::override_usage` instead of the one derived from arguments.

fn render(cmd: clap::Command) -> String {
    let mut buf = Vec::new();
    clap_mangen::Man::new(cmd).render(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn synopsis_section(page: &str) -> &str {
    let start = page.find(".SH SYNOPSIS").expect("man page has a SYNOPSIS");
    let rest = &page[start..];
    let end = rest[4..].find(".SH ").map(|i| i + 4).unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn vb_single_form_override_is_rendered() {
    let cmd = clap::Command::new("my-app")
        .about("Check file types and compare values")
        .override_usage("my-app [-clDas] <some_file>");
    let page = render(cmd);
    let syn = synopsis_section(&page);
    assert!(
        syn.contains("\\fBmy\\-app\\fR [\\-clDas] <some_file>"),
        "SYNOPSIS must show the override with the command name in bold: {syn:?}"
    );
}

#[test]
fn vb_multi_form_override_keeps_every_form() {
    let cmd = clap::Command::new("my-app")
        .about("Check file types and compare values")
        .override_usage(
            "my-app [OPTION]... EXPRESSION\n       my-app\n       [ EXPRESSION ]\n       ./my-app EXPRESSION",
        )
        .arg(
            clap::Arg::new("all")
                .short('a')
                .long("all")
                .help("Do not ignore entries starting with .")
                .action(clap::ArgAction::SetTrue),
        );
    let page = render(cmd);
    let syn = synopsis_section(&page);
    assert!(syn.contains("[OPTION]... EXPRESSION"), "first form missing: {syn:?}");
    assert!(syn.contains("[ EXPRESSION ]"), "bare-bracket form missing: {syn:?}");
    assert!(syn.contains("./my\\-app EXPRESSION"), "non-name form missing: {syn:?}");
    // One text line per form, separated by explicit breaks.
    assert!(syn.matches(".br").count() >= 3, "forms must be on separate lines: {syn:?}");
}

#[test]
fn vb_override_replaces_derived_synopsis() {
    let cmd = clap::Command::new("my-app")
        .about("about")
        .override_usage("my-app FILE...")
        .arg(clap::Arg::new("verbose").long("verbose").action(clap::ArgAction::SetTrue));
    let page = render(cmd);
    let syn = synopsis_section(&page);
    assert!(syn.contains("FILE..."), "override text missing: {syn:?}");
    assert!(
        !syn.contains("verbose"),
        "derived argument synopsis must not appear when overridden: {syn:?}"
    );
}

#[test]
fn vb_non_name_form_is_not_bolded() {
    let cmd = clap::Command::new("my-app")
        .about("about")
        .override_usage("my-app FILE\n       ./my-app FILE");
    let page = render(cmd);
    let syn = synopsis_section(&page);
    assert!(!syn.contains("\\fB./"), "forms not starting with the name stay roman: {syn:?}");
    assert!(syn.contains("./my\\-app FILE"), "non-name form must still appear: {syn:?}");
}
