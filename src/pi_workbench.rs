use serde::Deserialize;
use serde_json::Value;
use std::{
    fs,
    io::Read,
    os::unix::{fs::FileTypeExt, net::UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_RECORDS: usize = 32;
const MAX_DIRECTORY_ENTRIES: usize = 256;
const MAX_RECORD_BYTES: u64 = 1024 * 1024;
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
    dirs::home_dir()
        .map(|home| home.join(".local/share/pi"))
        .ok_or_else(|| "HOME is required for pi-workbench".to_owned())
}

pub(crate) struct LiveEntry {
    pub session: PiSession,
    pub session_id: String,
    pub context_path: Option<PathBuf>,
}

#[expect(
    clippy::too_many_lines,
    reason = "The bounded scan validates and filters each record before publishing sessions"
)]
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
        if UnixStream::connect(socket_path).is_err() {
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
    sessions.sort_by(|a, b| {
        project_label(&a.project)
            .to_lowercase()
            .cmp(&project_label(&b.project).to_lowercase())
            .then_with(|| a.project.cmp(&b.project))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| {
                a.location
                    .as_ref()
                    .map(|location| &location.pane)
                    .cmp(&b.location.as_ref().map(|location| &location.pane))
            })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildState {
    Pending,
    Success,
    Failure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiPullRequest {
    pub url: String,
    pub state: crate::pr_state::PrState,
    pub build: Option<BuildState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiBuild {
    pub repository: String,
    pub branch: String,
    pub state: Option<BuildState>,
}

#[derive(Clone, Debug, Default)]
pub struct PiContext {
    pub session_id: String,
    pub plans: Vec<PiPlan>,
    pub pull_requests: Vec<PiPullRequest>,
    pub builds: Vec<PiBuild>,
    pub skills: Vec<PiSkill>,
}

const MAX_CONTEXT_BYTES: u64 = 1024 * 1024;
const MAX_ITEMS: usize = 16;

#[expect(
    clippy::too_many_lines,
    reason = "Context schema and path validation precede constructing the published context"
)]
pub(crate) fn read_context(path: &Path, session_id: &str) -> Result<PiContext, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PiContext {
                session_id: session_id.to_owned(),
                ..PiContext::default()
            });
        }
        Err(error) => return Err(format!("context file: {error}")),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONTEXT_BYTES {
        return Err("context is not a regular file or exceeds 1 MiB".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| format!("open context: {error}"))?
        .take(MAX_CONTEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read context: {error}"))?;
    if bytes.len() as u64 > MAX_CONTEXT_BYTES {
        return Err("context exceeds 1 MiB".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid context JSON: {error}"))?;
    if value.get("version").and_then(Value::as_u64) != Some(2)
        || value.get("sessionId").and_then(Value::as_str) != Some(session_id)
    {
        return Err("context version or session ID mismatch".into());
    }
    let session_file = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".context.json"))
        .ok_or("invalid context filename")?;
    let directory = path.parent().ok_or("invalid context directory")?;
    let namespaces = value
        .get("extensions")
        .and_then(Value::as_object)
        .ok_or("invalid context extensions")?;
    let plans_data = namespaces.get("pi-plans").and_then(entry_data);
    let github_data = namespaces.get("pi-github").and_then(entry_data);
    let skills_data = namespaces.get("pi-skills").and_then(entry_data);
    let empty = Vec::new();
    let plans = plans_data
        .map(|data| data.get("plans").and_then(Value::as_array))
        .unwrap_or(Some(&empty))
        .ok_or("invalid context plans")?;
    let (pull_requests, builds) =
        github_data.map_or_else(|| Ok(Default::default()), read_github)?;
    let skills = skills_data
        .and_then(|data| data.get("skills"))
        .map(Value::as_array)
        .unwrap_or(Some(&Vec::new()))
        .cloned()
        .ok_or("invalid context skills")?;
    let skill_paths = skills_data
        .and_then(|data| data.get("skillPaths"))
        .map(Value::as_object)
        .unwrap_or(Some(&serde_json::Map::new()))
        .cloned()
        .ok_or("invalid context skill paths")?;
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
        return Err("invalid context item or path".into());
    }
    Ok(PiContext {
        session_id: session_id.to_owned(),
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
        pull_requests,
        builds,
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

type GithubContext = (Vec<PiPullRequest>, Vec<PiBuild>);

fn read_github(data: &Value) -> Result<GithubContext, String> {
    let invalid = || "invalid context pull requests or builds".to_owned();
    let empty = Vec::new();
    let prs = match data.get("pullRequests") {
        Some(value) => value.as_array().ok_or_else(invalid)?,
        None => &empty,
    };
    let Some(builds) = data.get("builds") else {
        let prs = prs
            .iter()
            .map(|url| {
                url.as_str()
                    .filter(|url| crate::pr_state::parse_url(url).is_some())
                    .map(|url| PiPullRequest {
                        url: url.to_owned(),
                        state: crate::pr_state::PrState::Unknown,
                        build: None,
                    })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid)?;
        return Ok((prs.into_iter().take(MAX_ITEMS).collect(), Vec::new()));
    };
    let builds = builds
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(read_build)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let prs = prs
        .iter()
        .map(read_pull_request)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let on_branch = |build: &PiBuild, repository: &str, branch: Option<&str>| {
        build.repository == repository && Some(build.branch.as_str()) == branch
    };
    let pull_requests = prs
        .iter()
        .map(|(pr, repository, branch)| PiPullRequest {
            build: builds
                .iter()
                .find(|build| on_branch(build, repository, branch.as_deref()))
                .and_then(|build| build.state),
            ..pr.clone()
        })
        .take(MAX_ITEMS)
        .collect();
    let builds = builds
        .into_iter()
        .filter(|build| {
            !prs.iter()
                .any(|(_, repository, branch)| on_branch(build, repository, branch.as_deref()))
        })
        .take(MAX_ITEMS)
        .collect();
    Ok((pull_requests, builds))
}

fn read_build(value: &Value) -> Option<PiBuild> {
    let repository = value.get("repository")?.as_str()?;
    let branch = value.get("branch")?.as_str()?;
    let state = match value.get("checks") {
        None | Some(Value::Null) => None,
        Some(checks) => match checks.get("state")?.as_str()? {
            "pending" => Some(BuildState::Pending),
            "success" => Some(BuildState::Success),
            "failure" => Some(BuildState::Failure),
            "none" => None,
            _ => return None,
        },
    };
    (valid_repository(repository) && valid_branch(branch)).then(|| PiBuild {
        repository: repository.to_owned(),
        branch: branch.to_owned(),
        state,
    })
}

fn read_pull_request(value: &Value) -> Option<(PiPullRequest, String, Option<String>)> {
    let url = value.get("url")?.as_str()?;
    let (label, _) = crate::pr_state::parse_url(url)?;
    let repository = value.get("repository")?.as_str()?;
    if label.split_once('#').map(|(repo, _)| repo) != Some(repository) {
        return None;
    }
    let branch = match value.get("branch") {
        None | Some(Value::Null) => None,
        Some(branch) => Some(branch.as_str().filter(|b| valid_branch(b))?.to_owned()),
    };
    let state = match value.get("state") {
        None | Some(Value::Null) => crate::pr_state::PrState::Unknown,
        Some(state) => match state.as_str()? {
            "open" => crate::pr_state::PrState::Open,
            "closed" => crate::pr_state::PrState::Closed,
            "merged" => crate::pr_state::PrState::Merged,
            _ => return None,
        },
    };
    let pr = PiPullRequest {
        url: url.to_owned(),
        state,
        build: None,
    };
    Some((pr, repository.to_owned(), branch))
}

fn valid_repository(repository: &str) -> bool {
    repository.len() <= 256
        && repository.split_once('/').is_some_and(|(owner, name)| {
            [owner, name].iter().all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b))
            })
        })
}

fn valid_branch(branch: &str) -> bool {
    !branch.is_empty() && branch.len() <= 255 && !branch.chars().any(char::is_control)
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
    let home = dirs::home_dir()?;
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
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().to_path_buf();
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
    }
    fn github_context(github: serde_json::Value) -> Result<PiContext, String> {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("session.jsonl.context.json");
        let value = serde_json::json!({
            "version": 2, "sessionId": "session", "extensions": {"pi-github": github}
        });
        fs::write(&path, value.to_string()).unwrap();
        read_context(&path, "session")
    }

    fn build(repository: &str, branch: &str, checks: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "repository": repository, "branch": branch, "sha": "abc",
            "pushedAt": "2026-10-07T10:40:30.000Z", "source": "agent", "checks": checks
        })
    }

    #[test]
    fn pull_requests_take_the_build_of_their_branch_and_other_builds_stay_separate() {
        let checks = |state: &str| serde_json::json!({"sha": "abc", "state": state, "updatedAt": "", "runs": []});
        let context = github_context(serde_json::json!({"version": 1, "data": {
            "builds": [
                build("owner/repo", "feature", checks("failure")),
                build("owner/repo", "main", checks("success")),
                build("owner/other", "main", checks("none")),
                build("owner/other", "release", serde_json::Value::Null),
            ],
            "pullRequests": [
                {"repository": "owner/repo", "number": 7, "url": "https://github.com/owner/repo/pull/7",
                 "branch": "feature", "title": "Feature", "state": "merged"},
                {"repository": "owner/repo", "number": 8, "url": "https://github.com/owner/repo/pull/8",
                 "branch": null, "title": null, "state": null},
            ]
        }}))
        .unwrap();
        assert_eq!(
            context.pull_requests,
            [
                PiPullRequest {
                    url: "https://github.com/owner/repo/pull/7".into(),
                    state: crate::pr_state::PrState::Merged,
                    build: Some(BuildState::Failure),
                },
                PiPullRequest {
                    url: "https://github.com/owner/repo/pull/8".into(),
                    state: crate::pr_state::PrState::Unknown,
                    build: None,
                },
            ]
        );
        let build = |repository: &str, branch: &str, state| PiBuild {
            repository: repository.into(),
            branch: branch.into(),
            state,
        };
        assert_eq!(
            context.builds,
            [
                build("owner/repo", "main", Some(BuildState::Success)),
                build("owner/other", "main", None),
                build("owner/other", "release", None),
            ]
        );
    }

    #[test]
    fn legacy_pull_request_urls_are_read_without_state_or_build() {
        let context = github_context(serde_json::json!({"version": 1, "data": {
            "pullRequests": ["https://github.com/owner/repo/pull/42"]
        }}))
        .unwrap();
        assert_eq!(
            context.pull_requests,
            [PiPullRequest {
                url: "https://github.com/owner/repo/pull/42".into(),
                state: crate::pr_state::PrState::Unknown,
                build: None,
            }]
        );
        assert!(context.builds.is_empty());
        let unpublished = github_context(serde_json::json!({"version": 2, "data": {
            "branches": [{"repository": "owner/repo", "branch": "main"}]
        }}))
        .unwrap();
        assert!(unpublished.pull_requests.is_empty() && unpublished.builds.is_empty());
    }

    #[test]
    fn invalid_builds_or_pull_requests_reject_the_context() {
        let pr = |repository: &str, branch: serde_json::Value, state: &str| {
            serde_json::json!({"repository": repository, "number": 1,
                "url": "https://github.com/owner/repo/pull/1", "branch": branch, "title": null, "state": state})
        };
        for data in [
            serde_json::json!({"builds": [build("owner", "main", serde_json::Value::Null)]}),
            serde_json::json!({"builds": [build("owner/repo/x", "main", serde_json::Value::Null)]}),
            serde_json::json!({"builds": [build("owner/repo", "main\u{1b}[31m", serde_json::Value::Null)]}),
            serde_json::json!({"builds": [build("owner/repo", "", serde_json::Value::Null)]}),
            serde_json::json!({"builds": [build("owner/repo", "main", serde_json::json!({"state": "skipped"}))]}),
            serde_json::json!({"builds": [], "pullRequests": [pr("other/repo", serde_json::Value::Null, "open")]}),
            serde_json::json!({"builds": [], "pullRequests": [pr("owner/repo", serde_json::json!(3), "open")]}),
            serde_json::json!({"builds": [], "pullRequests": [pr("owner/repo", serde_json::Value::Null, "draft")]}),
            serde_json::json!({"builds": [], "pullRequests": ["https://github.com/owner/repo/pull/1"]}),
            serde_json::json!({"pullRequests": ["https://example.com/owner/repo/pull/1"]}),
        ] {
            assert!(
                github_context(serde_json::json!({"version": 1, "data": data})).is_err(),
                "accepted {data}"
            );
        }
    }

    #[test]
    fn a_missing_context_file_is_an_empty_context() {
        let tempdir = tempfile::tempdir().unwrap();
        let context = read_context(&tempdir.path().join("new.jsonl.context.json"), "new").unwrap();
        assert_eq!(context.session_id, "new");
        assert!(context.plans.is_empty() && context.pull_requests.is_empty());
    }

    #[test]
    fn reads_only_matching_regular_context_and_caps_entries() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().to_path_buf();
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
        assert!(read_context(&path, "session").is_err());
        fs::write(&path, value.to_string()).unwrap();
        assert!(read_context(&path, "another").is_err());
        let link = dir.join("link.context.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_context(&link, "session").is_err());
        fs::write(&path, "{").unwrap();
        assert!(read_context(&path, "session").is_err());
    }
}
