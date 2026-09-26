use serde::Deserialize;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const MAX_OUTPUT: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GobJob {
    pub id: String,
    pub name: String,
    pub started_at: Option<OffsetDateTime>,
    pub avg_duration_ms: u64,
}

impl GobJob {
    pub fn percent(&self, now: OffsetDateTime) -> Option<f64> {
        let started = self.started_at?;
        if self.avg_duration_ms == 0 {
            return None;
        }
        let elapsed = (now - started).whole_milliseconds().max(0) as f64;
        Some((elapsed / self.avg_duration_ms as f64 * 100.0).clamp(0.0, 100.0))
    }
}

#[derive(Deserialize)]
struct Record {
    id: String,
    status: String,
    command: Vec<String>,
    workdir: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    started_at: String,
    #[serde(default)]
    avg_duration_ms: i64,
}

fn parse(bytes: &[u8], workdir: &Path) -> Result<Vec<GobJob>, String> {
    let values: Vec<serde_json::Value> =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid gob list: {error}"))?;
    let mut jobs = Vec::new();
    for value in values {
        let Ok(record) = serde_json::from_value::<Record>(value) else {
            continue;
        };
        if record.status != "running" || Path::new(&record.workdir) != workdir {
            continue;
        }
        if record.id.is_empty() || record.id.len() > 128 {
            continue;
        }
        let name = if !record.description.trim().is_empty() {
            record.description
        } else {
            record
                .command
                .iter()
                .map(|arg| arg.chars().take(512).collect::<String>())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let name = if name.trim().is_empty() {
            record.id.clone()
        } else {
            name
        };
        jobs.push(GobJob {
            id: record.id,
            name: name.chars().take(1024).collect(),
            started_at: OffsetDateTime::parse(&record.started_at, &Rfc3339).ok(),
            avg_duration_ms: record.avg_duration_ms.max(0) as u64,
        });
        if jobs.len() == 100 {
            break;
        }
    }
    jobs.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(jobs)
}

pub(crate) fn list(workdir: &Path) -> Result<Vec<GobJob>, String> {
    list_with("gob", workdir)
}

fn list_with(binary: &str, workdir: &Path) -> Result<Vec<GobJob>, String> {
    if !workdir.is_dir() {
        return Ok(Vec::new());
    }
    let mut child = match Command::new(binary)
        .args(["list", "--json"])
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("gob list: {error}")),
    };
    let stdout = child.stdout.take().ok_or("gob stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("gob stderr unavailable")?;
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes);
        (result, bytes)
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(4096).read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = output.join();
                let _ = errors.join();
                return Err("gob list timed out".into());
            }
            Err(error) => return Err(format!("gob list: {error}")),
        }
    };
    let (read, bytes) = output.join().map_err(|_| "gob output reader failed")?;
    read.map_err(|error| format!("gob list output: {error}"))?;
    let stderr = errors.join().map_err(|_| "gob error reader failed")?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("gob list output exceeds 2 MiB".into());
    }
    if !status.success() {
        return Err(format!(
            "gob list failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    parse(&bytes, workdir)
}

#[cfg(test)]
mod tests {
    use super::list_with;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn gob_cli_filters_to_running_jobs_in_selected_directory() {
        let dir = std::env::temp_dir().join(format!("starmux-gob-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-gob");
        let json = serde_json::json!([
            {"id":"a", "status":"running", "command":["make","test"], "workdir":dir, "started_at":"2026-01-01T00:00:00Z", "avg_duration_ms":1000},
            {"id":"b", "status":"stopped", "command":["sleep","2"], "workdir":dir},
            {"id":"c", "status":"running", "command":["foreign"], "workdir":"/elsewhere"}
        ]);
        fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", json)).unwrap();
        let mut mode = fs::metadata(&script).unwrap().permissions();
        mode.set_mode(0o700);
        fs::set_permissions(&script, mode).unwrap();
        let jobs = list_with(script.to_str().unwrap(), &dir).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "make test");
        fs::remove_dir_all(dir).unwrap();
    }
}
