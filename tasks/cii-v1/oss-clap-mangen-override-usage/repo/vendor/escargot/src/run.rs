use std::path;
use std::process;

use crate::error::{CargoError, CargoResult, ErrorKind};
use crate::format;
use crate::msg::CommandMessages;

/// The `run` subcommand (emulated).
///
/// Created via [`CargoBuild::run`][crate::CargoBuild::run].
///
/// Benefits over spawning `cargo run`:
/// - Able to cache binary path, avoiding cargo overhead.
/// - Independent of CWD.
/// - stdout/stderr are clean of `cargo run` output.
///
/// Relevant features
/// - `print` for logged output to be printed instead, generally for test writing.
///
/// # Example
///
/// To create a [`CargoRun`]:
/// ```rust
/// # let target_dir = tempfile::TempDir::new().unwrap();
/// let run = escargot::CargoBuild::new()
///     .bin("bin")
///     .current_release()
///     .current_target()
///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
///     .target_dir(target_dir.path())
///     .run()
///     .unwrap();
/// println!("artifact={}", run.path().display());
/// ```
/// See [`CargoRun::path`] for how to then run the newly compiled
/// program.
#[derive(Debug)]
pub struct CargoRun {
    bin_path: path::PathBuf,
}

impl CargoRun {
    pub(crate) fn from_message(
        msgs: CommandMessages,
        is_bin: bool,
        is_example: bool,
    ) -> CargoResult<Self> {
        let kind = match (is_bin, is_example) {
            (true, true) => {
                return Err(CargoError::new(ErrorKind::CommandFailed)
                    .set_context("Ambiguous which binary is intended, multiple selected"));
            }
            (false, true) => "example",
            _ => "bin",
        };
        let bin_path = extract_binary_path(msgs, kind)?;
        Ok(Self { bin_path })
    }

    /// Path to the specified binary.
    ///
    /// This is to support alternative ways of launching the binary besides [`Command`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run = escargot::CargoBuild::new()
    ///     .bin("bin")
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run()
    ///     .unwrap();
    /// println!("artifact={}", run.path().display());
    /// ```
    /// or
    /// ```rust,no_run
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run = escargot::CargoBuild::new()
    ///     .example("example_fixture")
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/example/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run()
    ///     .unwrap();
    /// println!("artifact={}", run.path().display());
    /// ```
    ///
    /// [`Command`]: std::process::Command
    pub fn path(&self) -> &path::Path {
        &self.bin_path
    }

    /// Run the build artifact.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run = escargot::CargoBuild::new()
    ///     .bin("bin")
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run()
    ///     .unwrap()
    ///     .command()
    ///     .arg("--help")
    ///     .status()
    ///     .unwrap();
    /// ```
    /// or
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run = escargot::CargoBuild::new()
    ///     .example("example_fixture")
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/example/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run()
    ///     .unwrap()
    ///     .command()
    ///     .arg("--help")
    ///     .status()
    ///     .unwrap();
    /// ```
    pub fn command(&self) -> process::Command {
        process::Command::new(self.path())
    }
}

fn extract_bin<'a>(msg: &'a format::Message<'_>, desired_kind: &str) -> Option<&'a path::Path> {
    match msg {
        format::Message::CompilerArtifact(art) => {
            if !art.profile.test
                && art.target.crate_types == ["bin"]
                && art.target.kind == [desired_kind]
            {
                Some(art.filenames.first().expect("files must exist"))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn transpose<T, E>(r: Result<Option<T>, E>) -> Option<Result<T, E>> {
    match r {
        Ok(Some(x)) => Some(Ok(x)),
        Ok(None) => None,
        Err(e) => Some(Err(e)),
    }
}

fn extract_binary_paths(
    msgs: CommandMessages,
    kind: &'static str,
) -> impl Iterator<Item = Result<path::PathBuf, CargoError>> {
    msgs.filter_map(move |m| {
        let m = m.and_then(|m| {
            let m = m.decode()?;
            format::log_message(&m);
            let p = extract_bin(&m, kind).map(|p| p.to_path_buf());
            Ok(p)
        });
        transpose(m)
    })
}

fn extract_binary_path(
    msgs: CommandMessages,
    kind: &'static str,
) -> Result<path::PathBuf, CargoError> {
    let bins: Result<Vec<_>, CargoError> = extract_binary_paths(msgs, kind).collect();
    let bins = bins?;
    if bins.is_empty() {
        return Err(CargoError::new(ErrorKind::CommandFailed).set_context("No binaries in crate"));
    } else if bins.len() != 1 {
        return Err(CargoError::new(ErrorKind::CommandFailed)
            .set_context(std::format!("Ambiguous which binary is intended: {bins:?}")));
    }
    Ok(bins.into_iter().next().expect("already validated"))
}
