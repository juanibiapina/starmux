use crate::{
    config::{Config, Provider},
    input::Context,
    view::{Row, Span, Style},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

static CANCEL: AtomicBool = AtomicBool::new(false);
const MAX_OUTPUT: u64 = 65536;
const PANE_PATH_PLACEHOLDER: &str = "{pane.path}";
const SESSION_ID_PLACEHOLDER: &str = "{session.id}";

extern "C" fn cancel(_signal: libc::c_int) {
    CANCEL.store(true, Ordering::SeqCst);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Facts {
    Plain(String),
    Json(JsonRows),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRows {
    pub version: u32,
    pub rows: Vec<JsonRow>,
    pub metadata: Option<serde_json::Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonRow {
    pub segments: Vec<JsonSegment>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonSegment {
    pub text: String,
    pub style: String,
    pub fill: Option<String>,
}

fn substitute(value: &str, ctx: &Context) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix(PANE_PATH_PLACEHOLDER) {
            result.push_str(&ctx.pane_path);
            rest = next;
        } else if let Some(next) = rest.strip_prefix(SESSION_ID_PLACEHOLDER) {
            result.push_str(&ctx.current_session);
            rest = next;
        } else {
            let ch = rest.chars().next().unwrap();
            result.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    result
}

fn cache_path(name: &str, config: &Provider, ctx: &Context) -> Option<PathBuf> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dependencies: Vec<_> = config
        .dependencies
        .iter()
        .map(|s| substitute(&format!("{{{s}}}"), ctx))
        .collect();
    let key = serde_json::to_vec(&(1, name, config, dependencies)).ok()?;
    let hash = Sha256::digest(key);
    Some(
        root.join("starmux/providers")
            .join(format!("{hash:x}.json")),
    )
}

pub fn cached(
    cfg: &Config,
    ctx: &Context,
    names: &[String],
) -> (BTreeMap<String, Facts>, Vec<String>) {
    let mut hits = BTreeMap::new();
    let mut stale = Vec::new();
    for name in names {
        let Some(provider) = cfg.provider.get(name) else {
            continue;
        };
        if provider.disabled {
            continue;
        }
        if provider.cache {
            if let Some(path) = cache_path(name, provider, ctx) {
                if let Ok(bytes) = fs::read(path) {
                    if bytes.len() <= MAX_OUTPUT as usize {
                        if let Ok(facts) = serde_json::from_slice(&bytes) {
                            hits.insert(name.clone(), facts);
                        }
                    }
                }
            }
        }
        stale.push(name.clone());
    }
    (hits, stale)
}

fn decode(config: &Provider, bytes: Vec<u8>) -> Result<Facts, String> {
    let text = String::from_utf8(bytes).map_err(|_| "invalid UTF-8".to_string())?;
    match config.decoder.as_str() {
        "plain" => Ok(Facts::Plain(text.trim_end_matches('\n').to_owned())),
        "json-v1" => {
            let json: JsonRows = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if json.version != 1 {
                return Err("unsupported json-v1 version".into());
            }
            if json.rows.len() > 1000 || json.rows.iter().any(|r| r.segments.len() > 100) {
                return Err("too many rows or segments".into());
            }
            for segment in json.rows.iter().flat_map(|r| &r.segments) {
                if !matches!(segment.style.as_str(), "default" | "footer" | "detail")
                    || segment.fill.as_ref().is_some_and(|fill| {
                        !matches!(
                            fill.as_str(),
                            "background" | "highlight" | "border" | "current"
                        )
                    })
                {
                    return Err("unknown semantic style or fill".into());
                }
            }
            Ok(Facts::Json(json))
        }
        _ => Err("unknown decoder".into()),
    }
}

fn execute(name: &str, provider: &Provider, ctx: &Context) -> Result<Facts, String> {
    if provider.command.is_empty() {
        return Err("empty command".into());
    }
    let argv: Vec<_> = provider
        .command
        .iter()
        .map(|s| substitute(s, ctx))
        .collect();
    let mut command = if let Some(shell) = &provider.shell {
        let mut cmd = Command::new(shell);
        cmd.arg("-c").arg(argv.join(" "));
        cmd
    } else {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        cmd
    };
    command
        .current_dir(substitute(&provider.cwd, ctx))
        .env("STARMUX_PANE_PATH", &ctx.pane_path)
        .env("STARMUX_SESSION_ID", &ctx.current_session)
        .env("STARMUX_PROVIDER", name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let pid = child.id() as i32;
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(MAX_OUTPUT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        if CANCEL.load(Ordering::SeqCst)
            || started.elapsed() >= Duration::from_millis(provider.timeout_ms)
        {
            #[cfg(unix)]
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("cancelled or timed out".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let bytes = reader
        .join()
        .map_err(|_| "reader failed")?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("exit status {status}"));
    }
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("output exceeds 65536 bytes".into());
    }
    decode(provider, bytes)
}

pub fn hide_failed(
    cfg: &Config,
    ctx: &Context,
    names: &[String],
    refreshed: &BTreeMap<String, Facts>,
) {
    for name in names {
        if refreshed.contains_key(name) {
            continue;
        }
        if let Some(provider) = cfg.provider.get(name).filter(|p| p.failure == "hide") {
            if let Some(path) = cache_path(name, provider, ctx) {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub fn run(cfg: &Config, ctx: &Context, names: &[String]) -> BTreeMap<String, Facts> {
    if names.is_empty() {
        return BTreeMap::new();
    }
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTERM, cancel as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, cancel as *const () as libc::sighandler_t);
    }
    std::thread::scope(|scope| {
        let tasks: Vec<_> = names
            .iter()
            .filter_map(|name| {
                cfg.provider
                    .get(name)
                    .filter(|p| !p.disabled)
                    .map(|p| (name.clone(), p))
            })
            .map(|(name, config)| {
                scope.spawn(move || {
                    let result = execute(&name, config, ctx);
                    if let Ok(facts) = &result {
                        if config.cache {
                            if let Some(path) = cache_path(&name, config, ctx) {
                                if let Ok(serialized) = serde_json::to_vec(facts) {
                                    if serialized.len() <= MAX_OUTPUT as usize
                                        && fs::create_dir_all(path.parent().unwrap()).is_ok()
                                    {
                                        let temp = path
                                            .with_extension(format!("{}.tmp", std::process::id()));
                                        if fs::write(&temp, serialized).is_ok() {
                                            let _ = fs::rename(&temp, &path);
                                        }
                                        let _ = fs::remove_file(temp);
                                    }
                                }
                            }
                        }
                    } else if let Err(error) = &result {
                        eprintln!("starmux provider {name}: {error}");
                    }
                    (name, result)
                })
            })
            .collect();
        tasks
            .into_iter()
            .filter_map(|task| {
                task.join()
                    .ok()
                    .and_then(|(name, result)| result.ok().map(|value| (name, value)))
            })
            .collect()
    })
}

pub fn rows(facts: &Facts) -> Vec<Row> {
    match facts {
        Facts::Plain(text) => text
            .split('\n')
            .map(|s| Row::new().text(s, Style::Footer))
            .collect(),
        Facts::Json(json) => json
            .rows
            .iter()
            .map(|row| {
                let mut result = Row::new();
                for segment in &row.segments {
                    let style = match segment.style.as_str() {
                        "detail" => Style::Detail,
                        _ => Style::Footer,
                    };
                    result.spans.push(Span {
                        text: segment.text.clone(),
                        style,
                    });
                    if let Some(name) = segment.fill.as_deref() {
                        result.fill = Some(
                            match name {
                                "highlight" => "#292e42",
                                "border" => "#3b4261",
                                "current" => "#c099ff",
                                _ => "#1b1d2b",
                            }
                            .to_owned(),
                        );
                    }
                }
                result
            })
            .collect(),
    }
}
