mod providers;

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const FRESH_MS: u64 = 60_000;
const STALE_MS: u64 = 30 * 60_000;
const BACKOFF_MS: u64 = 60_000;
const LEASE_MS: u64 = 30_000;
const MAX_STATE_BYTES: u64 = 32 * 1024;
const MAX_WINDOWS: usize = 16;
const MAX_AVAILABLE_RESETS: u32 = 9999;

pub(crate) const PROVIDERS: &[&str] = &[
    "anthropic",
    "copilot",
    "gemini",
    "antigravity",
    "codex",
    "kiro",
    "zai",
    "xai",
];

// Only fixed provider keys can become click tokens or browser destinations.
const USAGE_PAGES: &[(&str, &str, &str)] = &[
    ("anthropic", "su0", "https://claude.ai/settings/usage"),
    (
        "copilot",
        "su1",
        "https://github.com/settings/billing/premium_requests_usage",
    ),
    ("codex", "su4", "https://chatgpt.com/settings/usage"),
    (
        "zai",
        "su6",
        "https://z.ai/manage-apikey/coding-plan/personal/usage",
    ),
];

pub(crate) fn page_token(provider: &str) -> Option<&'static str> {
    USAGE_PAGES
        .iter()
        .find(|(name, _, _)| *name == provider)
        .map(|(_, token, _)| *token)
}

pub(crate) fn page_url(token: &str) -> Option<&'static str> {
    USAGE_PAGES
        .iter()
        .find(|(_, name, _)| *name == token)
        .map(|(_, _, url)| *url)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageWindow {
    pub label: String,
    #[serde(rename = "usedPercent")]
    pub used_percent: f64,
    #[serde(
        default,
        rename = "durationSeconds",
        skip_serializing_if = "Option::is_none"
    )]
    pub duration_seconds: Option<u64>,
    #[serde(default, rename = "resetAt", skip_serializing_if = "Option::is_none")]
    pub reset_at: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub provider: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub windows: Vec<UsageWindow>,
    #[serde(
        default,
        rename = "availableResets",
        skip_serializing_if = "Option::is_none"
    )]
    pub available_resets: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct UsageRow {
    pub provider: String,
    pub display_name: String,
    pub windows: Vec<UsageWindow>,
    pub available_resets: Option<u32>,
    pub stale: bool,
    pub fetched_at: Option<u64>,
    pub unavailable: bool,
    pub refresh_failure: Option<RefreshFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshFailure {
    SignInAgain,
    RefreshFailed,
}

#[derive(Debug)]
pub(crate) struct FetchError {
    pub retry_after: Option<Duration>,
    pub kind: RefreshFailure,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LastGood {
    fetched_at: u64,
    usage: UsageSnapshot,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    version: u8,
    last_good: Option<LastGood>,
    retry_at: Option<u64>,
    #[serde(default)]
    refresh_failure: Option<RefreshFailure>,
}

#[derive(Serialize, Deserialize)]
struct Lease {
    version: u8,
    token: String,
    expires_at: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn state_path(dir: &Path, provider: &str) -> PathBuf {
    dir.join(format!("provider-{provider}.json"))
}

fn lock_path(dir: &Path, provider: &str) -> PathBuf {
    dir.join(format!("provider-{provider}.lock"))
}

fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

fn read_state(dir: &Path, provider: &str) -> State {
    let state = read_bounded(&state_path(dir, provider), MAX_STATE_BYTES)
        .and_then(|bytes| serde_json::from_slice::<State>(&bytes).ok())
        .filter(|state| state.version == 1)
        .unwrap_or_default();
    if let Some(good) = &state.last_good {
        if good.usage.provider != provider
            || good.fetched_at > now_ms().saturating_add(60_000)
            || good.usage.windows.len() > MAX_WINDOWS
            || good
                .usage
                .available_resets
                .is_some_and(|count| count > MAX_AVAILABLE_RESETS)
            || good.usage.windows.iter().any(|window| {
                !window.used_percent.is_finite()
                    || !(0.0..=100.0).contains(&window.used_percent)
                    || window.label.len() > 256
                    || window
                        .duration_seconds
                        .is_some_and(|seconds| seconds == 0 || seconds > 366 * 86_400)
                    || window
                        .reset_at
                        .is_some_and(|seconds| seconds > now_ms() / 1000 + 366 * 86_400)
            })
            || good.usage.display_name.len() > 256
        {
            return State::default();
        }
    }
    state
}

fn read_lease(dir: &Path, provider: &str) -> Option<Lease> {
    let lease =
        serde_json::from_slice::<Lease>(&read_bounded(&lock_path(dir, provider), 1024)?).ok()?;
    (lease.version == 1 && !lease.token.is_empty() && lease.token.len() < 128).then_some(lease)
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn token() -> String {
    format!(
        "{}-{}-{:?}",
        std::process::id(),
        now_ms(),
        std::thread::current().id()
    )
}

// An advisory guard serializes stale-lease removal and replacement. Callers wait
// for the guard because it is held briefly and a forked child can inherit it
// until exec. Live leases are still exclusive files, so a process never waits
// for another fetch.
fn try_lease(dir: &Path, provider: &str) -> Option<String> {
    ensure_dir(dir).ok()?;
    let guard = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(format!("provider-{provider}.guard")))
        .ok()?;
    guard.lock_exclusive().ok()?;
    let path = lock_path(dir, provider);
    if path.exists() {
        match read_lease(dir, provider) {
            Some(lease) if lease.expires_at > now_ms() => return None,
            None => {
                let old_enough = fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age > Duration::from_millis(LEASE_MS));
                if !old_enough {
                    return None;
                }
                fs::remove_file(&path).ok()?;
            }
            _ => fs::remove_file(&path).ok()?,
        }
    }
    let token = token();
    let lease = Lease {
        version: 1,
        token: token.clone(),
        expires_at: now_ms().saturating_add(LEASE_MS),
    };
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).ok()?;
    if file.write_all(&serde_json::to_vec(&lease).ok()?).is_err() {
        let _ = fs::remove_file(path);
        return None;
    }
    Some(token)
}

fn owns_lease(dir: &Path, provider: &str, token: &str) -> bool {
    read_lease(dir, provider)
        .is_some_and(|lease| lease.token == token && lease.expires_at > now_ms())
}

fn release_lease(dir: &Path, provider: &str, token: &str) {
    let Ok(guard) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(format!("provider-{provider}.guard")))
    else {
        return;
    };
    if guard.lock_exclusive().is_ok() && owns_lease(dir, provider, token) {
        let _ = fs::remove_file(lock_path(dir, provider));
    }
}

fn write_state(dir: &Path, provider: &str, state: &State, token: &str) -> Result<(), String> {
    if !owns_lease(dir, provider, token) {
        return Err("usage lease lost".into());
    }
    let target = state_path(dir, provider);
    let temp = dir.join(format!("provider-{provider}.{}.tmp", token));
    let bytes = serde_json::to_vec(state).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err("usage state too large".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    let result = (|| {
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        let guard = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(format!("provider-{provider}.guard")))
            .map_err(|error| error.to_string())?;
        guard.lock_exclusive().map_err(|error| error.to_string())?;
        if !owns_lease(dir, provider, token) {
            return Err("usage lease lost".into());
        }
        fs::rename(&temp, &target).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn row(provider: &str, state: &State) -> UsageRow {
    match &state.last_good {
        Some(good) => UsageRow {
            provider: provider.to_owned(),
            display_name: good.usage.display_name.clone(),
            windows: good.usage.windows.clone(),
            available_resets: good.usage.available_resets,
            stale: now_ms().saturating_sub(good.fetched_at) > STALE_MS,
            fetched_at: Some(good.fetched_at),
            unavailable: false,
            refresh_failure: state.refresh_failure,
        },
        None => UsageRow {
            provider: provider.to_owned(),
            display_name: provider.to_owned(),
            windows: Vec::new(),
            available_resets: None,
            stale: false,
            fetched_at: None,
            unavailable: true,
            refresh_failure: state.refresh_failure,
        },
    }
}

fn due(state: &State) -> bool {
    let now = now_ms();
    state
        .last_good
        .as_ref()
        .is_none_or(|good| now.saturating_sub(good.fetched_at) >= FRESH_MS)
        && state.retry_at.is_none_or(|retry| retry <= now)
}

pub(crate) fn resolve(providers: &[String], dir: &Path, spawn: bool) -> Vec<UsageRow> {
    providers
        .iter()
        .map(|provider| {
            let state = read_state(dir, provider);
            if spawn && due(&state) {
                if let Some(token) = try_lease(dir, provider) {
                    if launch_worker(dir, provider, &token).is_err() {
                        release_lease(dir, provider, &token);
                    }
                }
            }
            row(provider, &state)
        })
        .collect()
}

fn launch_worker(dir: &Path, provider: &str, token: &str) -> Result<(), String> {
    let mut child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
        .arg("usage-refresh")
        .arg(format!("--provider={provider}"))
        .arg(format!("--data-dir={}", dir.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    child
        .stdin
        .take()
        .ok_or("missing worker pipe")?
        .write_all(token.as_bytes())
        .map_err(|error| error.to_string())
}

pub fn refresh(provider: &str, dir: &Path, token: &str) -> Result<(), String> {
    if !PROVIDERS.contains(&provider) || !owns_lease(dir, provider, token) {
        return Err("invalid usage refresh lease".into());
    }
    let result = refresh_owned(provider, dir, token);
    release_lease(dir, provider, token);
    result
}

fn refresh_owned(provider: &str, dir: &Path, token: &str) -> Result<(), String> {
    refresh_owned_with(provider, dir, token, providers::fetch)
}

fn refresh_owned_with(
    provider: &str,
    dir: &Path,
    token: &str,
    fetch: impl FnOnce(&str, Option<&str>) -> Result<UsageSnapshot, FetchError>,
) -> Result<(), String> {
    let state = read_state(dir, provider);
    if !due(&state) {
        return Ok(());
    }
    let fetched = fetch(provider, None);
    if !owns_lease(dir, provider, token) {
        return Err("usage lease lost".into());
    }
    let next = match fetched {
        Ok(snapshot) => State {
            version: 1,
            last_good: Some(LastGood {
                fetched_at: now_ms(),
                usage: snapshot,
            }),
            retry_at: None,
            refresh_failure: None,
        },
        Err(error) => State {
            version: 1,
            last_good: state.last_good,
            retry_at: Some(
                now_ms().saturating_add(
                    error
                        .retry_after
                        .unwrap_or(Duration::from_millis(BACKOFF_MS))
                        .as_millis() as u64,
                ),
            ),
            refresh_failure: Some(error.kind),
        },
    };
    write_state(dir, provider, &next, token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "starmux-usage-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn snapshot() -> UsageSnapshot {
        UsageSnapshot {
            provider: "codex".into(),
            display_name: "Codex Plan".into(),
            windows: vec![UsageWindow {
                label: "5h".into(),
                used_percent: 37.0,
                duration_seconds: None,
                reset_at: None,
            }],
            available_resets: None,
        }
    }

    #[test]
    fn one_lease_controls_a_refresh_and_fresh_results_are_reused() {
        let dir = temp_dir();
        let first = try_lease(&dir, "codex").unwrap();
        assert!(try_lease(&dir, "codex").is_none());
        let requests = AtomicUsize::new(0);
        refresh_owned_with("codex", &dir, &first, |_, _| {
            requests.fetch_add(1, Ordering::SeqCst);
            Ok(UsageSnapshot {
                available_resets: Some(1),
                ..snapshot()
            })
        })
        .unwrap();
        release_lease(&dir, "codex", &first);
        let second = try_lease(&dir, "codex").unwrap();
        refresh_owned_with("codex", &dir, &second, |_, _| {
            requests.fetch_add(1, Ordering::SeqCst);
            Ok(snapshot())
        })
        .unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        let cached = resolve(&["codex".into()], &dir, false);
        assert_eq!(cached[0].windows[0].used_percent, 37.0);
        assert_eq!(cached[0].available_resets, Some(1));
        release_lease(&dir, "codex", &second);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_cache_and_invalid_reset_count() {
        let dir = temp_dir();
        let lease = try_lease(&dir, "codex").unwrap();
        let state = State {
            version: 1,
            last_good: Some(LastGood {
                fetched_at: now_ms(),
                usage: snapshot(),
            }),
            retry_at: None,
            refresh_failure: None,
        };
        write_state(&dir, "codex", &state, &lease).unwrap();
        assert_eq!(
            resolve(&["codex".into()], &dir, false)[0].available_resets,
            None
        );
        let mut invalid = state;
        invalid.last_good.as_mut().unwrap().usage.available_resets = Some(10_000);
        write_state(&dir, "codex", &invalid, &lease).unwrap();
        assert!(resolve(&["codex".into()], &dir, false)[0].unavailable);
        release_lease(&dir, "codex", &lease);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_cache_shows_the_failed_refresh_and_old_cache_still_loads() {
        let dir = temp_dir();
        let lease = try_lease(&dir, "codex").unwrap();
        refresh_owned_with("codex", &dir, &lease, |_, _| {
            Err(FetchError {
                retry_after: None,
                kind: RefreshFailure::RefreshFailed,
            })
        })
        .unwrap();
        release_lease(&dir, "codex", &lease);
        let failed = &resolve(&["codex".into()], &dir, false)[0];
        assert!(failed.unavailable);
        assert_eq!(failed.refresh_failure, Some(RefreshFailure::RefreshFailed));

        let mut legacy = serde_json::to_value(State {
            version: 1,
            last_good: Some(LastGood {
                fetched_at: now_ms(),
                usage: snapshot(),
            }),
            retry_at: None,
            refresh_failure: None,
        })
        .unwrap();
        legacy.as_object_mut().unwrap().remove("refresh_failure");
        fs::write(
            state_path(&dir, "codex"),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        let loaded = &resolve(&["codex".into()], &dir, false)[0];
        assert_eq!(loaded.windows[0].used_percent, 37.0);
        assert_eq!(loaded.refresh_failure, None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn usage_only_becomes_stale_after_thirty_minutes() {
        let now = now_ms();
        let mut state = State {
            version: 1,
            last_good: Some(LastGood {
                fetched_at: now - 29 * 60_000,
                usage: snapshot(),
            }),
            retry_at: Some(now + 60_000),
            refresh_failure: None,
        };
        assert!(!row("codex", &state).stale);
        state.last_good.as_mut().unwrap().fetched_at = now - 31 * 60_000;
        assert!(row("codex", &state).stale);
    }

    #[test]
    fn retry_deadline_preserves_last_good_and_suppresses_requests() {
        let dir = temp_dir();
        let lease = try_lease(&dir, "codex").unwrap();
        write_state(
            &dir,
            "codex",
            &State {
                version: 1,
                last_good: Some(LastGood {
                    fetched_at: now_ms() - FRESH_MS,
                    usage: snapshot(),
                }),
                retry_at: None,
                refresh_failure: None,
            },
            &lease,
        )
        .unwrap();
        refresh_owned_with("codex", &dir, &lease, |_, _| {
            Err(FetchError {
                retry_after: Some(Duration::from_secs(120)),
                kind: RefreshFailure::SignInAgain,
            })
        })
        .unwrap();
        release_lease(&dir, "codex", &lease);
        let row = &resolve(&["codex".into()], &dir, false)[0];
        assert!(!row.stale);
        assert_eq!(row.windows[0].used_percent, 37.0);
        assert_eq!(row.refresh_failure, Some(RefreshFailure::SignInAgain));
        let next = try_lease(&dir, "codex").unwrap();
        refresh_owned_with("codex", &dir, &next, |_, _| {
            panic!("retry deadline must suppress fetch")
        })
        .unwrap();
        release_lease(&dir, "codex", &next);
        let lease = try_lease(&dir, "codex").unwrap();
        let mut state = read_state(&dir, "codex");
        state.retry_at = None;
        write_state(&dir, "codex", &state, &lease).unwrap();
        refresh_owned_with("codex", &dir, &lease, |_, _| {
            let mut updated = snapshot();
            updated.windows[0].used_percent = 2.0;
            Ok(updated)
        })
        .unwrap();
        release_lease(&dir, "codex", &lease);
        let recovered = &resolve(&["codex".into()], &dir, false)[0];
        assert_eq!(recovered.windows[0].used_percent, 2.0);
        assert_eq!(recovered.refresh_failure, None);
        fs::remove_dir_all(dir).unwrap();
    }
}
