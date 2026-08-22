//! Hidden pass-to-pass guards: derived synopsis and help output unchanged.

fn render(cmd: clap::Command) -> String {
    let mut buf = Vec::new();
    clap_mangen::Man::new(cmd).render(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn vb_derived_synopsis_still_rendered_without_override() {
    let cmd = clap::Command::new("my-app").about("about").arg(
        clap::Arg::new("all")
            .short('a')
            .long("all")
            .help("Do not ignore entries starting with .")
            .action(clap::ArgAction::SetTrue),
    );
    let page = render(cmd);
    assert!(page.contains(".SH SYNOPSIS"), "man page must have a SYNOPSIS: {page:?}");
    assert!(page.contains("\\fBmy\\-app\\fR"), "derived synopsis must show the name: {page:?}");
    assert!(page.contains("\\-\\-all"), "derived synopsis/options must mention the flag: {page:?}");
}

#[test]
fn vb_help_output_still_shows_override_usage() {
    let mut cmd = clap::Command::new("my-app")
        .about("about")
        .override_usage("my-app CUSTOM_FORM");
    let help = cmd.render_help().to_string();
    assert!(help.contains("my-app CUSTOM_FORM"), "help must render the override: {help:?}");
}

#[test]
fn vb_man_page_sections_present() {
    let cmd = clap::Command::new("my-app").about("does things");
    let page = render(cmd);
    for section in [".SH NAME", ".SH SYNOPSIS", ".SH DESCRIPTION"] {
        assert!(page.contains(section), "missing {section}: {page:?}");
    }
}
