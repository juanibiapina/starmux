use crate::{Session, Sidebar, Snapshot, Window};
use std::{collections::BTreeMap, process::Command, time::Instant};

#[cfg(test)]
use std::sync::{Arc, Mutex};

const MAX_WIDTH: usize = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    pub session: String,
    pub window: String,
}

impl Focus {
    pub fn new(session: impl Into<String>, window: impl Into<String>) -> Result<Self, String> {
        let session = session.into();
        let window = window.into();
        if !valid_id(&session, '$') || !valid_id(&window, '@') {
            return Err("invalid expected focus".into());
        }
        Ok(Self { session, window })
    }

    fn verify(&self, snapshot: &Snapshot) -> Result<(), String> {
        let selected = snapshot
            .sessions
            .iter()
            .find(|session| session.id == snapshot.current_session)
            .and_then(|session| session.windows.iter().find(|window| window.selected));
        if snapshot.current_session != self.session
            || selected.is_none_or(|window| window.id != self.window)
        {
            return Err("tmux focus changed during query".into());
        }
        Ok(())
    }
}

pub trait Tmux {
    fn snapshot(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        window_options: &[(String, String)],
    ) -> Result<Snapshot, String>;

    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessTmux;

impl Tmux for ProcessTmux {
    fn snapshot(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        window_options: &[(String, String)],
    ) -> Result<Snapshot, String> {
        if socket.is_empty() || client.is_empty() {
            return Err("missing tmux socket or client name".into());
        }
        validate_width(width)?;
        let output = Command::new("tmux")
            .args(["-S", socket, "display-message", "-p", "-c", client, "--"])
            .arg(transport(window_options))
            .output()
            .map_err(|error| format!("tmux query failed: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "tmux query failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if output.stdout.len() > 128 * 1024 {
            return Err("tmux snapshot exceeds 128 KiB".into());
        }
        let output = String::from_utf8(output.stdout).map_err(|_| "tmux snapshot is not UTF-8")?;
        parse_snapshot(&words(&output)?, width, window_options)
    }

    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        if socket.is_empty() || client.is_empty() {
            return Err("missing tmux socket or client name".into());
        }
        let target = crate::navigation::target(token)?;
        let output = Command::new("tmux")
            .args(["-S", socket, "switch-client", "-c", client, "-t", &target])
            .output()
            .map_err(|error| format!("tmux switch failed: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "tmux switch failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct MemoryTmux {
    snapshot: Snapshot,
    activations: Arc<Mutex<Vec<(String, String, String)>>>,
}

#[cfg(test)]
impl MemoryTmux {
    fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            activations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn activations(&self) -> Vec<(String, String, String)> {
        self.activations.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Tmux for MemoryTmux {
    fn snapshot(
        &self,
        _socket: &str,
        _client: &str,
        width: usize,
        window_options: &[(String, String)],
    ) -> Result<Snapshot, String> {
        validate_width(width)?;
        let aliases: BTreeMap<_, _> = window_options.iter().cloned().collect();
        let mut snapshot = self.snapshot.clone();
        snapshot.width = width;
        for window in snapshot
            .sessions
            .iter_mut()
            .flat_map(|session| &mut session.windows)
        {
            window.options.retain(|name, _| aliases.contains_key(name));
        }
        Ok(snapshot)
    }

    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        crate::navigation::target(token)?;
        self.activations.lock().unwrap().push((
            socket.to_owned(),
            client.to_owned(),
            token.to_owned(),
        ));
        Ok(())
    }
}

pub struct Application<T> {
    sidebar: Sidebar,
    tmux: T,
}

impl<T: Tmux> Application<T> {
    pub fn new(sidebar: Sidebar, tmux: T) -> Self {
        Self { sidebar, tmux }
    }

    pub fn render_query(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        focus: Option<&Focus>,
    ) -> Result<String, String> {
        let snapshot = self.snapshot(socket, client, width, focus)?;
        self.sidebar.render(&snapshot)
    }

    pub fn explain(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        focus: Option<&Focus>,
    ) -> Result<String, String> {
        let snapshot = self.snapshot(socket, client, width, focus)?;
        let mut result = format!(
            "modules: {}\nclient: {}x{} pane {}\n",
            self.sidebar.module_names().join(", "),
            snapshot.client_width,
            snapshot.client_height,
            snapshot.current_pane
        );
        for session in &snapshot.sessions {
            result.push_str(&format!("session {} {:?}\n", session.id, session.name));
            for window in &session.windows {
                result.push_str(&format!(
                    "  window {} index={} pane={} path={:?} selected={}\n",
                    window.id, window.index, window.pane, window.path, window.selected
                ));
            }
        }
        Ok(result)
    }

    pub fn timings(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        focus: Option<&Focus>,
    ) -> Result<String, String> {
        let started = Instant::now();
        let snapshot = self.snapshot(socket, client, width, focus)?;
        let queried = started.elapsed().as_micros();
        self.sidebar.render(&snapshot)?;
        Ok(format!(
            "{{\"query_us\":{queried},\"render_us\":{}}}",
            started.elapsed().as_micros().saturating_sub(queried)
        ))
    }

    pub fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        self.tmux.activate(socket, client, token)
    }

    fn snapshot(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        focus: Option<&Focus>,
    ) -> Result<Snapshot, String> {
        validate_width(width)?;
        let snapshot = self.tmux.snapshot(
            socket,
            client,
            width,
            &self.sidebar.requested_window_options(),
        )?;
        if let Some(focus) = focus {
            focus.verify(&snapshot)?;
        }
        Ok(snapshot)
    }
}

fn validate_width(width: usize) -> Result<(), String> {
    if width == 0 || width > MAX_WIDTH {
        Err(format!("width must be 1..{MAX_WIDTH}"))
    } else {
        Ok(())
    }
}

fn transport(window_options: &[(String, String)]) -> String {
    let session = "--session-id=#{q/s:session_id} --session-name=#{q/s:session_name} --window-count=#{session_windows}";
    let mut window = String::from("--window-id=#{window_id} --window-index=#{window_index} --window-name=#{q/s:window_name} --selected=#{window_active} --window-pane=#{pane_id} --window-path=#{q/s:pane_current_path}");
    for (alias, option) in window_options {
        window.push_str(&format!(" --window-option-{alias}=#{{q/s:{option}}}"));
    }
    window.push_str(" --end-window");
    format!("--input-version=2 --client-width=#{{client_width}} --client-height=#{{client_height}} --current-session=#{{q/s:session_id}} --current-pane=#{{pane_id}} --pane-path=#{{q/s:pane_current_path}} --session-count=#{{server_sessions}} #{{S:{session} #{{W:{window} }} --end-session }}")
}

struct Fields<'a> {
    values: &'a [String],
    cursor: usize,
}

impl Fields<'_> {
    fn take(&mut self, key: &str) -> Result<String, String> {
        let value = self
            .values
            .get(self.cursor)
            .ok_or_else(|| format!("missing --{key}"))?;
        self.cursor += 1;
        value
            .strip_prefix(&format!("--{key}="))
            .map(str::to_owned)
            .ok_or_else(|| format!("expected --{key}= at argument {}", self.cursor))
    }

    fn marker(&mut self, key: &str) -> Result<(), String> {
        let value = self
            .values
            .get(self.cursor)
            .ok_or_else(|| format!("missing --{key}"))?;
        self.cursor += 1;
        if value == &format!("--{key}") {
            Ok(())
        } else {
            Err(format!("expected --{key}"))
        }
    }

    fn number(&mut self, key: &str, max: usize) -> Result<usize, String> {
        let value = self.take(key)?;
        let number = value
            .parse::<usize>()
            .map_err(|_| format!("invalid --{key}"))?;
        if number > max {
            Err(format!("--{key} exceeds {max}"))
        } else {
            Ok(number)
        }
    }

    fn flag(&mut self, key: &str) -> Result<bool, String> {
        match self.take(key)?.as_str() {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(format!("invalid --{key}: expected 0 or 1")),
        }
    }

    fn id(&mut self, key: &str, prefix: char) -> Result<String, String> {
        let value = self.take(key)?;
        if valid_id(&value, prefix) {
            Ok(value)
        } else {
            Err(format!("invalid --{key}"))
        }
    }
}

fn valid_id(value: &str, prefix: char) -> bool {
    value.strip_prefix(prefix).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn parse_snapshot(
    values: &[String],
    width: usize,
    window_options: &[(String, String)],
) -> Result<Snapshot, String> {
    let mut fields = Fields { values, cursor: 0 };
    if fields.take("input-version")? != "2" {
        return Err("unsupported tmux snapshot version".into());
    }
    let client_width = fields.number("client-width", 10000)?;
    let client_height = fields.number("client-height", 10000)?;
    let current_session = fields.id("current-session", '$')?;
    let current_pane = fields.id("current-pane", '%')?;
    let pane_path = fields.take("pane-path")?;
    let session_count = fields.number("session-count", 10000)?;
    let mut sessions = Vec::with_capacity(session_count);
    let mut window_total = 0;
    for _ in 0..session_count {
        let id = fields.id("session-id", '$')?;
        let name = fields.take("session-name")?;
        let window_count = fields.number("window-count", 10000)?;
        window_total += window_count;
        if window_total > 10000 {
            return Err("too many windows".into());
        }
        let mut windows = Vec::with_capacity(window_count);
        for _ in 0..window_count {
            let window_id = fields.id("window-id", '@')?;
            let index = fields.number("window-index", 1_000_000)?;
            let window_name = fields.take("window-name")?;
            let selected = fields.flag("selected")?;
            let pane = fields.id("window-pane", '%')?;
            let path = fields.take("window-path")?;
            let mut options = BTreeMap::new();
            for (alias, _) in window_options {
                options.insert(
                    alias.clone(),
                    fields.take(&format!("window-option-{alias}"))?,
                );
            }
            fields.marker("end-window")?;
            windows.push(Window {
                id: window_id,
                index,
                name: window_name,
                selected,
                pane,
                path,
                options,
            });
        }
        fields.marker("end-session")?;
        sessions.push(Session { id, name, windows });
    }
    if fields.cursor != values.len() {
        return Err("unexpected trailing argument".into());
    }
    Ok(Snapshot {
        width,
        client_width,
        client_height,
        current_session,
        current_pane,
        pane_path,
        sessions,
    })
}

fn words(source: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = source.chars();
    let mut quote = None;
    let mut started = false;
    while let Some(character) = chars.next() {
        match (quote, character) {
            (None, '\'' | '"') => {
                quote = Some(character);
                started = true;
            }
            (Some(expected), value) if expected == value => quote = None,
            (None, '\\') | (Some('"'), '\\') => {
                current.push(chars.next().ok_or("trailing shell escape")?);
                started = true;
            }
            (None, value) if value.is_whitespace() => {
                if started {
                    result.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (_, value) => {
                current.push(value);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unterminated quoted tmux value".into());
    }
    if started {
        result.push(current);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{words, Application, Focus, MemoryTmux, Sidebar, Snapshot};
    use crate::{Session, Window};
    use std::collections::BTreeMap;

    #[test]
    fn shell_quoted_snapshot_values_remain_literal() {
        assert_eq!(
            words("--name='space and '\\''quote' --path='' --literal='#[fg=red]#{oops}$x'\n")
                .unwrap(),
            [
                "--name=space and 'quote",
                "--path=",
                "--literal=#[fg=red]#{oops}$x"
            ]
        );
    }

    #[test]
    fn application_uses_the_tmux_port_for_rendering_focus_and_activation() {
        let snapshot = Snapshot {
            width: 30,
            client_width: 100,
            client_height: 25,
            current_session: "$0".into(),
            current_pane: "%0".into(),
            pane_path: "/tmp".into(),
            sessions: vec![Session {
                id: "$0".into(),
                name: "main".into(),
                windows: vec![Window {
                    id: "@0".into(),
                    index: 0,
                    name: "code".into(),
                    selected: true,
                    pane: "%0".into(),
                    path: "/tmp".into(),
                    options: BTreeMap::new(),
                }],
            }],
        };
        let tmux = MemoryTmux::new(snapshot);
        let app = Application::new(Sidebar::defaults().unwrap(), tmux.clone());
        let rendered = app
            .render_query(
                "socket",
                "client",
                18,
                Some(&Focus::new("$0", "@0").unwrap()),
            )
            .unwrap();
        assert!(rendered.contains("code"));
        assert!(app
            .render_query(
                "socket",
                "client",
                18,
                Some(&Focus::new("$0", "@1").unwrap()),
            )
            .is_err());
        app.activate("socket", "client", "sw0").unwrap();
        assert_eq!(
            tmux.activations(),
            vec![("socket".into(), "client".into(), "sw0".into())]
        );
    }
}
