use std::path;
use std::process;

use crate::error::{CargoError, CargoResult};
use crate::format;
use crate::msg::CommandMessages;

/// The `test` subcommand (emulated).
///
/// Created via [`CargoBuild::run_tests`].
///
/// Benefits over spawning `cargo test`:
/// - Able to cache binary path, avoiding cargo overhead.
/// - Independent of CWD.
/// - stdout/stderr are clean of `cargo test` output.
///
/// Required feature: `test_unstable` since the format parsed is unstable.
///
/// Relevant features
/// - `print` for logged output to be printed instead, generally for test writing.
///
/// # Example
///
/// ```rust
/// # let target_dir = tempfile::TempDir::new().unwrap();
/// let run = escargot::CargoBuild::new()
///     .test("test")
///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
///     .target_dir(target_dir.path())
///     .run_tests().unwrap()
///     .next().unwrap().unwrap();
/// println!("artifact={}", run.path().display());
/// ```
///
/// [`CargoBuild::run_tests`]: crate::CargoBuild::run_tests()
#[derive(Debug)]
pub struct CargoTest {
    bin_path: path::PathBuf,
    kind: String,
    name: String,
}

impl CargoTest {
    pub(crate) fn with_messages(
        msgs: CommandMessages,
    ) -> impl Iterator<Item = Result<Self, CargoError>> {
        extract_binary_paths(msgs)
    }

    /// The `name` of test
    ///
    /// Used to offer filtering or displays.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run: Result<Vec<_>, _> = escargot::CargoBuild::new()
    ///     .tests()
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run_tests()
    ///     .unwrap()
    ///     .collect();
    /// let run = run.unwrap();
    /// let mut names: Vec<_> = run.iter().map(|r| r.name()).collect();
    /// names.sort_unstable();
    /// assert_eq!(names, ["test", "test_fixture", "test_fixture"]);
    /// ```
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// The `kind` of test
    ///
    /// Used to distinguish between integration tests (`test`) and unit tests (`bin`, `lib`).
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run: Result<Vec<_>, _> = escargot::CargoBuild::new()
    ///     .tests()
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run_tests()
    ///     .unwrap()
    ///     .collect();
    /// let run = run.unwrap();
    /// let mut kinds: Vec<_> = run.iter().map(|r| r.kind()).collect();
    /// kinds.sort_unstable();
    /// assert_eq!(kinds, ["bin", "lib", "test"]);
    /// ```
    pub fn kind(&self) -> &str {
        self.kind.as_str()
    }

    /// Path to the specified binary.
    ///
    /// This is to support alternative ways of launching the binary besides [`Command`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run: Vec<_> = escargot::CargoBuild::new()
    ///     .tests()
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run_tests()
    ///     .unwrap()
    ///     .collect();
    /// assert_eq!(run.len(), 3);
    /// ```
    ///
    /// [`Command`]: std::process::Command
    pub fn path(&self) -> &path::Path {
        &self.bin_path
    }

    /// Run the build artifact.
    pub fn command(&self) -> process::Command {
        let mut cmd = process::Command::new(self.path());
        cmd.arg("-Z").arg("unstable-options").arg("--format=json");
        cmd
    }

    /// Run the configured test, returning test events.
    pub fn exec(&self) -> CargoResult<CommandMessages> {
        CommandMessages::with_command(self.command())
    }
}

fn extract_bin(msg: &format::Message<'_>) -> Option<CargoTest> {
    match msg {
        format::Message::CompilerArtifact(art) => {
            if art.profile.test {
                let bin_path = art
                    .filenames
                    .first()
                    .expect("files must exist")
                    .to_path_buf();
                let kind = art
                    .target
                    .kind
                    .first()
                    .expect("kind must exist")
                    .as_ref()
                    .to_owned();
                let name = art.target.name.as_ref().to_owned();
                Some(CargoTest {
                    bin_path,
                    kind,
                    name,
                })
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
) -> impl Iterator<Item = Result<CargoTest, CargoError>> {
    msgs.filter_map(move |m| {
        let m = m.and_then(|m| {
            let m = m.decode()?;
            format::log_message(&m);
            let p = extract_bin(&m);
            Ok(p)
        });
        transpose(m)
    })
}
