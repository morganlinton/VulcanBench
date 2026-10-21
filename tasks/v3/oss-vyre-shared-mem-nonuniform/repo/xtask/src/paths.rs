//! Workspace-rooted paths for xtask.

pub(crate) mod quick_cache_dir;
pub(crate) mod workspace_root;

pub(crate) use quick_cache_dir::quick_cache_dir;
pub(crate) use workspace_root::workspace_root;
