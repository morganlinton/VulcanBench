use super::*;
pub(super) fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[derive(Clone)]
pub(super) struct IncludeSearchDirs {
    pub(super) quote_dirs: Vec<PathBuf>,
    pub(super) user_dirs: Vec<PathBuf>,
    pub(super) system_dirs: Vec<PathBuf>,
    pub(super) after_dirs: Vec<PathBuf>,
    pub(super) include_next_dirs: Vec<PathBuf>,
}

pub(super) fn expand_explicit_include_dirs(include_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::with_capacity(include_dirs.len().saturating_mul(6));
    for dir in include_dirs {
        push_unique_path(&mut dirs, dir.clone());
        push_unique_path(&mut dirs, dir.join("generated"));
        push_unique_path(&mut dirs, dir.join("uapi"));
        push_unique_path(&mut dirs, dir.join("generated/uapi"));
        if let Some(parent) = dir.parent() {
            push_unique_path(&mut dirs, parent.join("include"));
            push_unique_path(&mut dirs, parent.join("generated"));
        }
    }
    dirs
}

pub(super) fn push_all_unique(paths: &mut Vec<PathBuf>, source: &[PathBuf]) {
    for path in source {
        push_unique_path(paths, path.clone());
    }
}

pub(super) fn expanded_include_search_dirs(
    include_dirs: &[PathBuf],
    quote_include_dirs: &[PathBuf],
    explicit_system_include_dirs: &[PathBuf],
    after_include_dirs: &[PathBuf],
    use_system_include_dirs: bool,
    system_include_sysroot: Option<&Path>,
) -> Result<IncludeSearchDirs, String> {
    let quote_dirs = expand_explicit_include_dirs(quote_include_dirs);
    let user_dirs = expand_explicit_include_dirs(include_dirs);
    let mut system_dirs = expand_explicit_include_dirs(explicit_system_include_dirs);
    if use_system_include_dirs {
        if let Some(sysroot) = system_include_sysroot {
            push_unique_path(&mut system_dirs, sysroot.join("include"));
            push_unique_path(&mut system_dirs, sysroot.join("usr/include"));
        }
        if system_dirs.is_empty() {
            for dir in system_include_dirs()? {
                push_unique_path(
                    &mut system_dirs,
                    sysrooted_system_include_dir(dir, system_include_sysroot)?,
                );
            }
        }
    }
    let after_dirs = expand_explicit_include_dirs(after_include_dirs);
    let include_next_capacity = quote_dirs
        .len()
        .checked_add(user_dirs.len())
        .and_then(|count| count.checked_add(system_dirs.len()))
        .and_then(|count| count.checked_add(after_dirs.len()))
        .ok_or_else(|| {
            "vyre-frontend-c include-next directory capacity overflows usize. Fix: reduce include roots before preprocessing."
                .to_string()
        })?;
    let mut include_next_dirs = Vec::with_capacity(include_next_capacity);
    push_all_unique(&mut include_next_dirs, &quote_dirs);
    push_all_unique(&mut include_next_dirs, &user_dirs);
    push_all_unique(&mut include_next_dirs, &system_dirs);
    push_all_unique(&mut include_next_dirs, &after_dirs);
    Ok(IncludeSearchDirs {
        quote_dirs,
        user_dirs,
        system_dirs,
        after_dirs,
        include_next_dirs,
    })
}

pub(super) fn sysrooted_system_include_dir(
    dir: &Path,
    sysroot: Option<&Path>,
) -> Result<PathBuf, String> {
    let Some(sysroot) = sysroot else {
        return Ok(dir.to_path_buf());
    };
    if !dir.is_absolute() {
        return Err(format!(
            "vyre-frontend-c: compiler system include path `{}` is not absolute and cannot be relocated through sysroot `{}`. Fix: repair the compiler include probe output.",
            dir.display(),
            sysroot.display()
        ));
    }
    let relative = dir.strip_prefix("/").map_err(|error| {
        format!(
            "vyre-frontend-c: failed to strip root from system include path `{}` for sysroot `{}`: {error}. Fix: repair the compiler include probe output.",
            dir.display(),
            sysroot.display()
        )
    })?;
    Ok(sysroot.join(relative))
}

pub(super) fn search_include_file(
    name: &str,
    tu_dir: &Path,
    include_dirs: &IncludeSearchDirs,
) -> Option<PathBuf> {
    let rel = tu_dir.join(name);
    if rel.is_file() {
        return Some(rel);
    }
    for d in include_dirs
        .quote_dirs
        .iter()
        .chain(include_dirs.user_dirs.iter())
        .chain(include_dirs.system_dirs.iter())
        .chain(include_dirs.after_dirs.iter())
    {
        let p = d.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    search_asm_generic_compat_include(name, &include_dirs.include_next_dirs)
}

pub(super) fn search_system_include_file(
    name: &str,
    include_dirs: &IncludeSearchDirs,
) -> Option<PathBuf> {
    for d in include_dirs
        .user_dirs
        .iter()
        .chain(include_dirs.system_dirs.iter())
        .chain(include_dirs.after_dirs.iter())
    {
        let p = d.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut asm_dirs = Vec::new();
    push_all_unique(&mut asm_dirs, &include_dirs.user_dirs);
    push_all_unique(&mut asm_dirs, &include_dirs.system_dirs);
    push_all_unique(&mut asm_dirs, &include_dirs.after_dirs);
    search_asm_generic_compat_include(name, &asm_dirs)
}

pub(super) fn search_include_next_file(
    name: &str,
    from: &Path,
    include_dirs: &IncludeSearchDirs,
) -> Result<Option<PathBuf>, String> {
    let from_dir = from.parent().unwrap_or_else(|| Path::new("."));
    let mut start_after = 0usize;
    for (idx, dir) in include_dirs.include_next_dirs.iter().enumerate() {
        if paths_equivalent(dir, from_dir)? {
            start_after = idx.saturating_add(1);
            break;
        }
    }
    for d in include_dirs.include_next_dirs.iter().skip(start_after) {
        let p = d.join(name);
        if p.is_file() {
            return Ok(Some(p));
        }
    }
    Ok(search_asm_generic_compat_include(
        name,
        &include_dirs.include_next_dirs[start_after.min(include_dirs.include_next_dirs.len())..],
    ))
}

pub(super) fn paths_equivalent(a: &Path, b: &Path) -> Result<bool, String> {
    if a == b {
        return Ok(true);
    }
    let a = fs::canonicalize(a).map_err(|error| {
        format!(
            "vyre-frontend-c: failed to canonicalize include directory `{}` for include-next ordering: {error}. Fix: remove stale include roots or repair permissions.",
            a.display()
        )
    })?;
    let b = fs::canonicalize(b).map_err(|error| {
        format!(
            "vyre-frontend-c: failed to canonicalize including directory `{}` for include-next ordering: {error}. Fix: repair the including file path or include root.",
            b.display()
        )
    })?;
    Ok(a == b)
}

pub(super) fn search_asm_generic_compat_include(
    name: &str,
    include_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let generic = name.strip_prefix("asm/")?;
    let generic_name = Path::new("asm-generic").join(generic);
    include_dirs
        .iter()
        .map(|d| d.join(&generic_name))
        .find(|p| p.is_file())
}
