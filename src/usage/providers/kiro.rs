use super::*;
use crate::process::{self, Limits};
use std::{
    process::{Command, Output, Stdio},
    time::Instant,
};

fn run_kiro(args: &[&str], timeout: Duration) -> Result<Output, FetchError> {
    let mut command = Command::new("kiro-cli");
    command
        .args(args)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null());
    let limits = Limits {
        deadline: Instant::now() + timeout,
        stdout: 1024 * 1024,
        stderr: 0,
    };
    process::run(command, limits).map_err(|_| failure())
}

pub(super) fn strip_ansi(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for code in chars.by_ref() {
                        if ('@'..='~').contains(&code) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    for code in chars.by_ref() {
                        if code == '\u{7}' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub(super) fn fetch() -> Result<UsageSnapshot, FetchError> {
    let whoami = run_kiro(&["whoami"], Duration::from_secs(5))?;
    if !whoami.status.success() {
        return Err(failure());
    }
    let output = run_kiro(
        &["chat", "--no-interactive", "/usage"],
        Duration::from_secs(10),
    )?;
    if !output.status.success() {
        return Err(failure());
    }
    let plain = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    let percent = plain
        .split('█')
        .next_back()
        .and_then(|s| s.trim_start().split('%').next())
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            let start = plain.find("covered in plan")?;
            let preceding = &plain[..start];
            let open = preceding.rfind('(')?;
            let (used, total) = preceding[open + 1..].split_once(" of ")?;
            let total: f64 = total.trim().parse().ok()?;
            if total > 0. {
                Some(used.trim().parse::<f64>().ok()? / total * 100.)
            } else {
                None
            }
        })
        .unwrap_or(0.);
    Ok(snapshot(
        "kiro",
        "Kiro Plan",
        vec![window("Credits", percent)],
        None,
    ))
}
