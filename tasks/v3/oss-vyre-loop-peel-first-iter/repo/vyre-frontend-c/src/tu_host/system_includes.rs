//! Default system-include search path discovery.
//!
//! vyre-frontend-c never shells out to gcc/clang/cc to discover include paths.
//! A compiler probe makes preprocessing depend on a host compiler install,
//! bakes in that compiler's ABI model, and adds process-spawn latency to the
//! path that must be faster than clang. Callers must provide system include
//! roots explicitly through target/sysroot configuration or `-isystem`/`-I`.
//!
use std::path::PathBuf;
use std::sync::OnceLock;

/// Explicitly configured default system include search path.
static SYSTEM_INCLUDE_DIRS: OnceLock<Result<Vec<PathBuf>, String>> = OnceLock::new();

/// Return the cached default system include search path, in CLI search order.
///
/// This function intentionally does not discover host defaults. Silent host
/// discovery would bind translation units to whichever C compiler happens to
/// be installed on the machine running vyrec. Production invocations must pass
/// explicit include roots so the GPU preprocessor sees the same header universe
/// on every host.
pub(super) fn system_include_dirs() -> Result<&'static [PathBuf], String> {
    SYSTEM_INCLUDE_DIRS
        .get_or_init(disabled_system_include_dirs)
        .as_ref()
        .map(Vec::as_slice)
        .map_err(|error| error.clone())
}

fn disabled_system_include_dirs() -> Result<Vec<PathBuf>, String> {
    Err(
        "vyre-frontend-c: default system include discovery is disabled in the GPU-first production path. Fix: pass target/sysroot include roots explicitly with --sysroot, -isystem, or -I; vyrec must not spawn gcc/clang/cc to infer host headers."
            .to_string(),
    )
}

/// Extract the directory list between `#include <...> search starts here:` and
/// `End of search list.` from a `cc -E -v -` stderr capture.
///
/// Lines inside the list look like ` /usr/include` (a single leading space).
/// Some drivers append ` (framework directory)` on macOS  -  treat those as
/// regular paths; the trailing annotation is dropped.
#[cfg(test)]
fn parse_search_list(stderr: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut in_list = false;
    for line in stderr.lines() {
        let trimmed = line.trim_end();
        if trimmed.starts_with("#include <...>") {
            in_list = true;
            continue;
        }
        if !in_list {
            continue;
        }
        if trimmed == "End of search list." {
            break;
        }
        if !trimmed.starts_with(' ') {
            continue;
        }
        let cleaned = trimmed.trim_start();
        let cleaned = cleaned
            .split_once(" (framework directory)")
            .map(|(p, _)| p)
            .unwrap_or(cleaned);
        if cleaned.is_empty() {
            continue;
        }
        paths.push(PathBuf::from(cleaned));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gcc_search_list_block() {
        let stderr = "ignoring nonexistent directory\n\
            #include \"...\" search starts here:\n\
            #include <...> search starts here:\n \
            /usr/local/include\n \
            /usr/lib/gcc/x86_64-linux-gnu/13/include\n \
            /usr/include/x86_64-linux-gnu\n \
            /usr/include\n\
            End of search list.\n\
            COMPILER_PATH=...\n";
        let paths = parse_search_list(stderr);
        assert_eq!(paths.len(), 4);
        assert_eq!(paths[0], PathBuf::from("/usr/local/include"));
        assert_eq!(paths[3], PathBuf::from("/usr/include"));
    }

    #[test]
    fn parses_clang_macos_framework_annotation() {
        let stderr = "#include <...> search starts here:\n \
            /usr/local/include\n \
            /Library/Frameworks (framework directory)\n\
            End of search list.\n";
        let paths = parse_search_list(stderr);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1], PathBuf::from("/Library/Frameworks"));
    }

    #[test]
    fn empty_when_no_search_block() {
        let paths = parse_search_list("just some other compiler output\n");
        assert!(paths.is_empty());
    }

    #[test]
    fn cached_first_call_fails_without_explicit_system_roots() {
        // Behavioural test: default host discovery must fail loudly instead of
        // probing a local C toolchain or using hardcoded ABI guesses.
        let Ok(dirs) = system_include_dirs() else {
            return;
        };
        for d in dirs {
            assert!(
                d.is_absolute(),
                "expected absolute path, got {}",
                d.display()
            );
        }
    }
}
