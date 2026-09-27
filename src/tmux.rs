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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pane {
    pub id: String,
    pub session: String,
    pub session_name: String,
    pub window: String,
    pub state: String,
}

pub trait Tmux {
    fn panes(&self, socket: &str) -> Result<Vec<Pane>, String>;

    fn snapshot(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        window_options: &[(String, String)],
    ) -> Result<Snapshot, String>;

    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String>;

    fn open_url(&self, url: &str) -> Result<(), String>;

    fn open_file(
        &self,
        path: &std::path::Path,
        command: Option<&[std::ffi::OsString]>,
    ) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessTmux;

impl Tmux for ProcessTmux {
    fn panes(&self, socket: &str) -> Result<Vec<Pane>, String> {
        if socket.is_empty() {
            return Err("missing tmux socket".into());
        }
        let output = Command::new("tmux")
            .args(["-S", socket, "list-panes", "-a", "-F"])
            .arg("--pane-id=#{pane_id} --session-id=#{session_id} --session-name=#{q/s:session_name} --window-id=#{window_id} --state=#{q/s:@pi_state}")
            .output()
            .map_err(|error| format!("tmux panes query failed: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "tmux panes query failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if output.stdout.len() > 128 * 1024 {
            return Err("tmux panes query exceeds 128 KiB".into());
        }
        let text = String::from_utf8(output.stdout).map_err(|_| "tmux panes query is not UTF-8")?;
        parse_panes(&words(&text)?)
    }

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

    fn open_url(&self, url: &str) -> Result<(), String> {
        open_browser_target(std::ffi::OsStr::new(url))
    }

    fn open_file(
        &self,
        path: &std::path::Path,
        command: Option<&[std::ffi::OsString]>,
    ) -> Result<(), String> {
        if let Some(command) = command {
            let (executable, args) = command.split_first().ok_or("empty file opener command")?;
            let status = Command::new(executable)
                .args(args)
                .status()
                .map_err(|error| format!("file opener failed: {error}"))?;
            return if status.success() {
                Ok(())
            } else {
                Err(format!("file opener exited with {status}"))
            };
        }
        open_browser_target(path.as_os_str())
    }

    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        if socket.is_empty() || client.is_empty() {
            return Err("missing tmux socket or client name".into());
        }
        if token.starts_with("su") {
            let url = crate::usage::page_url(token).ok_or("invalid click target")?;
            return open_browser_url(url);
        }
        if token.starts_with("sp") {
            let (pane_id, window_id) = crate::navigation::pane_target(token)?;
            let pane = self
                .panes(socket)?
                .into_iter()
                .find(|pane| pane.id == pane_id && pane.window == window_id)
                .ok_or("Pi pane click target is no longer present")?;
            let target = format!("{}:{}", pane.session, pane.window);
            tmux_switch(socket, client, &target)?;
            let output = Command::new("tmux")
                .args(["-S", socket, "select-pane", "-t", &pane.id])
                .output()
                .map_err(|error| format!("tmux pane selection failed: {error}"))?;
            return if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "tmux pane selection failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            };
        }
        let target = crate::navigation::target(token)?;
        tmux_switch(socket, client, &target)
    }
}

fn open_browser_url(url: &str) -> Result<(), String> {
    open_browser_target(std::ffi::OsStr::new(url))
}

fn open_browser_target(target: &std::ffi::OsStr) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err("opening browser pages is unsupported on this platform".into());

    let status = Command::new(opener)
        .arg(target)
        .status()
        .map_err(|error| format!("browser opener failed: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("browser opener exited with {status}"))
    }
}

fn tmux_switch(socket: &str, client: &str, target: &str) -> Result<(), String> {
    let output = Command::new("tmux")
        .args(["-S", socket, "switch-client", "-c", client, "-t", target])
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

#[cfg(test)]
#[derive(Clone, Debug)]
struct MemoryTmux {
    snapshot: Snapshot,
    panes: Vec<Pane>,
    activations: Arc<Mutex<Vec<(String, String, String)>>>,
    opened_urls: Arc<Mutex<Vec<String>>>,
    opened_files: Arc<Mutex<Vec<std::path::PathBuf>>>,
    file_commands: Arc<Mutex<Vec<Option<Vec<std::ffi::OsString>>>>>,
}

#[cfg(test)]
impl MemoryTmux {
    fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            panes: Vec::new(),
            activations: Arc::new(Mutex::new(Vec::new())),
            opened_urls: Arc::new(Mutex::new(Vec::new())),
            opened_files: Arc::new(Mutex::new(Vec::new())),
            file_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_panes(mut self, panes: Vec<Pane>) -> Self {
        self.panes = panes;
        self
    }

    fn activations(&self) -> Vec<(String, String, String)> {
        self.activations.lock().unwrap().clone()
    }

    fn opened_urls(&self) -> Vec<String> {
        self.opened_urls.lock().unwrap().clone()
    }

    fn opened_files(&self) -> Vec<std::path::PathBuf> {
        self.opened_files.lock().unwrap().clone()
    }

    fn file_commands(&self) -> Vec<Option<Vec<std::ffi::OsString>>> {
        self.file_commands.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Tmux for MemoryTmux {
    fn open_file(
        &self,
        path: &std::path::Path,
        command: Option<&[std::ffi::OsString]>,
    ) -> Result<(), String> {
        self.opened_files.lock().unwrap().push(path.to_owned());
        self.file_commands
            .lock()
            .unwrap()
            .push(command.map(<[_]>::to_vec));
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        self.opened_urls.lock().unwrap().push(url.to_owned());
        Ok(())
    }

    fn panes(&self, _socket: &str) -> Result<Vec<Pane>, String> {
        Ok(self.panes.clone())
    }

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
        if token.starts_with("sp") {
            crate::navigation::pane_target(token)?;
        } else if token.starts_with("su") {
            crate::usage::page_url(token).ok_or("invalid click target")?;
        } else {
            crate::navigation::target(token)?;
        }
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
        let (pi_sessions, context) = self.pi_sessions(socket, &snapshot)?;
        let states = self.pr_states(context.as_ref(), true);
        let usage = self.usage_rows(true)?;
        let jobs = self.gob_jobs(&snapshot)?;
        let (commands, _) = self.commands(&snapshot);
        self.sidebar.render_with_inputs(
            &snapshot,
            crate::RenderInputs {
                pi_sessions: &pi_sessions,
                usage_rows: &usage,
                gob_jobs: &jobs,
                context: context.as_ref(),
                states: &states,
                commands: &commands,
            },
        )
    }

    fn pi_sessions(
        &self,
        socket: &str,
        snapshot: &Snapshot,
    ) -> Result<(Vec<crate::PiSession>, Option<crate::PiContext>), String> {
        let Some(data_dir) = self.sidebar.pi_live_data_dir() else {
            return Ok((Vec::new(), None));
        };
        let path = if data_dir.is_empty() {
            crate::pi_live::default_data_dir()?
        } else {
            std::path::PathBuf::from(data_dir)
        };
        let mut entries = crate::pi_live::list(&path, std::path::Path::new(socket))?;
        if entries.iter().any(|entry| entry.session.location.is_some()) {
            let panes = self.tmux.panes(socket)?;
            for entry in &mut entries {
                let session = &mut entry.session;
                let Some(location) = &session.location else {
                    continue;
                };
                let Some(pane) = panes.iter().find(|pane| {
                    pane.id == location.pane && pane.session_name == location.session_name
                }) else {
                    continue;
                };
                session.target = Some(crate::PiTarget {
                    pane: pane.id.clone(),
                    window: pane.window.clone(),
                });
                session.selected = snapshot.current_pane == pane.id;
                session.state = match pane.state.as_str() {
                    "notify" => "notify".into(),
                    "working" => "working".into(),
                    _ => session.state.clone(),
                };
            }
        }
        let context = if self.sidebar.pi_context_enabled() {
            entries
                .iter()
                .find(|entry| entry.session.selected)
                .and_then(|entry| {
                    entry
                        .context_path
                        .as_ref()
                        .and_then(|path| crate::pi_live::read_context(path, &entry.session_id))
                })
        } else {
            None
        };
        Ok((
            entries.into_iter().map(|entry| entry.session).collect(),
            context,
        ))
    }

    fn pr_states(
        &self,
        context: Option<&crate::PiContext>,
        spawn: bool,
    ) -> Vec<crate::pr_state::PrState> {
        let Some(context) = context else {
            return Vec::new();
        };
        let Some(dir) = crate::pr_state::default_dir() else {
            return vec![crate::pr_state::PrState::Unknown; context.pull_requests.len()];
        };
        context
            .pull_requests
            .iter()
            .map(|url| crate::pr_state::resolve(url, &dir, spawn))
            .collect()
    }

    fn commands(&self, snapshot: &Snapshot) -> (BTreeMap<String, String>, Vec<String>) {
        let mut rows = BTreeMap::new();
        let mut errors = Vec::new();
        for (name, argv) in self.sidebar.command_options() {
            match crate::command::run(argv, std::path::Path::new(&snapshot.pane_path)) {
                Ok(Some(text)) => {
                    rows.insert(name.to_owned(), text);
                }
                Ok(None) => {}
                Err(error) => errors.push(format!("{name}: {error}")),
            }
        }
        (rows, errors)
    }

    fn gob_jobs(&self, snapshot: &Snapshot) -> Result<Vec<crate::GobJob>, String> {
        if !self.sidebar.gob_enabled() {
            return Ok(Vec::new());
        }
        crate::gob::list(std::path::Path::new(&snapshot.pane_path))
    }

    fn usage_rows(&self, spawn: bool) -> Result<Vec<crate::usage::UsageRow>, String> {
        let Some((providers, data_dir)) = self.sidebar.usage_options() else {
            return Ok(Vec::new());
        };
        if providers.is_empty() {
            return Ok(Vec::new());
        }
        let dir = match data_dir {
            Some(path) => std::path::PathBuf::from(path),
            None => crate::usage::default_dir()?,
        };
        Ok(crate::usage::resolve(providers, &dir, spawn))
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
        let (mut pi_sessions, context) = self.pi_sessions(socket, &snapshot)?;
        crate::pi_live::sort_sessions(&mut pi_sessions);
        for session in pi_sessions {
            result.push_str(&format!(
                "pi-live project={:?} {:?} {:?} target={:?} selected={}\n",
                session.project, session.state, session.name, session.target, session.selected
            ));
        }
        if let Some(context) = context {
            result.push_str(&format!(
                "pi-context plans={} prs={} skills={}\n",
                context.plans.len(),
                context.pull_requests.len(),
                context.skills.len()
            ));
        }
        for usage in self.usage_rows(false)? {
            result.push_str(&format!(
                "usage provider={} stale={} unavailable={} windows={}\n",
                usage.provider,
                usage.stale,
                usage.unavailable,
                usage.windows.len()
            ));
        }
        for job in self.gob_jobs(&snapshot)? {
            result.push_str(&format!(
                "gob id={:?} name={:?} progress={:?}\n",
                job.id,
                job.name,
                job.percent(time::OffsetDateTime::now_utc())
            ));
        }
        let (commands, errors) = self.commands(&snapshot);
        for (name, output) in commands {
            result.push_str(&format!("{name} output={output:?}\n"));
        }
        for error in errors {
            result.push_str(&format!("{error}\n"));
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
        let (pi_sessions, context) = self.pi_sessions(socket, &snapshot)?;
        let states = self.pr_states(context.as_ref(), false);
        let queried = started.elapsed().as_micros();
        let usage = self.usage_rows(false)?;
        let usage_us = started.elapsed().as_micros().saturating_sub(queried);
        let jobs = self.gob_jobs(&snapshot)?;
        let gob_us = started
            .elapsed()
            .as_micros()
            .saturating_sub(queried + usage_us);
        let (commands, _) = self.commands(&snapshot);
        let command_us = started
            .elapsed()
            .as_micros()
            .saturating_sub(queried + usage_us + gob_us);
        self.sidebar.render_with_inputs(
            &snapshot,
            crate::RenderInputs {
                pi_sessions: &pi_sessions,
                usage_rows: &usage,
                gob_jobs: &jobs,
                context: context.as_ref(),
                states: &states,
                commands: &commands,
            },
        )?;
        Ok(format!(
            "{{\"query_us\":{queried},\"usage_us\":{usage_us},\"gob_us\":{gob_us},\"command_us\":{command_us},\"render_us\":{}}}",
            started.elapsed().as_micros().saturating_sub(queried + usage_us + gob_us + command_us)
        ))
    }

    pub fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        if token.starts_with("sl") || token.starts_with("ss") {
            if !self.sidebar.pi_context_enabled() {
                return Err("file click target is not enabled".into());
            }
            let index = crate::navigation::file_index(token)?;
            let snapshot = self.snapshot(socket, client, 1, None)?;
            let mut context = None;
            for _ in 0..3 {
                context = self.pi_sessions(socket, &snapshot)?.1;
                if context.is_some() {
                    break;
                }
            }
            let context = context.ok_or("file click target is no longer present")?;
            let path = if token.starts_with("sl") {
                &context
                    .plans
                    .get(index)
                    .ok_or("plan click target is no longer present")?
                    .path
            } else {
                context
                    .skills
                    .get(index)
                    .and_then(|skill| skill.path.as_ref())
                    .ok_or("skill click target is no longer present")?
            };
            if crate::navigation::file_token(&token[..2], &snapshot.current_pane, path, index)?
                != token
                || !std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
            {
                return Err("file click target is no longer present".into());
            }
            let command = self
                .sidebar
                .file_open_args(path, &snapshot.current_pane, socket);
            return self.tmux.open_file(path, command.as_deref());
        }
        if token.starts_with("sr") {
            if !self.sidebar.pi_context_enabled() {
                return Err("PR click target is not enabled".into());
            }
            let snapshot = self.snapshot(socket, client, 1, None)?;
            let (_, context) = self.pi_sessions(socket, &snapshot)?;
            let context = context.ok_or("PR click target is no longer present")?;
            let url = crate::navigation::pr_target(
                token,
                &snapshot.current_pane,
                &context.pull_requests,
            )?;
            return self.tmux.open_url(url);
        }
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
    format!("--input-version=3 --client-width=#{{client_width}} --client-height=#{{client_height}} --status=#{{status}} --current-session=#{{q/s:session_id}} --current-pane=#{{pane_id}} --pane-path=#{{q/s:pane_current_path}} --session-count=#{{server_sessions}} #{{S:{session} #{{W:{window} }} --end-session }}")
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

fn parse_panes(values: &[String]) -> Result<Vec<Pane>, String> {
    if !values.len().is_multiple_of(5) || values.len() / 5 > 10000 {
        return Err("invalid tmux panes query".into());
    }
    let mut fields = Fields { values, cursor: 0 };
    let mut panes = Vec::new();
    while fields.cursor < values.len() {
        let id = fields.id("pane-id", '%')?;
        let session = fields.id("session-id", '$')?;
        let session_name = fields.take("session-name")?;
        let window = fields.id("window-id", '@')?;
        let state = fields.take("state")?;
        panes.push(Pane {
            id,
            session,
            session_name,
            window,
            state,
        });
    }
    Ok(panes)
}

fn parse_snapshot(
    values: &[String],
    width: usize,
    window_options: &[(String, String)],
) -> Result<Snapshot, String> {
    let mut fields = Fields { values, cursor: 0 };
    if fields.take("input-version")? != "3" {
        return Err("unsupported tmux snapshot version".into());
    }
    let client_width = fields.number("client-width", 10000)?;
    let client_height = fields.number("client-height", 10000)?;
    let status = fields.take("status")?;
    let status_lines = match status.as_str() {
        "off" => 0,
        "on" => 1,
        _ => status
            .parse::<usize>()
            .ok()
            .filter(|lines| *lines <= client_height)
            .ok_or("invalid tmux status height")?,
    };
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
        status_lines,
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
    use super::{words, Application, Focus, MemoryTmux, Pane, Sidebar, Snapshot};
    use crate::{Session, Window};
    use std::collections::BTreeMap;

    #[test]
    fn named_commands_follow_the_selected_pane_and_fail_independently() {
        use std::{fs, time::Instant};
        let root = std::env::temp_dir().join(format!("starmux-commands-{}", std::process::id()));
        fs::create_dir_all(root.join("first")).unwrap();
        fs::create_dir_all(root.join("second")).unwrap();
        let config = "modules = [\"command.cwd\", \"command.slow\"]\n[commands.cwd]\nargv = [\"/bin/pwd\"]\n[commands.slow]\nargv = [\"sleep\", \"2\"]\n";
        let sidebar = Sidebar::from_toml(config).unwrap();
        for (path, expected) in [
            (root.join("first"), "first"),
            (root.join("second"), "second"),
        ] {
            let snapshot = Snapshot {
                width: 30,
                client_width: 100,
                client_height: 25,
                status_lines: 1,
                current_session: "$0".into(),
                current_pane: "%0".into(),
                pane_path: path.to_str().unwrap().into(),
                sessions: vec![Session {
                    id: "$0".into(),
                    name: "main".into(),
                    windows: vec![],
                }],
            };
            let app = Application::new(sidebar.clone(), MemoryTmux::new(snapshot));
            let started = Instant::now();
            let rendered = app.render_query("socket", "client", 300, None).unwrap();
            assert!(
                rendered.contains(&format!("/{expected}")),
                "{rendered}; {}",
                app.explain("socket", "client", 30, None).unwrap()
            );
            assert!(started.elapsed().as_millis() < 1200);
            assert!(app
                .explain("socket", "client", 30, None)
                .unwrap()
                .contains("command.slow: command timed out"));
        }
        fs::remove_dir_all(root).unwrap();
    }

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
            status_lines: 1,
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

    #[test]
    fn enabled_pi_live_renders_only_reachable_published_sessions() {
        use std::{
            fs,
            io::{Read, Write},
            os::unix::net::UnixListener,
            thread,
        };
        let data_dir = std::env::temp_dir().join(format!(
            "starmux-pi-live-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        fs::create_dir_all(data_dir.join("status")).unwrap();
        fs::create_dir_all(data_dir.join("sockets")).unwrap();
        let repo = data_dir.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        let socket_path = data_dir.join("sockets/live.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let responder = thread::spawn(move || {
            for _ in 0..20 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; 128];
                let len = socket.read(&mut request).unwrap();
                assert!(std::str::from_utf8(&request[..len])
                    .unwrap()
                    .contains("\"ping\""));
                socket
                    .write_all(b"{\"ok\":true,\"result\":{\"type\":\"pong\"}}\n")
                    .unwrap();
            }
        });
        let record = |id: &str, socket: &std::path::Path, name: &str, cwd: &std::path::Path| {
            serde_json::json!({
                "version": 1, "sessionId": id, "name": name, "pid": 1,
                "cwd": cwd, "socketPath": socket, "startedAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z", "state": "working"
            })
            .to_string()
        };
        let mut live: serde_json::Value = serde_json::from_str(&record(
            "live",
            &socket_path,
            "#[fg=red]#{oops}",
            &repo.join("src"),
        ))
        .unwrap();
        live["state"] = "idle".into();
        let session_file = data_dir.join("live.jsonl");
        let context_path = data_dir.join("live.jsonl.context.json");
        live["sessionFile"] = serde_json::json!(session_file);
        live["contextPath"] = serde_json::json!(context_path);
        let plan_path = data_dir.join("live.jsonl.plans/0123456789abcdef01234567.md");
        fs::create_dir_all(plan_path.parent().unwrap()).unwrap();
        fs::write(&plan_path, "# Build search").unwrap();
        let skill_path = data_dir.join("skills/testing/SKILL.md");
        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "# Testing").unwrap();
        fs::write(
            &context_path,
            serde_json::json!({
                "version": 1, "sessionId": "live",
                "plans": [{"id": "0123456789abcdef01234567", "title": "#[fg=red] Build search", "path": "live.jsonl.plans/0123456789abcdef01234567.md"}],
                "pullRequests": [],
                "skills": ["testing"],
                "skillPaths": {"testing": skill_path}
            })
            .to_string(),
        )
        .unwrap();
        live["tmux"] = serde_json::json!({
            "paneId": "%0", "sessionName": "main", "windowIndex": 5, "windowName": "old",
            "socketPath": "/tmp/starmux-current.sock"
        });
        fs::write(data_dir.join("status/live.json"), live.to_string()).unwrap();
        let mut unnamed: serde_json::Value =
            serde_json::from_str(&record("unnamed", &socket_path, "", &repo)).unwrap();
        unnamed["tmux"] = serde_json::json!({
            "paneId": "%1", "sessionName": "main", "socketPath": "/tmp/starmux-current.sock"
        });
        fs::write(data_dir.join("status/unnamed.json"), unnamed.to_string()).unwrap();
        fs::write(
            data_dir.join("status/stale.json"),
            record(
                "stale",
                &data_dir.join("sockets/stale.sock"),
                "stale",
                &repo,
            ),
        )
        .unwrap();
        fs::write(data_dir.join("status/broken.json"), "{bad json").unwrap();
        let config = format!(
            "modules = [\"sessions\", \"pi-live\", \"pi-context\"]\n[pi-live]\ndata_dir = {:?}\nformat = \"$name $state\"\n[pi-context]\nopen_command = [\"dev\", \"tmux\", \"edit\", \"{{file}}\", \"{{pane}}\", \"{{socket}}\"]\n",
            data_dir.to_str().unwrap()
        );
        let tmux = MemoryTmux::new(Snapshot {
            width: 40,
            client_width: 100,
            client_height: 25,
            status_lines: 1,
            current_session: "$0".into(),
            current_pane: "%0".into(),
            pane_path: "/tmp".into(),
            sessions: vec![Session {
                id: "$0".into(),
                name: "main".into(),
                windows: vec![],
            }],
        })
        .with_panes(vec![Pane {
            id: "%0".into(),
            session: "$0".into(),
            session_name: "main".into(),
            window: "@9".into(),
            state: "notify".into(),
        }]);
        let app = Application::new(Sidebar::from_toml(&config).unwrap(), tmux.clone());
        let rendered = app
            .render_query("/tmp/starmux-current.sock", "client", 40, None)
            .unwrap();
        let mut other_snapshot = tmux.snapshot.clone();
        other_snapshot.current_pane = "%1".into();
        let other_tmux = MemoryTmux::new(other_snapshot).with_panes(tmux.panes.clone());
        let other_app = Application::new(Sidebar::from_toml(&config).unwrap(), other_tmux);
        let other_rendered = other_app
            .render_query("/tmp/starmux-current.sock", "client", 40, None)
            .unwrap();
        assert!(
            !other_rendered.contains(" Plans") && !other_rendered.contains("Build search"),
            "{other_rendered}"
        );
        let context_only = format!(
            "modules = [\"pi-context\"]\n[pi-live]\ndata_dir = {:?}\n",
            data_dir.to_str().unwrap()
        );
        let context_app =
            Application::new(Sidebar::from_toml(&context_only).unwrap(), tmux.clone());
        let context_rendered = context_app
            .render_query("/tmp/starmux-current.sock", "client", 40, None)
            .unwrap();
        assert!(
            context_rendered.contains("Build search") && !context_rendered.contains("unnamed"),
            "{context_rendered}"
        );
        let plan_token = crate::navigation::file_token("sl", "%0", &plan_path, 0).unwrap();
        let skill_token = crate::navigation::file_token("ss", "%0", &skill_path, 0).unwrap();
        app.activate("/tmp/starmux-current.sock", "client", &plan_token)
            .unwrap();
        app.activate("/tmp/starmux-current.sock", "client", &skill_token)
            .unwrap();
        assert_eq!(tmux.opened_files(), [plan_path.clone(), skill_path.clone()]);
        assert_eq!(
            tmux.file_commands(),
            [
                Some(vec![
                    "dev".into(),
                    "tmux".into(),
                    "edit".into(),
                    plan_path.as_os_str().to_owned(),
                    "%0".into(),
                    "/tmp/starmux-current.sock".into()
                ]),
                Some(vec![
                    "dev".into(),
                    "tmux".into(),
                    "edit".into(),
                    skill_path.as_os_str().to_owned(),
                    "%0".into(),
                    "/tmp/starmux-current.sock".into()
                ]),
            ]
        );
        fs::remove_file(&plan_path).unwrap();
        assert!(app
            .activate("/tmp/starmux-current.sock", "client", &plan_token)
            .is_err());
        assert_eq!(tmux.opened_files().len(), 2);
        let pr_url = "https://github.com/owner/repo/pull/42";
        let second_url = "https://github.com/owner/another/pull/47";
        let mut updated_context: serde_json::Value =
            serde_json::from_slice(&fs::read(&context_path).unwrap()).unwrap();
        updated_context["pullRequests"] = serde_json::json!([pr_url, second_url]);
        fs::write(&context_path, updated_context.to_string()).unwrap();
        let token = crate::navigation::pr_token("%0", pr_url, 0).unwrap();
        let second_token = crate::navigation::pr_token("%0", second_url, 1).unwrap();
        app.activate("/tmp/starmux-current.sock", "client", &token)
            .unwrap();
        app.activate("/tmp/starmux-current.sock", "client", &second_token)
            .unwrap();
        assert_eq!(tmux.opened_urls(), [pr_url, second_url]);
        assert!(other_app
            .activate("/tmp/starmux-current.sock", "client", &token)
            .is_err());
        updated_context["pullRequests"] = serde_json::json!([]);
        fs::write(&context_path, updated_context.to_string()).unwrap();
        assert!(app
            .activate("/tmp/starmux-current.sock", "client", &token)
            .is_err());
        assert_eq!(tmux.opened_urls(), [pr_url, second_url]);
        responder.join().unwrap();
        assert!(rendered.contains("#[bold] repo#[bg=default]"), "{rendered}");
        assert!(rendered.contains("#[range=user|sp9 ]"), "{rendered}");
        assert!(
            rendered.contains(
                "#[default,reverse,bold]##[fg=red]##{oops} #[default,fg=magenta,bg=default]●"
            ),
            "{rendered}"
        );
        assert!(rendered.contains("unnamed #[fg=yellow]●"), "{rendered}");
        assert!(rendered.contains(" Plans"), "{rendered}");
        assert!(!rendered.contains(" Context"), "{rendered}");
        assert!(rendered.contains("◇"), "{rendered}");
        assert!(rendered.contains("##[fg=red] Build search"), "{rendered}");
        assert!(rendered.contains("✦"), "{rendered}");
        assert!(rendered.find("##[fg=red]").unwrap() < rendered.find("unnamed").unwrap());
        assert!(!rendered.contains("stale"), "{rendered}");
        assert_eq!(rendered.matches("#[range=").count(), 4);
        app.activate("/tmp/starmux-current.sock", "client", "sp9")
            .unwrap();
        assert_eq!(
            tmux.activations(),
            vec![(
                "/tmp/starmux-current.sock".into(),
                "client".into(),
                "sp9".into()
            )]
        );
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn pi_live_excludes_foreign_and_legacy_records_before_matching_panes() {
        use std::{
            fs,
            io::{Read, Write},
            os::unix::net::UnixListener,
            thread,
        };
        let data_dir = std::env::temp_dir().join(format!(
            "sx-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        fs::create_dir_all(data_dir.join("status")).unwrap();
        fs::create_dir_all(data_dir.join("sockets")).unwrap();
        let pi_socket = data_dir.join("sockets/live.sock");
        let listener = UnixListener::bind(&pi_socket).unwrap();
        let responder = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 128];
                assert!(stream.read(&mut request).unwrap() > 0);
                stream
                    .write_all(b"{\"ok\":true,\"result\":{\"type\":\"pong\"}}\n")
                    .unwrap();
            }
        });
        let record = |id: &str, tmux: serde_json::Value| {
            serde_json::json!({
                "version": 1, "sessionId": id, "name": id, "pid": 1,
                "cwd": "/tmp", "socketPath": pi_socket,
                "startedAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
                "state": "idle", "tmux": tmux
            })
            .to_string()
        };
        for index in 0..40 {
            let id = format!("a-foreign-{index:02}");
            fs::write(data_dir.join("status").join(format!("{id}.json")), record(&id,
                serde_json::json!({"paneId": "%0", "sessionName": "main", "socketPath": "/tmp/other-tmux.sock"})
            )).unwrap();
        }
        fs::write(
            data_dir.join("status/legacy.json"),
            record(
                "legacy",
                serde_json::json!({"paneId": "%0", "sessionName": "main"}),
            ),
        )
        .unwrap();
        fs::write(
            data_dir.join("status/no-tmux.json"),
            record("no-tmux", serde_json::Value::Null),
        )
        .unwrap();
        fs::write(data_dir.join("status/z-local.json"), record("z-local",
            serde_json::json!({"paneId": "%0", "sessionName": "main", "socketPath": "/tmp/current-tmux.sock"})
        )).unwrap();
        let config = format!(
            "modules = [\"pi-live\"]\n[pi-live]\ndata_dir = {:?}\n",
            data_dir.to_str().unwrap()
        );
        let tmux = MemoryTmux::new(Snapshot {
            width: 40,
            client_width: 100,
            client_height: 25,
            status_lines: 1,
            current_session: "$0".into(),
            current_pane: "%0".into(),
            pane_path: "/tmp".into(),
            sessions: vec![Session {
                id: "$0".into(),
                name: "main".into(),
                windows: vec![],
            }],
        })
        .with_panes(vec![Pane {
            id: "%0".into(),
            session: "$0".into(),
            session_name: "main".into(),
            window: "@1".into(),
            state: "notify".into(),
        }]);
        let app = Application::new(Sidebar::from_toml(&config).unwrap(), tmux);
        let rendered = app
            .render_query("/tmp/current-tmux.sock", "client", 40, None)
            .unwrap();
        assert!(rendered.contains("z-local"), "{rendered}");
        assert!(rendered.contains("#[range=user|sp"), "{rendered}");
        assert!(rendered.contains("fg=magenta,bg=default]●"), "{rendered}");
        assert!(
            !rendered.contains("foreign")
                && !rendered.contains("legacy")
                && !rendered.contains("no-tmux"),
            "{rendered}"
        );
        let explained = app
            .explain("/tmp/current-tmux.sock", "client", 40, None)
            .unwrap();
        assert!(
            explained.contains("z-local") && !explained.contains("foreign"),
            "{explained}"
        );
        responder.join().unwrap();
        fs::remove_dir_all(data_dir).unwrap();
    }
}
