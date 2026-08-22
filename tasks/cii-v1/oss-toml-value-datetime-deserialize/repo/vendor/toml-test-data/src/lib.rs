//! Test cases from the [toml-test](https://github.com/toml-lang/toml-test) conformance suite
//!
//! To read and write these test cases, see [`toml-test`](https://docs.rs/toml-test).
//!
//! To run the test cases against your TOML implementation, see [`toml-test-harness`](https://docs.rs/toml-test-harness).

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
#![allow(clippy::self_named_module_files)] // `include_dir!`?

use std::borrow::Cow;

const TESTS_DIR: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets/toml-test/tests");

/// Get the test cases for a given spec version
pub fn version(ver: &str) -> impl Iterator<Item = &'static std::path::Path> {
    TESTS_DIR
        .get_file(format!("files-toml-{ver}"))
        .and_then(|f| std::str::from_utf8(f.contents()).ok())
        .into_iter()
        .flat_map(|f| f.lines())
        .map(std::path::Path::new)
}

/// Get all supported spec versions and their test cases
pub fn versions() -> std::collections::HashMap<&'static str, Vec<&'static std::path::Path>> {
    TESTS_DIR
        .files()
        .filter_map(|f| {
            let name = f.path().file_name()?;
            let version = name.to_str()?.strip_prefix("files-toml-")?;
            let paths = std::str::from_utf8(f.contents())
                .ok()?
                .lines()
                .map(std::path::Path::new)
                .collect::<Vec<_>>();
            Some((version, paths))
        })
        .collect()
}

/// Valid TOML test case
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Valid<'a> {
    pub name: Cow<'a, std::path::Path>,
    pub fixture: Cow<'a, [u8]>,
    pub expected: Cow<'a, [u8]>,
}

impl<'a> Valid<'a> {
    pub fn borrow<'b: 'a>(&'b self) -> Valid<'b> {
        Self {
            name: Cow::Borrowed(self.name()),
            fixture: Cow::Borrowed(self.fixture()),
            expected: Cow::Borrowed(self.expected()),
        }
    }

    pub fn name(&self) -> &std::path::Path {
        self.name.as_ref()
    }

    pub fn fixture(&self) -> &[u8] {
        self.fixture.as_ref()
    }

    pub fn expected(&self) -> &[u8] {
        self.expected.as_ref()
    }
}

/// Returns all [`Valid`] TOML test cases
pub fn valid() -> impl Iterator<Item = Valid<'static>> {
    let valid_dir = TESTS_DIR.get_dir("valid").unwrap();
    valid_files(valid_dir).chain(valid_dir.dirs().flat_map(|d| {
        assert_eq!(d.dirs().count(), 0);
        valid_files(d)
    }))
}

fn valid_files<'d>(
    dir: &'d include_dir::Dir<'static>,
) -> impl Iterator<Item = Valid<'static>> + 'd {
    dir.files()
        .filter(|f| f.path().extension().unwrap_or_default() == "toml")
        .map(move |f| {
            let t = f;
            let j = dir
                .files()
                .find(|f| {
                    f.path().parent() == t.path().parent()
                        && f.path().file_stem() == t.path().file_stem()
                        && f.path().extension().unwrap() == "json"
                })
                .unwrap();
            let name = Cow::Borrowed(t.path());
            let fixture = Cow::Borrowed(t.contents());
            let expected = Cow::Borrowed(j.contents());
            Valid {
                name,
                fixture,
                expected,
            }
        })
}

/// Invalid TOML test case
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid<'a> {
    pub name: Cow<'a, std::path::Path>,
    pub fixture: Cow<'a, [u8]>,
}

impl<'a> Invalid<'a> {
    pub fn borrow<'b: 'a>(&'b self) -> Invalid<'b> {
        Self {
            name: Cow::Borrowed(self.name()),
            fixture: Cow::Borrowed(self.fixture()),
        }
    }

    pub fn name(&self) -> &std::path::Path {
        self.name.as_ref()
    }

    pub fn fixture(&self) -> &[u8] {
        self.fixture.as_ref()
    }
}

/// Returns all [`Invalid`] TOML test cases
pub fn invalid() -> impl Iterator<Item = Invalid<'static>> {
    let invalid_dir = TESTS_DIR.get_dir("invalid").unwrap();
    assert_eq!(invalid_dir.files().count(), 0);
    invalid_dir.dirs().flat_map(|d| {
        assert_eq!(d.dirs().count(), 0);
        d.files().map(|f| {
            let t = f;
            let name = Cow::Borrowed(f.path());
            let fixture = Cow::Borrowed(t.contents());
            Invalid { name, fixture }
        })
    })
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
