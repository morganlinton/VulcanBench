use std::process;

pub(crate) fn cmd_quick_check(args: &[String]) {
    let mut op: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--op" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Fix: --op requires an operation name.");
                    process::exit(1);
                }
                op = Some(args[i].clone());
            }
            "--help" | "-h" => {
                println!(
                    "cargo_full run --bin xtask -- quick-check --op NAME\n\
                     \n\
                     Runs the minimal <5s verification path for a single op.\n"
                );
                process::exit(0);
            }
            _ => {
                eprintln!("Fix: unknown argument '{}'. See --help.", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let Some(op) = op else {
        eprintln!("Fix: specify --op <name>. See --help.");
        process::exit(1);
    };

    let report = super::run_quick_check(&op);
    super::print_quick_report(&report);

    if !report.pass {
        process::exit(1);
    }
}
