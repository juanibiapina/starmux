use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::{fs::FileTypeExt, net::UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_RECORDS: usize = 32;
const MAX_DIRECTORY_ENTRIES: usize = 256;
const MAX_RECORD_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024;
const PING_TIMEOUT: Duration = Duration::from_millis(40);
const QUERY_BUDGET: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiSession {
    pub name: String,
    pub project: String,
    pub state: String,
    pub location: Option<PiLocation>,
    pub target: Option<PiTarget>,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiLocation {
    pub pane: String,
    pub session_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiTarget {
    pub pane: String,
    pub window: String,
}

#[derive(Deserialize)]
struct TmuxLocation {
    #[serde(rename = "paneId")]
    pane_id: String,
    #[serde(rename = "sessionName")]
    session_name: String,
    #[serde(rename = "socketPath")]
    socket_path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Record {
    version: u64,
    #[serde(rename = "sessionId")]
    session_id: String,
    name: Option<String>,
    pid: u64,
    cwd: String,
    #[serde(rename = "startedAt")]
    started_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    state: String,
    extensions: Value,
    #[serde(rename = "sessionFile")]
    session_file: Option<PathBuf>,
    #[serde(rename = "contextPath")]
    context_path: Option<PathBuf>,
}

pub(crate) fn default_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/share/pi"))
        .ok_or_else(|| "HOME is required for pi-live".to_owned())
}

pub(crate) struct LiveEntry {
    pub session: PiSession,
    pub session_id: String,
    pub context_path: Option<PathBuf>,
}

pub(crate) fn list(data_dir: &Path, tmux_socket: &Path) -> Result<Vec<LiveEntry>, String> {
    let status_dir = data_dir.join("status");
    let entries = match fs::read_dir(&status_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", status_dir.display())),
    };
    let mut paths = Vec::new();
    for entry in entries.take(MAX_DIRECTORY_ENTRIES) {
        let entry = entry.map_err(|error| format!("{}: {error}", status_dir.display()))?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
            continue;
        };
        if valid_id(id) {
            paths.push((id.to_owned(), entry.path()));
        }
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));

    let started = Instant::now();
    let mut sessions = Vec::new();
    let mut matching_records = 0;
    for (id, path) in paths {
        if started.elapsed() >= QUERY_BUDGET {
            break;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() || metadata.len() > MAX_RECORD_BYTES {
            continue;
        }
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        let mut bytes = Vec::new();
        if file
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > MAX_RECORD_BYTES
        {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Record>(&bytes) else {
            continue;
        };
        let socket_path = extension_data(&record.extensions, "pi-socket")
            .and_then(|data| data.get("socketPath"))
            .and_then(Value::as_str)
            .map(Path::new);
        let tmux = extension_data(&record.extensions, "pi-tmux")
            .and_then(|data| serde_json::from_value::<TmuxLocation>(data.clone()).ok());
        if record.version != 2
            || record.session_id != id
            || record.pid == 0
            || record.cwd.is_empty()
            || record.started_at.is_empty()
            || record.updated_at.is_empty()
            || !matches!(record.state.as_str(), "idle" | "working")
            || socket_path.and_then(Path::parent) != Some(data_dir.join("sockets").as_path())
            || tmux.as_ref().is_none_or(|tmux| {
                tmux.socket_path.as_deref() != Some(tmux_socket)
                    || !tmux_socket.is_absolute()
                    || !valid_tmux_id(&tmux.pane_id, '%')
            })
        {
            continue;
        }
        if matching_records == MAX_RECORDS {
            break;
        }
        matching_records += 1;
        let Some(socket_path) = socket_path else {
            continue;
        };
        let Ok(socket_metadata) = fs::symlink_metadata(socket_path) else {
            continue;
        };
        if !socket_metadata.file_type().is_socket() {
            continue;
        }
        let remaining = QUERY_BUDGET.saturating_sub(started.elapsed());
        let timeout = PING_TIMEOUT.min(remaining);
        if timeout.is_zero() || !ping(socket_path, timeout) {
            continue;
        }
        let name = record
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| record.session_id.chars().take(8).collect());
        let location = tmux.map(|tmux| PiLocation {
            pane: tmux.pane_id,
            session_name: tmux.session_name,
        });
        let context_path = match (&record.session_file, &record.context_path) {
            (Some(file), Some(context))
                if file.is_absolute()
                    && context == &PathBuf::from(format!("{}.context.json", file.display())) =>
            {
                Some(context.clone())
            }
            _ => None,
        };
        sessions.push(LiveEntry {
            session_id: record.session_id,
            context_path,
            session: PiSession {
                name,
                project: project_root(&record.cwd),
                state: record.state,
                location,
                target: None,
                selected: false,
            },
        });
    }
    Ok(sessions)
}

pub(crate) fn sort_sessions(sessions: &mut [PiSession]) {
    let priority = |session: &PiSession| match session.state.as_str() {
        "notify" => 0,
        "working" => 1,
        _ if session.selected => 2,
        _ => 3,
    };
    let mut project_priority = BTreeMap::new();
    for session in sessions.iter() {
        let rank = project_priority.entry(session.project.clone()).or_insert(3);
        *rank = (*rank).min(priority(session));
    }
    sessions.sort_by(|a, b| {
        project_priority[&a.project]
            .cmp(&project_priority[&b.project])
            .then_with(|| {
                project_label(&a.project)
                    .to_lowercase()
                    .cmp(&project_label(&b.project).to_lowercase())
            })
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| priority(a).cmp(&priority(b)))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub(crate) fn project_label(project: &str) -> &str {
    Path::new(project)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(project)
}

fn project_root(cwd: &str) -> String {
    let path = Path::new(cwd);
    for ancestor in path.ancestors() {
        if ancestor.join(".git").exists() {
            return ancestor.to_string_lossy().into_owned();
        }
    }
    cwd.to_owned()
}

fn extension_data<'a>(extensions: &'a Value, name: &str) -> Option<&'a Value> {
    entry_data(extensions.get(name)?)
}

fn entry_data(entry: &Value) -> Option<&Value> {
    (entry.get("version")?.as_u64()? == 1).then_some(entry.get("data")?)
}

fn valid_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphanumeric())
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_tmux_id(value: &str, prefix: char) -> bool {
    value.strip_prefix(prefix).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn ping(path: &Path, timeout: Duration) -> bool {
    let started = Instant::now();
    let Ok(mut socket) = UnixStream::connect(path) else {
        return false;
    };
    if socket.set_write_timeout(Some(timeout)).is_err()
        || socket
            .write_all(b"{\"type\":\"ping\",\"protocolVersion\":1}\n")
            .is_err()
    {
        return false;
    }
    let mut response = [0; MAX_RESPONSE_BYTES];
    let mut length = 0;
    while length < response.len() {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() || socket.set_read_timeout(Some(remaining)).is_err() {
            return false;
        }
        let Ok(count) = socket.read(&mut response[length..]) else {
            return false;
        };
        if count == 0 {
            return false;
        }
        length += count;
        if let Some(end) = response[..length].iter().position(|byte| *byte == b'\n') {
            let Ok(value) = serde_json::from_slice::<Value>(&response[..end]) else {
                return false;
            };
            return value.get("ok") == Some(&Value::Bool(true))
                && value.pointer("/result/type") == Some(&Value::String("pong".into()));
        }
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiPlan {
    pub title: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiSkill {
    pub name: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct PiContext {
    pub plans: Vec<PiPlan>,
    pub pull_requests: Vec<String>,
    pub skills: Vec<PiSkill>,
}

const MAX_CONTEXT_BYTES: u64 = 1024 * 1024;
const MAX_ITEMS: usize = 16;

pub(crate) fn read_context(path: &Path, session_id: &str) -> Option<PiContext> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONTEXT_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_CONTEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_CONTEXT_BYTES {
        return None;
    }
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    if value.get("version")?.as_u64()? != 2 || value.get("sessionId")?.as_str()? != session_id {
        return None;
    }
    let session_file = path.file_name()?.to_str()?.strip_suffix(".context.json")?;
    let directory = path.parent()?;
    let namespaces = value.get("extensions")?.as_object()?;
    let plans_data = namespaces.get("pi-plans").and_then(entry_data);
    let github_data = namespaces.get("pi-github").and_then(entry_data);
    let skills_data = namespaces.get("pi-skills").and_then(entry_data);
    let empty = Vec::new();
    let plans = plans_data
        .map(|data| data.get("plans").and_then(Value::as_array))
        .unwrap_or(Some(&empty))?;
    let prs = github_data
        .and_then(|data| data.get("pullRequests"))
        .map(Value::as_array)
        .unwrap_or(Some(&Vec::new()))
        .cloned()?;
    let skills = skills_data
        .and_then(|data| data.get("skills"))
        .map(Value::as_array)
        .unwrap_or(Some(&Vec::new()))
        .cloned()?;
    let skill_paths = skills_data
        .and_then(|data| data.get("skillPaths"))
        .map(Value::as_object)
        .unwrap_or(Some(&serde_json::Map::new()))
        .cloned()?;
    if plans.iter().any(|p| {
        let id = p.get("id").and_then(Value::as_str);
        let title = p.get("title").and_then(Value::as_str);
        let relative = p.get("path").and_then(Value::as_str);
        id.is_none_or(|id| {
            id.len() != 24
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) || title.is_none_or(str::is_empty)
            || id
                .zip(relative)
                .is_none_or(|(id, relative)| relative != format!("{session_file}.plans/{id}.md"))
    }) || prs.iter().any(|p| {
        p.as_str()
            .is_none_or(|s| crate::pr_state::parse_url(s).is_none())
    }) || skills
        .iter()
        .any(|s| s.as_str().is_none_or(|name| !valid_skill_name(name)))
        || skill_paths.iter().any(|(name, value)| {
            !skills.iter().any(|skill| skill.as_str() == Some(name))
                || value
                    .as_str()
                    .is_none_or(|file| !valid_skill_path(Path::new(file)))
        })
    {
        return None;
    }
    Some(PiContext {
        plans: plans
            .iter()
            .take(MAX_ITEMS)
            .filter_map(|p| {
                Some(PiPlan {
                    title: p.get("title")?.as_str()?.to_owned(),
                    path: directory.join(p.get("path")?.as_str()?),
                })
            })
            .collect(),
        pull_requests: prs
            .iter()
            .take(MAX_ITEMS)
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect(),
        skills: skills
            .iter()
            .take(MAX_ITEMS)
            .filter_map(|s| {
                let name = s.as_str()?;
                let path = skill_paths
                    .get(name)
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .or_else(|| local_skill_path(name));
                Some(PiSkill {
                    name: name.to_owned(),
                    path,
                })
            })
            .collect(),
    })
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_skill_path(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some_and(|name| name == "SKILL.md")
        && path.as_os_str().len() <= 4096
}

fn local_skill_path(name: &str) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    [home.join(".agents/skills"), home.join(".pi/agent/skills")]
        .into_iter()
        .map(|dir| dir.join(name).join("SKILL.md"))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn namespaced_context_allows_missing_features() {
        let dir = std::env::temp_dir().join(format!("starmux-context-v2-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl.context.json");
        fs::write(
            &path,
            r#"{"version":2,"sessionId":"session","extensions":{}}"#,
        )
        .unwrap();
        let context = read_context(&path, "session").unwrap();
        assert!(
            context.plans.is_empty()
                && context.pull_requests.is_empty()
                && context.skills.is_empty()
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reads_only_matching_regular_context_and_caps_entries() {
        let dir = std::env::temp_dir().join(format!("starmux-context-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl.context.json");
        let value = serde_json::json!({
            "version": 2, "sessionId": "session",
            "extensions": {
                "pi-plans": {"version": 1, "data": {"plans": (0..20).map(|n| serde_json::json!({"id": format!("{n:024x}"), "title": format!("Plan {n}"), "path": format!("session.jsonl.plans/{n:024x}.md")})).collect::<Vec<_>>() }},
                "pi-github": {"version": 1, "data": {"pullRequests": ["https://github.com/owner/repo/pull/42"]}},
                "pi-skills": {"version": 1, "data": {"skills": ["testing"]}}
            }
        });
        fs::write(&path, value.to_string()).unwrap();
        assert_eq!(read_context(&path, "session").unwrap().plans.len(), 16);
        fs::write(&path, r#"{"version":1,"sessionId":"session","plans":[]}"#).unwrap();
        assert!(read_context(&path, "session").is_none());
        fs::write(&path, value.to_string()).unwrap();
        assert!(read_context(&path, "another").is_none());
        let link = dir.join("link.context.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_context(&link, "session").is_none());
        fs::write(&path, "{").unwrap();
        assert!(read_context(&path, "session").is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
