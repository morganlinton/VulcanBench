//! Serialization formats for cargo messages.

use std::borrow;
use std::path;

pub mod diagnostic;

#[cfg(feature = "test_unstable")]
pub mod test;

type CowPath<'a> = borrow::Cow<'a, path::Path>;
type CowStr<'a> = borrow::Cow<'a, str>;

/// A cargo message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
#[allow(clippy::large_enum_variant)]
pub enum Message<'a> {
    /// Build completed, all further output should not be parsed
    BuildFinished(BuildFinished),
    /// The compiler generated an artifact
    #[serde(borrow)]
    CompilerArtifact(Artifact<'a>),
    /// The compiler wants to display a message
    #[serde(borrow)]
    CompilerMessage(FromCompiler<'a>),
    /// A build script successfully executed.
    #[serde(borrow)]
    BuildScriptExecuted(BuildScript<'a>),
    #[cfg(not(feature = "strict_unstable"))]
    #[doc(hidden)]
    #[serde(other)]
    Unknown,
}

/// Build completed, all further output should not be parsed.
///
/// See <https://doc.rust-lang.org/cargo/reference/external-tools.html#build-finished>
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct BuildFinished {
    /// Whether or not the build finished successfully.
    pub success: bool,
}

/// A compiler-generated file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct Artifact<'a> {
    /// The workspace member this artifact belongs to
    #[serde(borrow)]
    pub package_id: WorkspaceMember<'a>,
    /// The full path to the artifact's manifest
    #[serde(borrow)]
    pub manifest_path: Option<CowPath<'a>>,
    /// The cargo target (lib, bin, example, etc.) that generated the artifacts.
    #[serde(borrow)]
    pub target: Target<'a>,
    /// The profile indicates which compiler settings were used.
    #[serde(borrow)]
    pub profile: ArtifactProfile<'a>,
    /// The enabled features for this artifact
    #[serde(borrow)]
    pub features: Vec<CowStr<'a>>,
    /// The full paths to the generated artifacts
    #[serde(borrow)]
    pub filenames: Vec<CowPath<'a>>,
    /// The path to the executable that was created
    ///
    /// `None` if this step did not generate an executable.
    #[serde(borrow)]
    #[serde(default)]
    pub executable: Option<CowPath<'a>>,
    /// Whether or not this step was actually executed.
    ///
    /// When `true`, this means that the pre-existing artifacts were
    /// up-to-date, and `rustc` was not executed. When `false`, this means that
    /// `rustc` was run to generate the artifacts.
    pub fresh: bool,
}

/// A single target (lib, bin, example, ...) provided by a crate
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct Target<'a> {
    /// Name as given in the `Cargo.toml` or generated from the file name
    #[serde(borrow)]
    pub name: CowStr<'a>,
    /// Kind of target ("bin", "example", "test", "bench", "lib")
    #[serde(borrow)]
    pub kind: Vec<CowStr<'a>>,
    /// Almost the same as `kind`, except when an example is a library instead of an executable.
    /// In that case `crate_types` contains things like `rlib` and `dylib` while `kind` is `example`
    #[serde(default)]
    #[serde(borrow)]
    pub crate_types: Vec<CowStr<'a>>,
    /// Whether this is a doctest or not
    #[serde(default)]
    pub doctest: Option<bool>,
    /// Whether this is documentation or not
    #[serde(default)]
    pub doc: Option<bool>,
    /// Whether this is a test file
    #[serde(default)]
    pub test: bool,

    #[serde(default)]
    #[serde(rename = "required-features")]
    /// This target is built only if these features are enabled.
    /// It doesn't apply to `lib` targets.
    #[serde(borrow)]
    pub required_features: Vec<CowStr<'a>>,
    /// Path to the main source file of the target
    #[serde(borrow)]
    pub src_path: CowPath<'a>,
    /// Rust edition for this target
    #[serde(default = "edition_default")]
    #[serde(borrow)]
    pub edition: CowStr<'a>,
}

fn edition_default() -> CowStr<'static> {
    "2015".into()
}

/// A workspace member. This is basically identical to `cargo::core::package_id::PackageId`, except
/// that this does not use `Arc` internally.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
pub struct WorkspaceMember<'a> {
    /// The raw package id as given by cargo
    #[serde(borrow)]
    raw: CowStr<'a>,
}

/// Profile settings used to determine which compiler flags to use for a
/// target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct ArtifactProfile<'a> {
    /// Optimization level. Possible values are 0-3, s or z.
    #[serde(borrow)]
    pub opt_level: CowStr<'a>,
    /// The amount of debug info.
    pub debuginfo: Option<DebugInfo<'a>>,
    /// State of the `cfg(debug_assertions)` directive, enabling macros like
    /// `debug_assert!`
    pub debug_assertions: bool,
    /// State of the overflow checks.
    pub overflow_checks: bool,
    /// Whether this profile is a test
    pub test: bool,
}

/// The amount of debug info. 0 for none, 1 for limited, 2 for full
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[serde(untagged)]
#[non_exhaustive]
pub enum DebugInfo<'a> {
    /// 0 for none, 1 for limited, 2 for full
    Level(u32),
    /// none, limited, full, etc
    #[serde(borrow)]
    Name(CowStr<'a>),
}

/// Message left by the compiler
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct FromCompiler<'a> {
    /// The workspace member this message belongs to
    #[serde(borrow)]
    pub package_id: WorkspaceMember<'a>,
    /// The full path to the artifact's manifest
    #[serde(borrow)]
    pub manifest_path: Option<CowPath<'a>>,
    /// The target this message is aimed at
    #[serde(borrow)]
    pub target: Target<'a>,
    /// The message the compiler sent.
    #[serde(borrow)]
    pub message: diagnostic::Diagnostic<'a>,
}

/// Output of a Build Script execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "strict_unstable", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct BuildScript<'a> {
    /// The workspace member this build script execution belongs to
    #[serde(borrow)]
    pub package_id: WorkspaceMember<'a>,
    /// The outdir used.
    #[serde(borrow)]
    #[serde(default)]
    pub out_dir: Option<CowPath<'a>>,
    /// The libs to link
    #[serde(borrow)]
    pub linked_libs: Vec<CowStr<'a>>,
    /// The paths to search when resolving libs
    #[serde(borrow)]
    pub linked_paths: Vec<CowPath<'a>>,
    /// The paths to search when resolving libs
    #[serde(borrow)]
    pub cfgs: Vec<CowPath<'a>>,
    /// The environment variables to add to the compilation
    #[serde(borrow)]
    pub env: Vec<(CowStr<'a>, CowStr<'a>)>,
}

#[cfg(not(feature = "print"))]
pub(crate) fn log_message(msg: &Message<'_>) {
    match msg {
        Message::BuildFinished(ref finished) => {
            log::trace!("Build Finished: {:?}", finished.success);
        }
        Message::CompilerArtifact(ref art) => {
            log::trace!("Building {:#?}", art.package_id,);
        }
        Message::CompilerMessage(ref comp) => {
            let content = comp
                .message
                .rendered
                .as_ref()
                .map(|s| s.as_ref())
                .unwrap_or_else(|| comp.message.message.as_ref());
            match comp.message.level {
                diagnostic::DiagnosticLevel::Ice => log::error!("{}", content),
                diagnostic::DiagnosticLevel::Error => log::error!("{}", content),
                diagnostic::DiagnosticLevel::Warning => log::warn!("{}", content),
                diagnostic::DiagnosticLevel::Note => log::info!("{}", content),
                diagnostic::DiagnosticLevel::Help => log::info!("{}", content),
                #[cfg(not(feature = "strict_unstable"))]
                _ => log::warn!("Unknown message: {:#?}", msg),
            }
        }
        Message::BuildScriptExecuted(ref script) => {
            log::trace!("Ran script from {:#?}", script.package_id);
        }
        #[cfg(not(feature = "strict_unstable"))]
        _ => {
            log::warn!("Unknown message: {:#?}", msg);
        }
    }
}

#[cfg(feature = "print")]
#[allow(clippy::print_stderr)]
pub(crate) fn log_message(msg: &Message<'_>) {
    match msg {
        Message::BuildFinished(ref finished) => {
            eprintln!("Build Finished: {:?}", finished.success);
        }
        Message::CompilerArtifact(ref art) => {
            eprintln!("Building {:#?}", art.package_id,);
        }
        Message::CompilerMessage(ref comp) => {
            let content = comp
                .message
                .rendered
                .as_ref()
                .map(|s| s.as_ref())
                .unwrap_or_else(|| comp.message.message.as_ref());
            match comp.message.level {
                diagnostic::DiagnosticLevel::Ice => eprintln!("{content}"),
                diagnostic::DiagnosticLevel::Error => eprintln!("{content}"),
                diagnostic::DiagnosticLevel::Warning => eprintln!("{content}"),
                diagnostic::DiagnosticLevel::Note => eprintln!("{content}"),
                diagnostic::DiagnosticLevel::Help => eprintln!("{content}"),
                #[cfg(not(feature = "strict_unstable"))]
                _ => eprintln!("Unknown message: {:#?}", msg),
            }
        }
        Message::BuildScriptExecuted(ref script) => {
            eprintln!("Ran script from {:#?}", script.package_id);
        }
        #[cfg(not(feature = "strict_unstable"))]
        _ => {
            eprintln!("Unknown message: {:#?}", msg);
        }
    }
}
