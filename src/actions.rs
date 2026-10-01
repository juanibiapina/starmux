use crate::Snapshot;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const MODULES: &[&str] = &[
    "top",
    "sessions",
    "divider",
    "pi-workbench",
    "usage",
    "gob",
    "git",
    "debug",
    "pi-context",
    "spacer",
    "blank",
];
const COMMON: &[&str] = &[
    "socket", "client", "session", "window", "pane", "path", "module", "kind", "part",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Action {
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance: Option<usize>,
    #[serde(default, rename = "match", skip_serializing_if = "BTreeMap::is_empty")]
    selectors: BTreeMap<String, toml::Value>,
    action: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Identity {
    pub module: String,
    pub instance: usize,
    pub presentation: String,
    fields: BTreeMap<String, String>,
}
impl Identity {
    pub(crate) fn new(kind: &str) -> Self {
        Self {
            fields: BTreeMap::from([("kind".into(), kind.into()), ("part".into(), "main".into())]),
            ..Self::default()
        }
    }
    pub(crate) fn get(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }
    pub(crate) fn field(mut self, name: &str, value: impl ToString) -> Self {
        self.fields.insert(name.into(), value.to_string());
        self
    }
}

#[derive(Clone, Debug)]
enum Piece {
    Text(String),
    Field(String),
}
#[derive(Clone, Debug)]
struct Compiled {
    args: Vec<Vec<Piece>>,
    fields: BTreeSet<String>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Actions {
    definitions: BTreeMap<String, Compiled>,
    rules: Vec<Rule>,
    fingerprint: String,
}
pub(crate) enum Choice {
    Default,
    None,
    Unavailable(String),
    Command { token: String },
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn catalog(module: &str) -> Vec<(&'static str, &'static [&'static str])> {
    match module {
        "top" => vec![("heading", &[]), ("metric", &["metric"])],
        "gob" => vec![("heading", &[]), ("job", &["job_id", "name"])],
        "usage" => vec![
            ("provider", &["provider"]),
            ("cache-age", &["provider"]),
            ("window", &["provider", "label", "duration_seconds"]),
        ],
        "pi-context" => vec![
            ("heading", &[]),
            ("category", &["category"]),
            ("plan", &["file", "title"]),
            ("skill", &["file", "name"]),
            ("pr", &["url"]),
        ],
        "sessions" => vec![
            ("session", &["target_session", "name"]),
            (
                "window",
                &["target_session", "target_window", "index", "name"],
            ),
        ],
        "pi-workbench" => vec![
            ("project", &["project"]),
            (
                "session",
                &["project", "name", "target_pane", "target_window"],
            ),
        ],
        "git" => vec![("line", &["slot"]), ("summary", &[])],
        "debug" => vec![("total", &[]), ("stage", &["stage"])],
        "divider" => vec![("divider", &[])],
        "blank" => vec![("blank", &[])],
        "spacer" => vec![("padding", &["slot"])],
        name if name.starts_with("command.") => vec![("output", &[])],
        _ => vec![],
    }
}
fn template(text: &str) -> Result<Vec<Piece>, String> {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' if chars.peek() == Some(&c) => {
                chars.next();
                literal.push(c);
            }
            '{' => {
                if !literal.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut literal)));
                }
                let mut name = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    name.push(next);
                }
                if !closed || !valid_name(&name) {
                    return Err("invalid action placeholder".into());
                }
                pieces.push(Piece::Field(name));
            }
            '}' => return Err("unmatched brace in action argument".into()),
            _ => literal.push(c),
        }
    }
    if !literal.is_empty() {
        pieces.push(Piece::Text(literal));
    }
    Ok(pieces)
}
fn fingerprint(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{:012x}", hash & 0xffffffffffff)
}
impl Actions {
    pub(crate) fn compile(
        definitions: &BTreeMap<String, Action>,
        rules: &[Rule],
        configs: &BTreeSet<String>,
        commands: &BTreeSet<String>,
    ) -> Result<Self, String> {
        if definitions.len() > 64 || rules.len() > 128 {
            return Err("at most 64 actions and 128 click rules are allowed".into());
        }
        let mut compiled = BTreeMap::new();
        for (name, action) in definitions {
            if !valid_name(name) || matches!(name.as_str(), "default" | "none") {
                return Err(format!("invalid or reserved action name {name}"));
            }
            validate_args(&action.argv)?;
            if action.argv[0].contains(['{', '}']) {
                return Err("action executable must be literal".into());
            }
            let args = action
                .argv
                .iter()
                .map(|arg| template(arg))
                .collect::<Result<Vec<_>, _>>()?;
            let fields: BTreeSet<String> = args
                .iter()
                .flatten()
                .filter_map(|piece| {
                    if let Piece::Field(name) = piece {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let known: BTreeSet<_> = MODULES
                .iter()
                .flat_map(|module| catalog(module))
                .flat_map(|(_, fields)| fields.iter().copied())
                .chain(COMMON.iter().copied())
                .chain(["occurrence"])
                .collect();
            if let Some(field) = fields
                .iter()
                .find(|field: &&String| !known.contains(field.as_str()))
            {
                return Err(format!("unknown action placeholder {{{field}}}"));
            }
            compiled.insert(name.clone(), Compiled { args, fields });
        }
        for rule in rules {
            validate_rule(rule, &compiled, configs, commands)?;
        }
        let serialized =
            serde_json::to_string(&(definitions, rules)).map_err(|error| error.to_string())?;
        Ok(Self {
            definitions: compiled,
            rules: rules.to_vec(),
            fingerprint: fingerprint(&serialized),
        })
    }
    pub(crate) fn resolve(
        &self,
        identity: &Identity,
        snapshot: &Snapshot,
        config: &str,
    ) -> Result<Choice, String> {
        let Some(rule) = self
            .rules
            .iter()
            .rev()
            .find(|rule| rule.matches(identity, config))
        else {
            return Ok(Choice::Default);
        };
        match rule.action.as_str() {
            "default" => return Ok(Choice::Default),
            "none" => return Ok(Choice::None),
            _ => {}
        }
        let definition = &self.definitions[&rule.action];
        let mut fields = identity.fields.clone();
        let window = snapshot
            .sessions
            .iter()
            .find(|s| s.id == snapshot.current_session)
            .and_then(|s| s.windows.iter().find(|w| w.selected))
            .ok_or("missing focused window")?;
        fields.extend(BTreeMap::from([
            ("module".into(), identity.module.clone()),
            ("session".into(), snapshot.current_session.clone()),
            ("window".into(), window.id.clone()),
            ("pane".into(), snapshot.current_pane.clone()),
            ("path".into(), snapshot.pane_path.clone()),
        ]));
        // Socket and client are supplied only after token validation, not included in row snapshots.
        fields.insert("socket".into(), "{socket}".into());
        fields.insert("client".into(), "{client}".into());
        if let Some(field) = definition
            .fields
            .iter()
            .find(|field| !fields.contains_key(*field))
        {
            return Ok(Choice::Unavailable(format!(
                "action {}: unavailable placeholder {{{field}}}",
                rule.action
            )));
        }
        if let Err(error) = expand(definition, &fields) {
            return Ok(Choice::Unavailable(format!(
                "action {}: {error}",
                rule.action
            )));
        }
        let code = module_code(&identity.module);
        let token = format!(
            "sc{code}{}",
            fingerprint(&format!(
                "{}|{config}|{identity:?}|{}|{}|{}|{}|{}",
                self.fingerprint,
                snapshot.current_session,
                window.id,
                snapshot.current_pane,
                snapshot.pane_path,
                rule.action
            ))
        );
        Ok(Choice::Command { token })
    }
    pub(crate) fn instantiate(
        &self,
        identity: &Identity,
        snapshot: &Snapshot,
        context: (&str, &str, &str),
    ) -> Result<Option<Vec<String>>, String> {
        let (config, socket, client) = context;
        let Some(rule) = self
            .rules
            .iter()
            .rev()
            .find(|rule| rule.matches(identity, config))
        else {
            return Ok(None);
        };
        let Some(definition) = self.definitions.get(&rule.action) else {
            return Ok(None);
        };
        let Choice::Command { .. } = self.resolve(identity, snapshot, config)? else {
            return Ok(None);
        };
        let mut fields = identity.fields.clone();
        let window = snapshot
            .sessions
            .iter()
            .find(|s| s.id == snapshot.current_session)
            .and_then(|s| s.windows.iter().find(|w| w.selected))
            .ok_or("missing focused window")?;
        fields.extend(BTreeMap::from([
            ("socket".into(), socket.into()),
            ("client".into(), client.into()),
            ("module".into(), identity.module.clone()),
            ("session".into(), snapshot.current_session.clone()),
            ("window".into(), window.id.clone()),
            ("pane".into(), snapshot.current_pane.clone()),
            ("path".into(), snapshot.pane_path.clone()),
        ]));
        expand(definition, &fields).map(Some)
    }
}
impl Rule {
    fn matches(&self, identity: &Identity, config: &str) -> bool {
        self.module.as_ref().is_none_or(|m| m == &identity.module)
            && self.config.as_ref().is_none_or(|c| c == config)
            && self.instance.is_none_or(|n| n == identity.instance)
            && self.selectors.iter().all(|(key, value)| {
                identity
                    .fields
                    .get(key)
                    .is_some_and(|actual| actual == &selector_value(value))
            })
    }
}
fn selector_value(value: &toml::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn validate_rule(
    rule: &Rule,
    definitions: &BTreeMap<String, Compiled>,
    configs: &BTreeSet<String>,
    commands: &BTreeSet<String>,
) -> Result<(), String> {
    if rule.instance == Some(0)
        || rule
            .config
            .as_ref()
            .is_some_and(|c| c != "default" && !configs.contains(c))
    {
        return Err("invalid click config or instance".into());
    }
    let modules: Vec<_> = MODULES
        .iter()
        .map(|m| m.to_string())
        .chain(commands.iter().map(|c| format!("command.{c}")))
        .collect();
    if rule.module.as_ref().is_some_and(|m| !modules.contains(m)) {
        return Err("unknown click module".into());
    }
    let candidates: Vec<_> = modules
        .iter()
        .filter(|m| rule.module.as_ref().is_none_or(|selected| selected == *m))
        .flat_map(|m| catalog(m))
        .filter(|(kind, fields)| {
            rule.selectors
                .iter()
                .all(|(key, value)| match key.as_str() {
                    "kind" => value.as_str() == Some(kind),
                    "part" | "occurrence" => true,
                    _ => fields.contains(&key.as_str()),
                })
        })
        .collect();
    if candidates.is_empty() {
        return Err("incompatible click selectors".into());
    }
    for (key, value) in &rule.selectors {
        let numeric = matches!(
            key.as_str(),
            "duration_seconds" | "index" | "slot" | "occurrence"
        );
        if (numeric
            && value
                .as_integer()
                .is_none_or(|n| n < 0 || (key != "index" && n == 0)))
            || (!numeric && value.as_str().is_none())
        {
            return Err(format!("invalid click selector {key}"));
        }
    }
    for (key, accepted) in [
        ("metric", &["cpu", "memory", "battery"][..]),
        ("category", &["plans", "skills", "prs"][..]),
        ("part", &["main", "name", "progress"][..]),
        ("stage", &crate::debug::STAGES[..]),
        ("provider", crate::usage::PROVIDERS),
    ] {
        if rule
            .selectors
            .get(key)
            .and_then(toml::Value::as_str)
            .is_some_and(|value| !accepted.contains(&value))
        {
            return Err(format!("invalid click selector {key}"));
        }
    }
    if let Some(definition) = definitions.get(&rule.action) {
        for field in &definition.fields {
            if !COMMON.contains(&field.as_str())
                && field != "occurrence"
                && candidates
                    .iter()
                    .any(|(_, fields)| !fields.contains(&field.as_str()))
            {
                return Err(format!(
                    "placeholder {{{field}}} unavailable for click rule"
                ));
            }
        }
    } else if !matches!(rule.action.as_str(), "none" | "default") {
        return Err(format!("unknown click action {}", rule.action));
    }
    Ok(())
}
fn expand(definition: &Compiled, fields: &BTreeMap<String, String>) -> Result<Vec<String>, String> {
    let args = definition
        .args
        .iter()
        .map(|pieces| {
            pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => Ok(text.as_str()),
                    Piece::Field(name) => fields
                        .get(name)
                        .map(String::as_str)
                        .ok_or_else(|| format!("unavailable action placeholder {name}")),
                })
                .collect::<Result<String, String>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_args(&args)?;
    Ok(args)
}
fn validate_args(args: &[String]) -> Result<(), String> {
    if args.is_empty()
        || args.len() > 16
        || args[0].is_empty()
        || args.iter().any(|a| a.len() > 1024 || a.contains('\0'))
    {
        return Err("action argv requires 1–16 arguments, a nonempty executable, at most 1024 bytes per argument, and no NUL".into());
    }
    Ok(())
}
fn module_code(module: &str) -> char {
    MODULES
        .iter()
        .position(|m| *m == module)
        .map_or('l', |n| char::from(b'a' + n as u8))
}
pub(crate) fn token_module(token: &str) -> Result<&'static str, String> {
    let bytes = token.as_bytes();
    if bytes.len() != 15 || &bytes[..2] != b"sc" || !bytes[3..].iter().all(u8::is_ascii_hexdigit) {
        return Err("invalid action click target".into());
    }
    if bytes[2] == b'c' {
        return Ok("divider");
    }
    if bytes[2] == b'l' {
        return Ok("command");
    }
    MODULES
        .get(usize::from(bytes[2].wrapping_sub(b'a')))
        .copied()
        .ok_or_else(|| "invalid action module".into())
}

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
struct OutputFile(std::path::PathBuf);
impl Drop for OutputFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn run(args: &[String], directory: &Path) -> Result<(), String> {
    validate_args(args)?;
    let path = std::env::temp_dir().join(format!(
        "starmux-action-{}-{}",
        std::process::id(),
        NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("create action stderr: {error}"))?;
    let output = OutputFile(path);
    let mut child = Command::new(&args[0])
        .args(&args[1..])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(file))
        .spawn()
        .map_err(|error| format!("start click action: {error}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if fs::metadata(&output.0).is_ok_and(|file| file.len() > 16384) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("click action stderr exceeds 16 KiB".into());
            }
            Ok(None) if started.elapsed() < Duration::from_secs(2) => {
                thread::sleep(Duration::from_millis(5))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("click action timed out after 2 seconds".into());
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("wait for click action: {error}"));
            }
        }
    };
    let mut bytes = Vec::new();
    fs::File::open(&output.0)
        .and_then(|file| file.take(16385).read_to_end(&mut bytes))
        .map_err(|error| format!("read action stderr: {error}"))?;
    if bytes.len() > 16384 {
        return Err("click action stderr exceeds 16 KiB".into());
    }
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "click action exited with {status}: {}",
            String::from_utf8_lossy(&bytes).trim()
        ))
    }
}
