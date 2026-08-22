# clap_mangen: SYNOPSIS ignores Command::override_usage

A command that sets `Command::override_usage` gets a man page whose SYNOPSIS
is still derived from the declared arguments. The help output honors the
override; the man page contradicts it.

```rust
let cmd = clap::Command::new("my-app")
    .about("Check file types and compare values")
    .override_usage("my-app [OPTION]... EXPRESSION")
    .arg(clap::Arg::new("all").short('a').long("all").action(clap::ArgAction::SetTrue));

let mut buf = Vec::new();
clap_mangen::Man::new(cmd).render(&mut buf).unwrap();
// SYNOPSIS shows the derived "my-app [-a]" form, not the override.
```

The SYNOPSIS should render the overridden usage instead of the derived one:

- An override may document several invocation forms, one per line (indented
  to line up under help's `Usage: ` prefix). Each non-empty form gets a line
  of its own in the SYNOPSIS rather than being collapsed; empty lines are
  dropped.
- A form starting with the command name gets the name set in bold, matching
  the derived synopsis style; other forms (e.g. `./my-app ...`) are rendered
  as plain text.
- Commands without an override keep today's derived SYNOPSIS, and the help
  output's handling of `override_usage` must not change.

`clap_builder` currently only exposes the overridden usage internally — the
man-page generator lives in a separate crate and needs a way to read it.
