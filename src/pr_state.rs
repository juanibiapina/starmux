use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const FRESH: u64 = 300;
const RETRY: u64 = 60;
const STALE: u64 = 1800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrState {
    Open,
    Draft,
    Merged,
    Closed,
    Unknown,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    url: String,
    version: u8,
    state: PrState,
    fetched: u64,
    retry: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn parse_url(url: &str) -> Option<(String, String)> {
    if url.len() > 512 {
        return None;
    }
    let path = url.strip_prefix("https://github.com/")?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 4
        || parts[2] != "pull"
        || parts[3].parse::<u64>().ok()? == 0
        || parts[3].starts_with('0')
        || parts[..2].iter().any(|p| {
            p.is_empty()
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
        })
    {
        return None;
    }
    Some((
        format!("{}/{}#{}", parts[0], parts[1], parts[3]),
        format!("repos/{}/{}/pulls/{}", parts[0], parts[1], parts[3]),
    ))
}

fn key(url: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in url.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
fn paths(dir: &Path, url: &str) -> (PathBuf, PathBuf) {
    let key = key(url);
    (
        dir.join(format!("{key}.json")),
        dir.join(format!("{key}.lock")),
    )
}
fn read(path: &Path, url: &str) -> Option<Cache> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(1025)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 {
        return None;
    }
    let cache: Cache = serde_json::from_slice(&bytes).ok()?;
    (cache.version == 1
        && cache.url == url
        && cache.fetched <= now()
        && cache.retry <= now().saturating_add(FRESH))
    .then_some(cache)
}

pub fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
            .join("starmux/pr-state"),
    )
}

pub fn resolve(url: &str, dir: &Path, spawn: bool) -> PrState {
    if parse_url(url).is_none() {
        return PrState::Unknown;
    }
    let (file, lock) = paths(dir, url);
    let cache = read(&file, url);
    if spawn
        && cache
            .as_ref()
            .is_none_or(|c| c.retry <= now() && now().saturating_sub(c.fetched) >= FRESH)
    {
        let _ = launch(url, dir, &lock);
    }
    cache
        .filter(|c| now().saturating_sub(c.fetched) <= STALE)
        .map_or(PrState::Unknown, |c| c.state)
}

fn launch(url: &str, dir: &Path, lock: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(lock)
    {
        Ok(mut file) => {
            file.write_all(now().to_string().as_bytes())
                .map_err(|e| e.to_string())?;
        }
        Err(_) => {
            let old = fs::read_to_string(lock)
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some_and(|t| now().saturating_sub(t) > 30);
            if !old {
                return Ok(());
            }
            let _ = fs::remove_file(lock);
            return Ok(());
        }
    }
    let result = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("pr-refresh")
        .arg(url)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if result.is_err() {
        let _ = fs::remove_file(lock);
    }
    result.map(|_| ()).map_err(|e| e.to_string())
}

pub fn refresh(url: &str, dir: &Path) -> Result<(), String> {
    let (_, endpoint) = parse_url(url).ok_or("invalid PR URL")?;
    let (file, lock) = paths(dir, url);
    if !lock.exists() {
        return Err("missing PR refresh lease".into());
    }
    let previous = read(&file, url);
    let result = fetch(&endpoint);
    let current = now();
    let cache = match result {
        Some(state) => Cache {
            url: url.into(),
            version: 1,
            state,
            fetched: current,
            retry: current + FRESH,
        },
        None => Cache {
            url: url.into(),
            version: 1,
            state: previous.as_ref().map_or(PrState::Unknown, |c| c.state),
            fetched: previous.map_or(0, |c| c.fetched),
            retry: current + RETRY,
        },
    };
    let temp = dir.join(format!("{}.{}.tmp", key(url), std::process::id()));
    let write = (|| -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        output
            .write_all(&serde_json::to_vec(&cache).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, file).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(&temp);
    let _ = fs::remove_file(lock);
    write
}

fn fetch(endpoint: &str) -> Option<PrState> {
    fetch_with("gh", endpoint)
}

fn fetch_with(command: &str, endpoint: &str) -> Option<PrState> {
    let mut child = Command::new(command)
        .args([
            "api",
            "--hostname",
            "github.com",
            endpoint,
            "--jq",
            "{state: .state, draft: .draft, merged_at: .merged_at}",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut bytes = Vec::new();
                child
                    .stdout
                    .take()?
                    .take(2049)
                    .read_to_end(&mut bytes)
                    .ok()?;
                if bytes.len() > 2048 {
                    return None;
                }
                return classify(&serde_json::from_slice(&bytes).ok()?);
            }
            Ok(None) if start.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn classify(value: &serde_json::Value) -> Option<PrState> {
    let state = value.get("state")?.as_str()?;
    let draft = value.get("draft")?.as_bool()?;
    let merged = value.get("merged_at")?;
    match state {
        "open" => Some(if draft { PrState::Draft } else { PrState::Open }),
        "closed" => Some(if merged.is_string() {
            PrState::Merged
        } else {
            PrState::Closed
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fetches_pr_state_from_a_local_command() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("starmux-pr-fetch-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("gh");
        fs::write(&script, "#!/bin/sh\nprintf '%s\\n' '{\"state\":\"closed\",\"draft\":false,\"merged_at\":\"2026-01-01\"}'\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            fetch_with(script.to_str().unwrap(), "repos/o/r/pulls/1"),
            Some(PrState::Merged)
        );
        fs::write(&script, "#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            fetch_with(script.to_str().unwrap(), "repos/o/r/pulls/1"),
            None
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cached_state_expires_and_is_bound_to_its_url() {
        let dir = std::env::temp_dir().join(format!("starmux-pr-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let url = "https://github.com/owner/repo/pull/42";
        let (path, _) = paths(&dir, url);
        let mut cache = Cache {
            url: url.into(),
            version: 1,
            state: PrState::Merged,
            fetched: now(),
            retry: now() + FRESH,
        };
        fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
        assert_eq!(resolve(url, &dir, false), PrState::Merged);
        cache.fetched = now() - STALE - 1;
        cache.retry = now() - 1;
        fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
        assert_eq!(resolve(url, &dir, false), PrState::Unknown);
        cache.url = "https://github.com/other/repo/pull/42".into();
        cache.fetched = now();
        fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
        assert_eq!(resolve(url, &dir, false), PrState::Unknown);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn classifies_pr_states() {
        for (json, expected) in [
            (
                r#"{"state":"open","draft":false,"merged_at":null}"#,
                PrState::Open,
            ),
            (
                r#"{"state":"open","draft":true,"merged_at":null}"#,
                PrState::Draft,
            ),
            (
                r#"{"state":"closed","draft":false,"merged_at":"2026-01-01"}"#,
                PrState::Merged,
            ),
            (
                r#"{"state":"closed","draft":false,"merged_at":null}"#,
                PrState::Closed,
            ),
        ] {
            assert_eq!(
                classify(&serde_json::from_str(json).unwrap()),
                Some(expected)
            );
        }
        assert!(parse_url("https://github.com/org/repo/pull/42").is_some());
        assert!(parse_url("https://github.com/org/repo/pull/42?x=1").is_none());
    }
}
