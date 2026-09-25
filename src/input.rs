#[derive(Debug, Clone)]
pub struct Window {
    pub id: String,
    pub index: usize,
    pub name: String,
    pub selected: bool,
    pub pane: String,
    pub path: String,
    pub pi_state: String,
    pub icon: String,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub windows: Vec<Window>,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub width: usize,
    pub client_width: usize,
    pub client_height: usize,
    pub current_session: String,
    pub current_pane: String,
    pub pane_path: String,
    pub sessions: Vec<Session>,
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
        if value
            .strip_prefix(prefix)
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            Ok(value)
        } else {
            Err(format!("invalid --{key}"))
        }
    }
}

pub fn parse(values: &[String]) -> Result<Context, String> {
    let mut f = Fields { values, cursor: 0 };
    let version = f.take("input-version")?;
    if version != "1" {
        return Err(format!(
            "input version {version} is unsupported; expected 1"
        ));
    }
    let width = f.number("width", 300)?;
    if width == 0 {
        return Err("width must be positive".into());
    }
    let client_width = f.number("client-width", 10000)?;
    let client_height = f.number("client-height", 10000)?;
    let current_session = f.id("current-session", '$')?;
    let current_pane = f.id("current-pane", '%')?;
    let pane_path = f.take("pane-path")?;
    let session_count = f.number("session-count", 10000)?;
    let mut sessions = Vec::with_capacity(session_count);
    let mut window_total = 0;
    for _ in 0..session_count {
        let id = f.id("session-id", '$')?;
        let name = f.take("session-name")?;
        let window_count = f.number("window-count", 10000)?;
        window_total += window_count;
        if window_total > 10000 {
            return Err("too many windows".into());
        }
        let mut windows = Vec::with_capacity(window_count);
        for _ in 0..window_count {
            let wid = f.id("window-id", '@')?;
            let index = f.number("window-index", 1000000)?;
            let wname = f.take("window-name")?;
            let selected = f.flag("selected")?;
            let pane = f.id("window-pane", '%')?;
            let path = f.take("window-path")?;
            let pi_state = f.take("pi-state")?;
            let icon = f.take("window-icon")?;
            f.marker("end-window")?;
            windows.push(Window {
                id: wid,
                index,
                name: wname,
                selected,
                pane,
                path,
                pi_state,
                icon,
            });
        }
        f.marker("end-session")?;
        sessions.push(Session { id, name, windows });
    }
    if f.cursor != values.len() {
        return Err("unexpected trailing argument".into());
    }
    if !sessions.iter().any(|s| s.id == current_session) {
        return Err("current session not present".into());
    }
    Ok(Context {
        width,
        client_width,
        client_height,
        current_session,
        current_pane,
        pane_path,
        sessions,
    })
}
