use crate::{
    gitbutler::{GitButlerBranch, GitButlerCommit, PushStatus},
    pi_workbench::{BuildState, LiveEntry, PiContext},
    pr_state::PrState,
    Sidebar,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const MAX_DEPTH: usize = 64;

pub fn render(sidebar: &Sidebar, dir: &Path, pid: Option<u32>) -> Result<String, String> {
    let gitbutler = if sidebar.gitbutler_enabled() {
        crate::gitbutler::query(dir).ok().flatten()
    } else {
        None
    };
    let git = if sidebar.git_enabled() && gitbutler.is_none() {
        crate::git::query(dir).ok().flatten()
    } else {
        None
    };
    let value = json!({
        "version": 1,
        "pi": pid.and_then(|pid| pi(sidebar, pid)),
        "jobs": sidebar.gob_enabled().then(|| crate::gob::list(dir).ok()).flatten().map(jobs),
        "git": git.map(git_json),
        "gitbutler": gitbutler.map(|status| json!({
            "base": status.base.as_ref().map(commit),
            "behind": status.behind,
            "unassignedChanges": status.unassigned_changes,
            "conflictedFiles": status.conflicted_files,
            "branches": status.branches.iter().map(branch).collect::<Vec<_>>(),
        })),
        "system": system(sidebar),
        "usage": usage(sidebar),
    });
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

fn pi(sidebar: &Sidebar, pid: u32) -> Option<Value> {
    let data_dir = sidebar.pi_workbench_data_dir()?;
    let data_dir = if data_dir.is_empty() {
        crate::pi_workbench::default_data_dir().ok()?
    } else {
        PathBuf::from(data_dir)
    };
    let entries = crate::pi_workbench::list(&data_dir, None).ok()?;
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let entry = entries
        .iter()
        .filter(|entry| descends_from(&system, entry.pid, pid))
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))?;
    let context = sidebar
        .pi_context_enabled()
        .then(|| context(sidebar, entry))
        .flatten();
    Some(json!({
        "session": {
            "id": entry.session_id,
            "name": entry.session.name,
            "state": entry.session.state,
            "project": entry.session.project,
        },
        "context": context,
    }))
}

fn descends_from(system: &System, pid: u32, ancestor: u32) -> bool {
    let mut current = Some(Pid::from_u32(pid));
    for _ in 0..MAX_DEPTH {
        let Some(pid) = current else {
            return false;
        };
        if pid.as_u32() == ancestor {
            return true;
        }
        current = system.process(pid).and_then(sysinfo::Process::parent);
    }
    false
}

fn context(sidebar: &Sidebar, entry: &LiveEntry) -> Option<Value> {
    let path = entry.context_path.as_ref()?;
    let mut context: PiContext = crate::pi_workbench::read_context(path, &entry.session_id).ok()?;
    if let Some(dir) = sidebar.cache_dir("pr-state") {
        for pull_request in &mut context.pull_requests {
            let cached = crate::pr_state::resolve(&pull_request.url, &dir, true);
            if cached != PrState::Unknown {
                pull_request.state = cached;
            }
        }
    }
    Some(json!({
        "plans": context.plans.iter().map(|plan| json!({
            "title": plan.title,
            "path": plan.path,
            "url": sidebar.plan_browser_url(&entry.session_id, &plan.path).ok().flatten(),
        })).collect::<Vec<_>>(),
        "pullRequests": context.pull_requests.iter().map(|pull_request| json!({
            "url": pull_request.url,
            "state": pr_state(pull_request.state),
            "build": pull_request.build.map(build_state),
        })).collect::<Vec<_>>(),
        "builds": context.builds.iter().map(|build| json!({
            "repository": build.repository,
            "branch": build.branch,
            "url": build.url,
            "state": build.state.map_or("none", build_state),
        })).collect::<Vec<_>>(),
        "skills": context.skills.iter().map(|skill| json!({
            "name": skill.name,
            "path": skill.path,
        })).collect::<Vec<_>>(),
    }))
}

fn jobs(jobs: Vec<crate::GobJob>) -> Value {
    let now = time::OffsetDateTime::now_utc();
    Value::Array(
        jobs.iter()
            .map(|job| {
                let progress = job.progress(now);
                json!({
                    "id": job.id,
                    "name": job.name,
                    "elapsedMs": progress.map(|p| p.elapsed.as_millis() as u64),
                    "typicalMs": progress.map(|p| p.typical.as_millis() as u64),
                    "upperMs": progress.map(|p| p.upper.as_millis() as u64),
                })
            })
            .collect(),
    )
}

fn git_json(status: crate::GitStatus) -> Value {
    json!({
        "branch": status.branch,
        "upstream": status.upstream,
        "ahead": status.ahead,
        "behind": status.behind,
        "staged": status.staged,
        "modified": status.modified,
        "untracked": status.untracked,
        "conflicts": status.conflicts,
        "stash": status.stash,
        "added": status.added,
        "deleted": status.deleted,
        "state": status.state,
        "unborn": status.unborn,
        "clean": status.clean(),
    })
}

fn commit(commit: &GitButlerCommit) -> Value {
    json!({ "id": commit.id, "message": commit.message, "conflicted": commit.conflicted })
}

fn branch(branch: &GitButlerBranch) -> Value {
    json!({
        "name": branch.name,
        "depth": branch.depth,
        "commits": branch.commits,
        "recentCommits": branch.recent_commits.iter().map(commit).collect::<Vec<_>>(),
        "upstreamCommits": branch.upstream_commits,
        "changes": branch.changes,
        "push": match branch.push {
            PushStatus::Pushed => "pushed",
            PushStatus::Unpushed => "unpushed",
            PushStatus::NeedsForce => "force",
            PushStatus::Local => "local",
            PushStatus::Integrated => "integrated",
        },
        "conflicted": branch.conflicted,
        "review": branch.review,
        "ci": branch.ci.map(build_state),
    })
}

fn system(sidebar: &Sidebar) -> Option<Value> {
    if !sidebar.top_enabled() {
        return None;
    }
    let status = crate::top::resolve(&sidebar.cache_dir("top")?, true);
    Some(json!({
        "cpu": status.cpu,
        "memory": status.memory,
        "battery": status.battery.map(|battery| json!({
            "percent": battery.percent,
            "charging": battery.charging,
            "full": battery.full,
        })),
    }))
}

fn usage(sidebar: &Sidebar) -> Option<Value> {
    let (providers, data_dir) = sidebar.usage_options()?;
    if providers.is_empty() {
        return None;
    }
    let dir = match data_dir {
        Some(path) => PathBuf::from(path),
        None => sidebar.cache_dir("usage")?,
    };
    let rows = crate::usage::resolve(providers, &dir, true);
    Some(Value::Array(
        rows.iter()
            .map(|row| {
                json!({
                    "provider": row.provider,
                    "name": row.display_name,
                    "stale": row.stale,
                    "unavailable": row.unavailable,
                    "fetchedAt": row.fetched_at,
                    "failure": row.refresh_failure.map(|failure| match failure {
                        crate::usage::RefreshFailure::SignInAgain => "sign_in_again",
                        crate::usage::RefreshFailure::RefreshFailed => "refresh_failed",
                    }),
                    "windows": row.windows,
                })
            })
            .collect(),
    ))
}

const fn build_state(state: BuildState) -> &'static str {
    match state {
        BuildState::Pending => "pending",
        BuildState::Success => "success",
        BuildState::Failure => "failure",
    }
}

const fn pr_state(state: PrState) -> &'static str {
    match state {
        PrState::Open => "open",
        PrState::Draft => "draft",
        PrState::Merged => "merged",
        PrState::Closed => "closed",
        PrState::Unknown => "unknown",
    }
}
