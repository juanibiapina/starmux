use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_millis(500);
const MAX_OUTPUT: u64 = 16 * 1024;
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct OutputFile(PathBuf);

impl Drop for OutputFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn run(argv: &[String], workdir: &Path) -> Result<Option<String>, String> {
    if !workdir.is_dir() {
        return Ok(None);
    }
    let path = std::env::temp_dir().join(format!(
        "starmux-command-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("create command output: {error}"))?;
    let output = OutputFile(path);
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
    let mut child = match Command::new(&argv[0])
        .args(args)
        .current_dir(workdir)
        .stdout(Stdio::from(file))
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("executable not found".into());
        }
        Err(error) => return Err(format!("start command: {error}")),
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if fs::metadata(&output.0).is_ok_and(|file| file.len() > MAX_OUTPUT) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command output exceeds 16 KiB".into());
            }
            Ok(None) if started.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command timed out after 500 ms".into());
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wait for command: {error}"));
            }
        }
    };
    if !status.success() {
        return Err(format!("command exited with {status}"));
    }
    let mut bytes = Vec::new();
    fs::File::open(&output.0)
        .map_err(|error| format!("read command output: {error}"))?
        .take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read command output: {error}"))?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("command output exceeds 16 KiB".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "command output is not UTF-8")?;
    Ok(text
        .lines()
        .next()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(str::to_owned))
}
