use crate::{
    process::{self, Limits, Stream},
    BuildState,
};
use serde::{de::IgnoredAny, Deserialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const MAX_OUTPUT: u64 = 4 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_millis(1500);
const WORKSPACE_HEAD: &str = "ref: refs/heads/gitbutler/workspace";
const MAX_STACKS: usize = 32;
const MAX_BRANCHES: usize = 64;
const MAX_NAME: usize = 256;
pub(crate) const MAX_COMMITS: usize = 20;
const MAX_MESSAGE: usize = 200;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitButlerStatus {
    pub base: Option<GitButlerCommit>,
    pub branches: Vec<GitButlerBranch>,
    pub unassigned_changes: u64,
    pub conflicted_files: u64,
    pub behind: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitButlerBranch {
    pub name: String,
    pub depth: usize,
    pub commits: u64,
    pub recent_commits: Vec<GitButlerCommit>,
    pub upstream_commits: u64,
    pub changes: u64,
    pub push: PushStatus,
    pub conflicted: bool,
    pub review: Option<String>,
    pub ci: Option<BuildState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitButlerCommit {
    pub id: String,
    pub message: String,
    pub conflicted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushStatus {
    Pushed,
    Unpushed,
    NeedsForce,
    Local,
    Integrated,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    #[serde(default)]
    uncommitted_changes: Vec<IgnoredAny>,
    #[serde(default)]
    conflicted_files: Vec<IgnoredAny>,
    #[serde(default)]
    stacks: Vec<WireStack>,
    merge_base: Option<WireCommit>,
    upstream_state: Option<WireUpstream>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireStack {
    #[serde(default)]
    assigned_changes: Vec<IgnoredAny>,
    #[serde(default)]
    branches: Vec<WireBranch>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireBranch {
    name: String,
    #[serde(default)]
    commits: Vec<WireCommit>,
    #[serde(default)]
    upstream_commits: Vec<IgnoredAny>,
    #[serde(default)]
    branch_status: String,
    #[serde(default)]
    review_id: Option<String>,
    #[serde(default)]
    ci: Option<WireCi>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCommit {
    #[serde(default)]
    commit_id: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    conflicted: Option<bool>,
}

#[derive(Deserialize)]
struct WireCi {
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: String,
}

#[derive(Deserialize)]
struct WireUpstream {
    #[serde(default)]
    behind: u64,
}

pub(crate) fn query(pane_path: &Path) -> Result<Option<GitButlerStatus>, String> {
    query_with("but", pane_path)
}

fn query_with(binary: &str, pane_path: &Path) -> Result<Option<GitButlerStatus>, String> {
    let Some(root) = workspace_root(pane_path) else {
        return Ok(None);
    };
    let mut command = Command::new(binary);
    command
        .arg("-C")
        .arg(&root)
        .args(["status", "--json"])
        .current_dir(&root)
        .env("NO_BG_TASKS", "1");
    let limits = Limits {
        deadline: Instant::now() + TIMEOUT,
        stdout: MAX_OUTPUT,
        stderr: 4096,
    };
    let output = match process::run(command, limits) {
        Ok(output) => output,
        Err(process::Error::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None)
        }
        Err(process::Error::Spawn(error)) => return Err(format!("but status: {error}")),
        Err(process::Error::TimedOut) => return Err("but status timed out".into()),
        Err(process::Error::TooLarge(Stream::Stdout)) => {
            return Err("but status output exceeds 4 MiB".into())
        }
        Err(process::Error::TooLarge(Stream::Stderr)) => {
            return Err("but status error output exceeds 4 KiB".into())
        }
        Err(process::Error::Io(error)) => return Err(format!("but status output: {error}")),
    };
    if !output.status.success() {
        return Err(format!(
            "but status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse(&output.stdout).map(Some)
}

fn workspace_root(path: &Path) -> Option<PathBuf> {
    let dir = path.ancestors().find(|dir| dir.join(".git").exists())?;
    let git = dir.join(".git");
    if !fs::metadata(&git).ok()?.is_dir() {
        return None;
    }
    let mut head = String::new();
    fs::File::open(git.join("HEAD"))
        .ok()?
        .take(256)
        .read_to_string(&mut head)
        .ok()?;
    (head.trim_end() == WORKSPACE_HEAD).then(|| dir.to_path_buf())
}

fn parse(bytes: &[u8]) -> Result<GitButlerStatus, String> {
    let wire: Wire =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid but status: {error}"))?;
    let mut branches = Vec::new();
    for stack in wire.stacks.into_iter().take(MAX_STACKS) {
        let changes = stack.assigned_changes.len() as u64;
        for (depth, branch) in stack.branches.into_iter().enumerate() {
            if branches.len() == MAX_BRANCHES {
                break;
            }
            branches.push(GitButlerBranch {
                name: branch.name.chars().take(MAX_NAME).collect(),
                depth,
                commits: branch.commits.len() as u64,
                conflicted: branch
                    .commits
                    .iter()
                    .any(|commit| commit.conflicted == Some(true)),
                recent_commits: branch
                    .commits
                    .into_iter()
                    .take(MAX_COMMITS)
                    .map(commit)
                    .collect(),
                upstream_commits: branch.upstream_commits.len() as u64,
                changes: if depth == 0 { changes } else { 0 },
                push: match branch.branch_status.as_str() {
                    "nothingToPush" => PushStatus::Pushed,
                    "unpushedCommits" => PushStatus::Unpushed,
                    "unpushedCommitsRequiringForce" => PushStatus::NeedsForce,
                    "integrated" => PushStatus::Integrated,
                    _ => PushStatus::Local,
                },
                review: branch.review_id.filter(|id| {
                    (1..=16).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
                }),
                ci: branch
                    .ci
                    .and_then(|ci| match (ci.status.as_str(), ci.conclusion.as_str()) {
                        (_, "failure") => Some(BuildState::Failure),
                        ("inProgress", _) => Some(BuildState::Pending),
                        (_, "success") => Some(BuildState::Success),
                        _ => None,
                    }),
            });
        }
    }
    Ok(GitButlerStatus {
        base: wire
            .merge_base
            .map(commit)
            .filter(|base| !base.id.is_empty()),
        branches,
        unassigned_changes: wire.uncommitted_changes.len() as u64,
        conflicted_files: wire.conflicted_files.len() as u64,
        behind: wire.upstream_state.map_or(0, |upstream| upstream.behind),
    })
}

fn commit(wire: WireCommit) -> GitButlerCommit {
    let id = if wire.commit_id.len() >= 7 && wire.commit_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        wire.commit_id[..7].to_owned()
    } else {
        String::new()
    };
    GitButlerCommit {
        id,
        message: wire
            .message
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(MAX_MESSAGE)
            .collect(),
        conflicted: wire.conflicted == Some(true),
    }
}

#[cfg(test)]
mod tests {
    use super::{query_with, GitButlerBranch, GitButlerCommit, GitButlerStatus, PushStatus};
    use crate::BuildState;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    fn repo(head: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::create_dir_all(dir.path().join("src/nested")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), head).unwrap();
        dir
    }

    fn fake_but(dir: &Path, body: &str) -> String {
        let script = dir.join("fake-but");
        fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        script.to_str().unwrap().to_owned()
    }

    fn branch(name: &str) -> GitButlerBranch {
        GitButlerBranch {
            name: name.into(),
            depth: 0,
            commits: 0,
            recent_commits: Vec::new(),
            upstream_commits: 0,
            changes: 0,
            push: PushStatus::Local,
            conflicted: false,
            review: None,
            ci: None,
        }
    }

    #[test]
    fn workspace_status_describes_stacks_branches_and_changes() {
        let repo = repo("ref: refs/heads/gitbutler/workspace\n");
        let tools = tempfile::tempdir().unwrap();
        let json = serde_json::json!({
            "uncommittedChanges": [
                {"cliId": "a", "filePath": "README.md", "changeType": "modified"},
                {"cliId": "b", "filePath": "new.rs", "changeType": "added"}
            ],
            "conflictedFiles": ["y"],
            "stacks": [
                {
                    "cliId": "g0",
                    "assignedChanges": [
                        {"cliId": "c", "filePath": "old.rs", "changeType": "removed"},
                        {"cliId": "d", "filePath": "moved.rs", "changeType": "renamed"}
                    ],
                    "branches": [
                        {
                            "cliId": "al", "name": "feature-top",
                            "commits": [
                                {"commitId": "aaaaaaa1111", "message": "Add top\n\nBody", "conflicted": false},
                                {"commitId": "bbbbbbb2222", "message": "Fix top", "conflicted": true}
                            ],
                            "upstreamCommits": [{}],
                            "branchStatus": "unpushedCommitsRequiringForce",
                            "reviewId": "123",
                            "ci": {"status": "inProgress", "conclusion": "unknown"}
                        },
                        {
                            "cliId": "bm", "name": "feature-base",
                            "commits": [{"commitId": "ccccccc3333", "message": "Base", "conflicted": null}],
                            "upstreamCommits": [],
                            "branchStatus": "nothingToPush",
                            "reviewId": "12a",
                            "ci": {"status": "complete", "conclusion": "failure"}
                        }
                    ]
                },
                {
                    "cliId": "g1",
                    "assignedChanges": [],
                    "branches": [{
                        "cliId": "cn", "name": "#[fg=red]",
                        "commits": [], "upstreamCommits": [],
                        "branchStatus": "somethingNew", "reviewId": null, "ci": null
                    }]
                }
            ],
            "mergeBase": {"commitId": "50e5d71236ff", "message": "Open builds\n\nmore"},
            "upstreamState": {"behind": 4, "latestCommit": {}, "lastFetched": null}
        });
        fs::write(tools.path().join("status.json"), json.to_string()).unwrap();
        let but = fake_but(
            tools.path(),
            &format!(
                "[ \"$*\" = \"-C {} status --json\" ] || exit 3\ncat '{}'",
                repo.path().display(),
                tools.path().join("status.json").display()
            ),
        );
        let status = query_with(&but, &repo.path().join("src/nested"))
            .unwrap()
            .unwrap();
        let commit = |id: &str, message: &str, conflicted| GitButlerCommit {
            id: id.into(),
            message: message.into(),
            conflicted,
        };
        assert_eq!(
            status,
            GitButlerStatus {
                base: Some(commit("50e5d71", "Open builds", false)),
                branches: vec![
                    GitButlerBranch {
                        commits: 2,
                        recent_commits: vec![
                            commit("aaaaaaa", "Add top", false),
                            commit("bbbbbbb", "Fix top", true),
                        ],
                        upstream_commits: 1,
                        changes: 2,
                        push: PushStatus::NeedsForce,
                        conflicted: true,
                        review: Some("123".into()),
                        ci: Some(BuildState::Pending),
                        ..branch("feature-top")
                    },
                    GitButlerBranch {
                        depth: 1,
                        commits: 1,
                        recent_commits: vec![commit("ccccccc", "Base", false)],
                        push: PushStatus::Pushed,
                        ci: Some(BuildState::Failure),
                        ..branch("feature-base")
                    },
                    branch("#[fg=red]"),
                ],
                unassigned_changes: 2,
                conflicted_files: 1,
                behind: 4,
            }
        );
    }

    #[test]
    fn repositories_outside_the_workspace_never_run_but() {
        let repo = repo("ref: refs/heads/main\n");
        let tools = tempfile::tempdir().unwrap();
        let marker = tools.path().join("ran");
        let but = fake_but(tools.path(), &format!("touch '{}'", marker.display()));
        assert_eq!(query_with(&but, repo.path()).unwrap(), None);
        assert_eq!(query_with(&but, tools.path()).unwrap(), None);
        assert!(!marker.exists());
    }

    #[test]
    fn failures_report_but_errors() {
        let repo = repo("ref: refs/heads/gitbutler/workspace\n");
        let tools = tempfile::tempdir().unwrap();
        let but = fake_but(tools.path(), "echo 'workspace broken' >&2\nexit 1");
        assert_eq!(
            query_with(&but, repo.path()).unwrap_err(),
            "but status failed: workspace broken"
        );
        let missing = tools.path().join("missing-but");
        assert_eq!(
            query_with(missing.to_str().unwrap(), repo.path()).unwrap(),
            None
        );
    }
}
