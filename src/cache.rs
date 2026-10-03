use std::path::{Path, PathBuf};

pub(crate) fn root(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured {
        return Some(PathBuf::from(path));
    }
    dirs::cache_dir().map(|root| root.join("starmux"))
}

pub(crate) fn child(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

pub(crate) fn default_child(name: &str) -> Option<PathBuf> {
    Some(child(&root(None)?, name))
}
