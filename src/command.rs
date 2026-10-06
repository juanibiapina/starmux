use crate::process::{self, Limits};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_millis(500);
const MAX_OUTPUT: u64 = 16 * 1024;
pub(crate) fn run(argv: &[String], workdir: &Path) -> Result<Option<String>, String> {
    if !workdir.is_dir() {
        return Ok(None);
    }
    let args: Vec<_> = argv[1..]
        .iter()
        .map(|arg| {
            if let Some(path) = arg.strip_prefix("~/") {
                let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
                Ok(PathBuf::from(home).join(path).into_os_string())
            } else {
                Ok(arg.into())
            }
        })
        .collect::<Result<_, &str>>()?;
    let mut command = Command::new(&argv[0]);
    command.args(args).current_dir(workdir);
    let limits = Limits {
        deadline: Instant::now() + TIMEOUT,
        stdout: MAX_OUTPUT,
        stderr: 0,
    };
    let output = process::run(command, limits).map_err(|error| match error {
        process::Error::Spawn(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "executable not found".to_owned()
        }
        process::Error::Spawn(error) => format!("start command: {error}"),
        process::Error::TimedOut => "command timed out after 500 ms".into(),
        process::Error::TooLarge(_) => "command output exceeds 16 KiB".into(),
        process::Error::Io(error) => format!("wait for command: {error}"),
    })?;
    if !output.status.success() {
        return Err(format!("command exited with {}", output.status));
    }
    let text = String::from_utf8(output.stdout).map_err(|_| "command output is not UTF-8")?;
    Ok(text
        .lines()
        .next()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(str::to_owned))
}
