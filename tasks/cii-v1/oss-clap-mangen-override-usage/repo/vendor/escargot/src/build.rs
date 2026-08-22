use std::ffi::OsStr;
use std::process;

use crate::cargo::Cargo;
use crate::cargo::CURRENT_TARGET;
use crate::error::CargoResult;
use crate::msg::CommandMessages;
use crate::run::CargoRun;
#[cfg(feature = "test_unstable")]
use crate::test::CargoTest;

/// The `build` subcommand.
///
/// # Example
///
/// ```rust
/// # let target_dir = tempfile::TempDir::new().unwrap();
/// escargot::CargoBuild::new()
///     .bin("bin")
///     .current_release()
///     .current_target()
///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
///     .target_dir(target_dir.path())
///     .exec()
///     .unwrap();
/// ```
#[derive(Debug)]
pub struct CargoBuild {
    cmd: process::Command,
    bin: bool,
    example: bool,
}

impl CargoBuild {
    /// Shortcut to create a `build` subcommand.
    ///
    /// See also [`Cargo`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .bin("bin")
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    ///
    pub fn new() -> Self {
        Cargo::new().build()
    }

    pub(crate) fn with_command(cmd: process::Command) -> Self {
        Self {
            cmd,
            bin: false,
            example: false,
        }
    }

    /// Return the underlying [`process::Command`]
    pub fn into_command(self) -> process::Command {
        self.cmd
    }

    /// Build from `name` package in workspaces.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .package("bin")
    ///     .bin("bin")
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn package<S: AsRef<OsStr>>(self, name: S) -> Self {
        self.arg("--package").arg(name)
    }

    /// Build all binaries.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .bins()
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn bins(mut self) -> Self {
        self.bin = true;
        self.arg("--bins")
    }

    /// Build only `name` binary.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .bin("bin")
    ///     .manifest_path("tests/testsuite/fixtures/bin/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn bin<S: AsRef<OsStr>>(mut self, name: S) -> Self {
        self.bin = true;
        self.arg("--bin").arg(name)
    }

    /// Build all examples
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .examples()
    ///     .manifest_path("tests/testsuite/fixtures/example/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn examples(mut self) -> Self {
        self.example = true;
        self.arg("--examples")
    }

    /// Build only `name` example.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .example("example_fixture")
    ///     .manifest_path("tests/testsuite/fixtures/example/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn example<S: AsRef<OsStr>>(mut self, name: S) -> Self {
        self.example = true;
        self.arg("--example").arg(name)
    }

    /// Build all tests
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .tests()
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn tests(self) -> Self {
        self.arg("--tests")
    }

    /// Build only `name` test.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// escargot::CargoBuild::new()
    ///     .test("test")
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .exec()
    ///     .unwrap();
    /// ```
    pub fn test<S: AsRef<OsStr>>(self, name: S) -> Self {
        self.arg("--test").arg(name)
    }

    /// Path to Cargo.toml
    pub fn manifest_path<S: AsRef<OsStr>>(self, path: S) -> Self {
        self.arg("--manifest-path").arg(path)
    }

    /// Build artifacts in release mode, with optimizations.
    pub fn release(self) -> Self {
        self.arg("--release")
    }

    /// Inserts or updates an environment variable mapping.
    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.cmd.env(key, val);

        self
    }

    /// Removes an environment variable
    pub fn env_remove<K>(mut self, key: K) -> Self
    where
        K: AsRef<OsStr>,
    {
        self.cmd.env_remove(key);
        self
    }

    /// Infer [`Self::release`] from how the current process was built
    pub fn current_release(self) -> Self {
        #[cfg(debug_assertions)]
        {
            self
        }
        #[cfg(not(debug_assertions))]
        {
            self.release()
        }
    }

    /// Build for the target triplet.
    pub fn target<S: AsRef<OsStr>>(self, triplet: S) -> Self {
        self.arg("--target").arg(triplet)
    }

    /// Infer [`Self::target`] from how the current process was built
    pub fn current_target(self) -> Self {
        self.target(CURRENT_TARGET)
    }

    /// Directory for all generated artifacts
    pub fn target_dir<S: AsRef<OsStr>>(self, dir: S) -> Self {
        self.arg("--target-dir").arg(dir)
    }

    /// Activate all available features
    pub fn all_features(self) -> Self {
        self.arg("--all-features")
    }

    /// Do not activate the `default` feature
    pub fn no_default_features(self) -> Self {
        self.arg("--no-default-features")
    }

    /// Space-separated list of features to activate
    pub fn features<S: AsRef<OsStr>>(self, features: S) -> Self {
        self.arg("--features").arg(features)
    }

    /// Manually pass an argument that is unsupported.
    ///
    /// Caution: Passing in `--` can throw off the API.
    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.cmd.arg(arg);
        self
    }

    /// Manually pass arguments that are unsupported.
    ///
    /// Caution: Passing in `--` can throw off the API.
    pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, args: I) -> Self {
        self.cmd.args(args);
        self
    }

    /// Build the configured target, returning compiler messages.
    pub fn exec(self) -> CargoResult<CommandMessages> {
        CommandMessages::with_command(self.cmd)
    }

    /// Provide a proxy for running the built target.
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
    pub fn run(self) -> CargoResult<CargoRun> {
        let msgs = CommandMessages::with_command(self.cmd)?;
        CargoRun::from_message(msgs, self.bin, self.example)
    }

    /// Provide a proxy for running the built target.
    ///
    /// Required feature: `test_unstable` since the format parsed is unstable.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let target_dir = tempfile::TempDir::new().unwrap();
    /// let run = escargot::CargoBuild::new()
    ///     .test("test")
    ///     .current_release()
    ///     .current_target()
    ///     .manifest_path("tests/testsuite/fixtures/test/Cargo.toml")
    ///     .target_dir(target_dir.path())
    ///     .run_tests().unwrap()
    ///     .next().unwrap().unwrap();
    /// println!("artifact={}", run.path().display());
    /// ```
    #[cfg(feature = "test_unstable")]
    pub fn run_tests(self) -> CargoResult<impl Iterator<Item = CargoResult<CargoTest>>> {
        let msgs = CommandMessages::with_command(self.cmd)?;
        Ok(CargoTest::with_messages(msgs))
    }
}

impl Default for CargoBuild {
    fn default() -> Self {
        Self::new()
    }
}
