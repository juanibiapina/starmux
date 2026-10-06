use crate::process::{self, Limits};
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

const LIMIT: u64 = 2 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, Default)]
pub struct GitStatus {
    pub branch: String,
    pub upstream: String,
    pub ahead: u64,
    pub behind: u64,
    pub staged: u64,
    pub modified: u64,
    pub untracked: u64,
    pub conflicts: u64,
    pub stash: u64,
    pub added: u64,
    pub deleted: u64,
    pub state: String,
    pub unborn: bool,
}

impl GitStatus {
    pub fn clean(&self) -> bool {
        self.staged == 0
            && self.modified == 0
            && self.untracked == 0
            && self.conflicts == 0
            && self.state.is_empty()
    }
}

fn git(dir: &Path, args: &[&str], started: Instant) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command
        .args(["--no-optional-locks", "-c", "color.ui=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0");
    let limits = Limits {
        deadline: started + DEADLINE,
        stdout: LIMIT,
        stderr: 0,
    };
    let output = process::run(command, limits).map_err(|error| match error {
        process::Error::Spawn(error) => format!("git: {error}"),
        process::Error::TimedOut => "git timed out".into(),
        process::Error::TooLarge(_) => "git output exceeds 2 MiB".into(),
        process::Error::Io(error) => format!("git wait: {error}"),
    })?;
    if !output.status.success() {
        return Err(format!("git {} exited with {}", args[0], output.status));
    }
    Ok(output.stdout)
}

pub(crate) fn query(dir: &Path) -> Result<Option<GitStatus>, String> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let started = Instant::now();
    let output = match git(
        dir,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--show-stash",
            "--untracked-files=normal",
        ],
        started,
    ) {
        Ok(output) => output,
        Err(error) if error.contains("exited with") || error.contains("No such file") => {
            return Ok(None)
        }
        Err(error) => return Err(error),
    };
    let mut result = parse_status(&output)?;
    if result.branch.is_empty() {
        return Ok(None);
    }
    let diff = if result.unborn {
        // An unborn branch has no HEAD to compare against.
        let staged = git(dir, &["diff", "--cached", "--numstat", "-z"], started)?;
        let unstaged = git(dir, &["diff", "--numstat", "-z"], started)?;
        [staged, unstaged].concat()
    } else {
        git(dir, &["diff", "HEAD", "--numstat", "-z"], started)?
    };
    parse_numstat(&diff, &mut result)?;
    if let Ok(path) = git(dir, &["rev-parse", "--absolute-git-dir"], started) {
        let git_dir = std::path::PathBuf::from(String::from_utf8_lossy(&path).trim());
        for (entry, state) in [
            ("rebase-merge", "rebase"),
            ("rebase-apply", "rebase"),
            ("MERGE_HEAD", "merge"),
            ("CHERRY_PICK_HEAD", "cherry-pick"),
            ("REVERT_HEAD", "revert"),
        ] {
            if git_dir.join(entry).exists() {
                result.state = state.into();
                break;
            }
        }
    }
    Ok(Some(result))
}

fn parse_status(bytes: &[u8]) -> Result<GitStatus, String> {
    let mut result = GitStatus::default();
    let mut oid = String::new();
    let mut records = bytes.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        if let Some(rest) = record.strip_prefix(b"# branch.head ") {
            result.branch = String::from_utf8_lossy(rest).into_owned();
        } else if let Some(rest) = record.strip_prefix(b"# branch.oid ") {
            oid = String::from_utf8_lossy(rest).into_owned();
        } else if let Some(rest) = record.strip_prefix(b"# branch.upstream ") {
            result.upstream = String::from_utf8_lossy(rest).into_owned();
        } else if let Some(rest) = record.strip_prefix(b"# branch.ab ") {
            let s = String::from_utf8_lossy(rest);
            let mut parts = s.split_whitespace();
            result.ahead = parts
                .next()
                .and_then(|v| v.strip_prefix('+'))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            result.behind = parts
                .next()
                .and_then(|v| v.strip_prefix('-'))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        } else if let Some(rest) = record.strip_prefix(b"# stash ") {
            result.stash = String::from_utf8_lossy(rest).parse().unwrap_or(0);
        } else if record.starts_with(b"? ") {
            result.untracked += 1;
        } else if record.starts_with(b"u ") {
            result.conflicts += 1;
        } else if record.starts_with(b"1 ") || record.starts_with(b"2 ") {
            let xy = &record[2..4];
            if xy[0] != b'.' {
                result.staged += 1;
            }
            if xy[1] != b'.' {
                result.modified += 1;
            }
            if record.starts_with(b"2 ") {
                records.next();
            } // rename's second path
        }
    }
    result.unborn = oid == "(initial)";
    if result.branch == "(detached)" {
        result.branch = format!(":{}", oid.chars().take(7).collect::<String>());
    }
    if result.branch == "(unknown)" {
        result.branch.clear();
    }
    Ok(result)
}

fn parse_numstat(bytes: &[u8], status: &mut GitStatus) -> Result<(), String> {
    let mut chunks = bytes.split(|b| *b == 0);
    while let Some(chunk) = chunks.next() {
        if chunk.is_empty() {
            continue;
        }
        let mut fields = chunk.splitn(3, |b| *b == b'\t');
        let added = fields.next().ok_or("invalid git numstat")?;
        let deleted = fields.next().ok_or("invalid git numstat")?;
        let path = fields.next().ok_or("invalid git numstat")?;
        if path.is_empty() {
            chunks.next();
            chunks.next();
        } // rename paths
        if let (Ok(a), Ok(d)) = (std::str::from_utf8(added), std::str::from_utf8(deleted)) {
            status.added = status.added.saturating_add(a.parse::<u64>().unwrap_or(0));
            status.deleted = status.deleted.saturating_add(d.parse::<u64>().unwrap_or(0));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::query;
    use std::{fs, path::Path, process::Command};

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn repository_summary_tracks_changes_and_stash() {
        let dir = std::env::temp_dir().join(format!("starmux-git-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("file"), "first\n").unwrap();
        let unborn = query(&dir).unwrap().unwrap();
        assert_eq!(unborn.branch, "main");
        assert_eq!(unborn.untracked, 1);
        git(&dir, &["add", "file"]);
        assert_eq!(query(&dir).unwrap().unwrap().added, 1);
        git(
            &dir,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@example.org",
                "commit",
                "-qm",
                "initial",
            ],
        );
        assert!(query(&dir).unwrap().unwrap().clean());
        fs::write(dir.join("file"), "changed\nmore\n").unwrap();
        let modified = query(&dir.join("nested")).unwrap().unwrap();
        assert_eq!(
            (modified.modified, modified.added, modified.deleted),
            (1, 2, 1)
        );
        git(&dir, &["add", "file"]);
        let staged = query(&dir).unwrap().unwrap();
        assert_eq!(
            (staged.staged, staged.modified, staged.added, staged.deleted),
            (1, 0, 2, 1)
        );
        git(&dir, &["stash", "push", "-q"]);
        let stashed = query(&dir).unwrap().unwrap();
        assert_eq!(stashed.stash, 1);
        assert!(stashed.clean());
        git(&dir, &["branch", "upstream"]);
        git(&dir, &["branch", "--set-upstream-to=upstream", "main"]);
        fs::write(dir.join("file"), "another\n").unwrap();
        git(&dir, &["add", "file"]);
        git(
            &dir,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@example.org",
                "commit",
                "-qm",
                "ahead",
            ],
        );
        let ahead = query(&dir).unwrap().unwrap();
        assert_eq!(ahead.upstream, "upstream");
        assert_eq!((ahead.ahead, ahead.behind), (1, 0));
        git(&dir, &["checkout", "-q", "--detach"]);
        assert!(query(&dir).unwrap().unwrap().branch.starts_with(':'));
        fs::remove_dir_all(dir).unwrap();
    }
}
