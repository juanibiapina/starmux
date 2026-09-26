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
const MAX_RECORD_BYTES: u64 = 16 * 1024;
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
    #[serde(rename = "socketPath")]
    socket_path: PathBuf,
    #[serde(rename = "startedAt")]
    started_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    state: String,
    tmux: Option<TmuxLocation>,
}

pub(crate) fn default_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/share/pi"))
        .ok_or_else(|| "HOME is required for pi-live".to_owned())
}

pub(crate) fn list(data_dir: &Path, tmux_socket: &Path) -> Result<Vec<PiSession>, String> {
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
        if record.version != 1
            || record.session_id != id
            || record.pid == 0
            || record.cwd.is_empty()
            || record.started_at.is_empty()
            || record.updated_at.is_empty()
            || !matches!(record.state.as_str(), "idle" | "working")
            || record.socket_path.parent() != Some(data_dir.join("sockets").as_path())
            || record.tmux.as_ref().is_none_or(|tmux| {
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
        let Ok(socket_metadata) = fs::symlink_metadata(&record.socket_path) else {
            continue;
        };
        if !socket_metadata.file_type().is_socket() {
            continue;
        }
        let remaining = QUERY_BUDGET.saturating_sub(started.elapsed());
        let timeout = PING_TIMEOUT.min(remaining);
        if timeout.is_zero() || !ping(&record.socket_path, timeout) {
            continue;
        }
        let name = record
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| record.session_id.chars().take(8).collect());
        let location = record.tmux.map(|tmux| PiLocation {
            pane: tmux.pane_id,
            session_name: tmux.session_name,
        });
        sessions.push(PiSession {
            name,
            project: project_root(&record.cwd),
            state: record.state,
            location,
            target: None,
            selected: false,
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
