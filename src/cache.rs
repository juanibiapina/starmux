use std::path::{Path, PathBuf};

pub(crate) fn root(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(path).join("starmux"));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        Some(home.join("Library/Caches/starmux"))
    } else {
        Some(home.join(".cache/starmux"))
    }
}

pub(crate) fn child(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

pub(crate) fn default_child(name: &str) -> Option<PathBuf> {
    Some(child(&root(None)?, name))
}
