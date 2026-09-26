use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MAX_WIDTH: usize = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub index: usize,
    pub name: String,
    pub selected: bool,
    pub pane: String,
    pub path: String,
    pub options: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub windows: Vec<Window>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub width: usize,
    pub client_width: usize,
    pub client_height: usize,
    pub current_session: String,
    pub current_pane: String,
    pub pane_path: String,
    pub sessions: Vec<Session>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    modules: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    palettes: BTreeMap<String, BTreeMap<String, String>>,
    sessions: SessionsConfig,
    divider: DividerConfig,
    #[serde(rename = "pi-live")]
    pi_live: PiLiveConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct SessionsConfig {
    disabled: bool,
    session_format: String,
    window_format: String,
    current_session_style: String,
    other_session_style: String,
    active_window_style: String,
    selected_window_style: String,
    other_window_style: String,
    current_session_fill: String,
    other_session_fill: String,
    active_window_fill: String,
    selected_window_fill: String,
    other_window_fill: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    window_options: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    indicator: Option<IndicatorConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct IndicatorConfig {
    source: String,
    fallback: String,
    style: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rules: Vec<IndicatorRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct IndicatorRule {
    when: BTreeMap<String, String>,
    text: String,
    style: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct PiLiveConfig {
    disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_dir: Option<String>,
    format: String,
    project_style: String,
    idle_style: String,
    working_style: String,
    notify_style: String,
    selected_style: String,
    selected_fill: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct DividerConfig {
    disabled: bool,
    character: String,
    style: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            modules: vec!["sessions".into(), "divider".into()],
            palette: None,
            palettes: BTreeMap::new(),
            sessions: SessionsConfig::default(),
            divider: DividerConfig::default(),
            pi_live: PiLiveConfig::default(),
        }
    }
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            session_format: " $name ".into(),
            window_format: " $index: $name".into(),
            current_session_style: "reverse,bold".into(),
            other_session_style: "bold".into(),
            active_window_style: "reverse,bold,nounderscore,noitalics".into(),
            selected_window_style: "bold,nounderscore,noitalics".into(),
            other_window_style: "nobold,nounderscore,noitalics".into(),
            current_session_fill: "default".into(),
            other_session_fill: "default".into(),
            active_window_fill: "default".into(),
            selected_window_fill: "default".into(),
            other_window_fill: "default".into(),
            window_options: BTreeMap::new(),
            indicator: None,
        }
    }
}

impl Default for IndicatorConfig {
    fn default() -> Self {
        Self {
            source: String::new(),
            fallback: String::new(),
            style: "$style".into(),
            rules: Vec::new(),
        }
    }
}

impl Default for IndicatorRule {
    fn default() -> Self {
        Self {
            when: BTreeMap::new(),
            text: String::new(),
            style: "$style".into(),
        }
    }
}

impl Default for PiLiveConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            data_dir: None,
            format: "   $state $name".into(),
            project_style: "bold".into(),
            idle_style: "fg=brightblack".into(),
            working_style: "fg=yellow".into(),
            notify_style: "fg=magenta".into(),
            selected_style: "reverse,bold".into(),
            selected_fill: "default".into(),
        }
    }
}

impl Default for DividerConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            character: "-".into(),
            style: "dim".into(),
        }
    }
}

#[derive(Clone, Debug)]
enum Node {
    Text(String),
    Variable(String),
    Optional(Vec<Node>),
    Styled(Vec<Node>, String),
}

#[derive(Clone, Debug)]
struct CompiledSessions {
    config: SessionsConfig,
    session_format: Vec<Node>,
    window_format: Vec<Node>,
}

#[derive(Clone, Debug)]
pub struct Sidebar {
    config: Config,
    palette: BTreeMap<String, String>,
    sessions: CompiledSessions,
    pi_live_format: Vec<Node>,
}

impl Sidebar {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let config = if text.trim().is_empty() {
            Config::default()
        } else {
            toml::from_str(text).map_err(|error| error.to_string())?
        };
        Self::compile(config)
    }

    pub fn defaults() -> Result<Self, String> {
        Self::compile(Config::default())
    }

    fn compile(config: Config) -> Result<Self, String> {
        let known = ["sessions", "divider", "pi-live"];
        let mut seen = BTreeSet::new();
        for name in &config.modules {
            if !known.contains(&name.as_str()) {
                return Err(format!("unknown module {name}"));
            }
            if !seen.insert(name) {
                return Err(format!("duplicate module {name}"));
            }
        }
        let palette = match &config.palette {
            Some(name) => config
                .palettes
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown palette {name}"))?,
            None => BTreeMap::new(),
        };
        for (name, value) in &palette {
            validate_name(name, "palette color")?;
            validate_color(value, &BTreeMap::new())?;
        }

        let session_format = Parser::parse(&config.sessions.session_format)?;
        validate_format(&session_format, &["id", "name"], &palette)?;
        let window_format = Parser::parse(&config.sessions.window_format)?;
        validate_format(
            &window_format,
            &["id", "index", "name", "indicator"],
            &palette,
        )?;
        let pi_live_format = Parser::parse(&config.pi_live.format)?;
        validate_format(&pi_live_format, &["name", "state"], &palette)?;
        if config
            .pi_live
            .data_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("pi-live data_dir must be an absolute path".into());
        }
        for style in [
            &config.sessions.current_session_style,
            &config.sessions.other_session_style,
            &config.sessions.active_window_style,
            &config.sessions.selected_window_style,
            &config.sessions.other_window_style,
            &config.divider.style,
            &config.pi_live.project_style,
            &config.pi_live.idle_style,
            &config.pi_live.working_style,
            &config.pi_live.notify_style,
            &config.pi_live.selected_style,
        ] {
            resolve_style(style, "default", &palette)?;
        }
        for fill in [
            &config.sessions.current_session_fill,
            &config.sessions.other_session_fill,
            &config.sessions.active_window_fill,
            &config.sessions.selected_window_fill,
            &config.sessions.other_window_fill,
            &config.pi_live.selected_fill,
        ] {
            resolve_color(fill, &palette)?;
        }
        for (alias, option) in &config.sessions.window_options {
            validate_name(alias, "window option alias")?;
            if !valid_tmux_option(option) {
                return Err(format!("invalid tmux window option {option}"));
            }
        }
        if let Some(indicator) = &config.sessions.indicator {
            validate_indicator(indicator, &config.sessions.window_options, &palette)?;
        }
        let mut graphemes = config.divider.character.graphemes(true);
        let Some(character) = graphemes.next() else {
            return Err("divider character must not be empty".into());
        };
        if graphemes.next().is_some() || UnicodeWidthStr::width(character) == 0 {
            return Err("divider character must be one visible grapheme".into());
        }

        let sessions = CompiledSessions {
            config: config.sessions.clone(),
            session_format,
            window_format,
        };
        Ok(Self {
            config,
            palette,
            sessions,
            pi_live_format,
        })
    }

    pub fn requested_window_options(&self) -> Vec<(String, String)> {
        if self.sessions.config.disabled
            || !self.config.modules.iter().any(|name| name == "sessions")
            || !contains_variable(&self.sessions.window_format, "indicator")
            || self.sessions.config.indicator.is_none()
        {
            return Vec::new();
        }
        self.sessions
            .config
            .window_options
            .iter()
            .map(|(alias, option)| (alias.clone(), option.clone()))
            .collect()
    }

    pub(crate) fn pi_live_data_dir(&self) -> Option<&str> {
        if self.config.pi_live.disabled || !self.config.modules.iter().any(|name| name == "pi-live")
        {
            return None;
        }
        Some(self.config.pi_live.data_dir.as_deref().unwrap_or(""))
    }

    pub fn render(&self, snapshot: &Snapshot) -> Result<String, String> {
        self.render_with_pi_live(snapshot, &[])
    }

    pub fn render_with_pi_live(
        &self,
        snapshot: &Snapshot,
        pi_sessions: &[crate::PiSession],
    ) -> Result<String, String> {
        validate_snapshot(snapshot)?;
        let mut rows = Vec::new();
        for name in &self.config.modules {
            match name.as_str() {
                "sessions" if !self.sessions.config.disabled => {
                    rows.extend(self.render_sessions(snapshot)?)
                }
                "divider" if !self.config.divider.disabled => {
                    rows.push(self.render_divider(snapshot.width)?)
                }
                "pi-live" if !self.config.pi_live.disabled => {
                    rows.extend(self.render_pi_live(pi_sessions)?)
                }
                "sessions" | "divider" | "pi-live" => {}
                _ => return Err(format!("unknown module {name}")),
            }
        }
        Ok(render_rows(&rows, snapshot.width))
    }

    pub fn print_config(&self) -> Result<String, String> {
        toml::to_string_pretty(&self.config).map_err(|error| error.to_string())
    }

    pub(crate) fn module_names(&self) -> &[String] {
        &self.config.modules
    }

    fn render_sessions(&self, snapshot: &Snapshot) -> Result<Vec<Row>, String> {
        let mut rows = Vec::new();
        for session in &snapshot.sessions {
            let current = session.id == snapshot.current_session;
            let style = if current {
                &self.sessions.config.current_session_style
            } else {
                &self.sessions.config.other_session_style
            };
            let fill = if current {
                &self.sessions.config.current_session_fill
            } else {
                &self.sessions.config.other_session_fill
            };
            let style = resolve_style(style, "default", &self.palette)?;
            let values = BTreeMap::from([
                ("id", Value::Text(session.id.clone())),
                ("name", Value::Text(session.name.clone())),
            ]);
            rows.push(Row {
                spans: render_format(
                    &self.sessions.session_format,
                    &values,
                    &style,
                    &self.palette,
                )?,
                fill: Some(resolve_color(fill, &self.palette)?),
                range: Some(Range::Session(session.id.clone())),
                focus: false,
                selected: false,
            });

            for window in &session.windows {
                let active = current && window.selected;
                let (style, fill) = if active {
                    (
                        &self.sessions.config.active_window_style,
                        &self.sessions.config.active_window_fill,
                    )
                } else if window.selected {
                    (
                        &self.sessions.config.selected_window_style,
                        &self.sessions.config.selected_window_fill,
                    )
                } else {
                    (
                        &self.sessions.config.other_window_style,
                        &self.sessions.config.other_window_fill,
                    )
                };
                let style = resolve_style(style, "default", &self.palette)?;
                let indicator = self.indicator(window, &style)?;
                let values = BTreeMap::from([
                    ("id", Value::Text(window.id.clone())),
                    ("index", Value::Text(window.index.to_string())),
                    ("name", Value::Text(window.name.clone())),
                    ("indicator", Value::Spans(indicator)),
                ]);
                rows.push(Row {
                    spans: render_format(
                        &self.sessions.window_format,
                        &values,
                        &style,
                        &self.palette,
                    )?,
                    fill: Some(resolve_color(fill, &self.palette)?),
                    range: Some(if current {
                        Range::Window(window.index)
                    } else {
                        Range::ForeignWindow(crate::navigation::token(&session.id, &window.id)?)
                    }),
                    focus: active,
                    selected: window.selected,
                });
            }
        }
        Ok(rows)
    }

    fn indicator(&self, window: &Window, window_style: &str) -> Result<Vec<Span>, String> {
        let Some(indicator) = &self.sessions.config.indicator else {
            return Ok(Vec::new());
        };
        for rule in &indicator.rules {
            if rule
                .when
                .iter()
                .all(|(name, expected)| window.options.get(name) == Some(expected))
            {
                return Ok(vec![Span {
                    text: rule.text.clone(),
                    style: resolve_style(&rule.style, window_style, &self.palette)?,
                }]);
            }
        }
        let text = window
            .options
            .get(&indicator.source)
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| indicator.fallback.clone());
        if text.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![Span {
                text,
                style: resolve_style(&indicator.style, window_style, &self.palette)?,
            }])
        }
    }

    fn render_pi_live(&self, sessions: &[crate::PiSession]) -> Result<Vec<Row>, String> {
        let mut ordered = sessions.to_vec();
        crate::pi_live::sort_sessions(&mut ordered);
        let mut labels: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for session in &ordered {
            labels
                .entry(crate::pi_live::project_label(&session.project))
                .or_default()
                .insert(&session.project);
        }
        let mut rows = Vec::new();
        let mut previous = None;
        for session in &ordered {
            if previous != Some(session.project.as_str()) {
                let mut title = crate::pi_live::project_label(&session.project).to_owned();
                if labels[title.as_str()].len() > 1 {
                    if let Some(parent) = std::path::Path::new(&session.project)
                        .parent()
                        .and_then(std::path::Path::file_name)
                        .and_then(|name| name.to_str())
                    {
                        title = format!("{parent}/{title}");
                    }
                }
                rows.push(Row {
                    spans: vec![Span {
                        text: format!(" {title}"),
                        style: resolve_style(
                            &self.config.pi_live.project_style,
                            "default",
                            &self.palette,
                        )?,
                    }],
                    fill: Some("default".into()),
                    range: None,
                    focus: false,
                    selected: false,
                });
                previous = Some(session.project.as_str());
            }
            rows.push(self.render_pi_session(session)?);
        }
        Ok(rows)
    }

    fn render_pi_session(&self, session: &crate::PiSession) -> Result<Row, String> {
        let icon_style = match session.state.as_str() {
            "notify" => &self.config.pi_live.notify_style,
            "working" => &self.config.pi_live.working_style,
            _ => &self.config.pi_live.idle_style,
        };
        let icon_style = resolve_style(icon_style, "default", &self.palette)?;
        let selected_fill = resolve_color(&self.config.pi_live.selected_fill, &self.palette)?;
        let icon_style = if session.selected {
            format!("default,{icon_style},bg={selected_fill}")
        } else {
            icon_style
        };
        let row_style = if session.selected {
            format!(
                "default,{}",
                resolve_style(
                    &self.config.pi_live.selected_style,
                    "default",
                    &self.palette
                )?
            )
        } else {
            "default".into()
        };
        let values = BTreeMap::from([
            ("name", Value::Text(session.name.clone())),
            (
                "state",
                Value::Spans(vec![Span {
                    text: "●".into(),
                    style: icon_style,
                }]),
            ),
        ]);
        let range = session
            .target
            .as_ref()
            .map(|target| {
                crate::navigation::pane_token(&target.pane, &target.window).map(Range::PiPane)
            })
            .transpose()?;
        Ok(Row {
            spans: render_format(&self.pi_live_format, &values, &row_style, &self.palette)?,
            fill: Some(if session.selected {
                selected_fill
            } else {
                "default".into()
            }),
            range,
            focus: session.selected,
            selected: session.selected,
        })
    }

    fn render_divider(&self, width: usize) -> Result<Row, String> {
        let glyph = self.config.divider.character.as_str();
        let glyph_width = UnicodeWidthStr::width(glyph);
        let count = width.saturating_sub(2) / glyph_width;
        Ok(Row {
            spans: vec![Span {
                text: format!(" {}", glyph.repeat(count)),
                style: resolve_style(&self.config.divider.style, "default", &self.palette)?,
            }],
            fill: None,
            range: None,
            focus: false,
            selected: false,
        })
    }
}

fn validate_indicator(
    indicator: &IndicatorConfig,
    options: &BTreeMap<String, String>,
    palette: &BTreeMap<String, String>,
) -> Result<(), String> {
    if !indicator.source.is_empty() && !options.contains_key(&indicator.source) {
        return Err(format!(
            "indicator source {} is not a window option alias",
            indicator.source
        ));
    }
    resolve_style(&indicator.style, "default", palette)?;
    for rule in &indicator.rules {
        if rule.when.is_empty() {
            return Err("indicator rule must contain at least one condition".into());
        }
        for alias in rule.when.keys() {
            if !options.contains_key(alias) {
                return Err(format!(
                    "indicator condition {alias} is not a window option alias"
                ));
            }
        }
        resolve_style(&rule.style, "default", palette)?;
    }
    Ok(())
}

fn validate_snapshot(snapshot: &Snapshot) -> Result<(), String> {
    if snapshot.width == 0 || snapshot.width > MAX_WIDTH {
        return Err(format!("width must be 1..{MAX_WIDTH}"));
    }
    if !snapshot
        .sessions
        .iter()
        .any(|session| session.id == snapshot.current_session)
    {
        return Err("current session not present".into());
    }
    Ok(())
}

fn valid_tmux_option(value: &str) -> bool {
    value.strip_prefix('@').is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    })
}

fn validate_name(value: &str, kind: &str) -> Result<(), String> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        Ok(())
    } else {
        Err(format!("invalid {kind} {value}"))
    }
}

fn validate_color(value: &str, palette: &BTreeMap<String, String>) -> Result<(), String> {
    let value = palette.get(value).map_or(value, String::as_str);
    let named = [
        "default",
        "terminal",
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "brightblack",
        "brightred",
        "brightgreen",
        "brightyellow",
        "brightblue",
        "brightmagenta",
        "brightcyan",
        "brightwhite",
    ];
    let valid_hex = value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
    let valid_index = value
        .strip_prefix("colour")
        .and_then(|digits| digits.parse::<u16>().ok())
        .is_some_and(|index| index <= 255);
    if named.contains(&value) || valid_hex || valid_index {
        Ok(())
    } else {
        Err(format!("invalid color {value}"))
    }
}

fn resolve_color(value: &str, palette: &BTreeMap<String, String>) -> Result<String, String> {
    validate_color(value, palette)?;
    Ok(palette
        .get(value)
        .cloned()
        .unwrap_or_else(|| value.to_owned()))
}

fn resolve_style(
    value: &str,
    inherited: &str,
    palette: &BTreeMap<String, String>,
) -> Result<String, String> {
    if value == "$style" {
        return Ok(inherited.to_owned());
    }
    let allowed = [
        "default",
        "bold",
        "nobold",
        "dim",
        "nodim",
        "underscore",
        "nounderscore",
        "italics",
        "noitalics",
        "reverse",
        "noreverse",
        "blink",
        "noblink",
        "hidden",
        "nohidden",
        "strikethrough",
        "nostrikethrough",
    ];
    let mut result = Vec::new();
    for token in value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if let Some(color) = token.strip_prefix("fg=") {
            result.push(format!("fg={}", resolve_color(color, palette)?));
        } else if let Some(color) = token.strip_prefix("bg=") {
            result.push(format!("bg={}", resolve_color(color, palette)?));
        } else if allowed.contains(&token) {
            result.push(token.to_owned());
        } else {
            return Err(format!("invalid style token {token}"));
        }
    }
    if result.is_empty() {
        Ok("default".into())
    } else {
        Ok(result.join(","))
    }
}

fn validate_format(
    nodes: &[Node],
    variables: &[&str],
    palette: &BTreeMap<String, String>,
) -> Result<(), String> {
    for node in nodes {
        match node {
            Node::Text(_) => {}
            Node::Variable(name) if variables.contains(&name.as_str()) => {}
            Node::Variable(name) => return Err(format!("unknown format variable ${name}")),
            Node::Optional(children) => {
                if !contains_any_variable(children) {
                    return Err("optional group must contain a variable".into());
                }
                validate_format(children, variables, palette)?;
            }
            Node::Styled(children, style) => {
                validate_format(children, variables, palette)?;
                resolve_style(style, "default", palette)?;
            }
        }
    }
    Ok(())
}

fn contains_any_variable(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Variable(_) => true,
        Node::Optional(children) | Node::Styled(children, _) => contains_any_variable(children),
        Node::Text(_) => false,
    })
}

fn contains_variable(nodes: &[Node], wanted: &str) -> bool {
    nodes.iter().any(|node| match node {
        Node::Variable(name) => name == wanted,
        Node::Optional(children) | Node::Styled(children, _) => contains_variable(children, wanted),
        Node::Text(_) => false,
    })
}

struct Parser {
    chars: Vec<char>,
    cursor: usize,
}

impl Parser {
    fn parse(source: &str) -> Result<Vec<Node>, String> {
        let mut parser = Self {
            chars: source.chars().collect(),
            cursor: 0,
        };
        let nodes = parser.nodes(None)?;
        if parser.cursor != parser.chars.len() {
            return Err("unexpected formatter input".into());
        }
        Ok(nodes)
    }

    fn nodes(&mut self, end: Option<char>) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        let mut text = String::new();
        while let Some(&ch) = self.chars.get(self.cursor) {
            if Some(ch) == end {
                self.cursor += 1;
                if !text.is_empty() {
                    nodes.push(Node::Text(std::mem::take(&mut text)));
                }
                return Ok(nodes);
            }
            match ch {
                '\\' => {
                    self.cursor += 1;
                    text.push(
                        *self
                            .chars
                            .get(self.cursor)
                            .ok_or("trailing formatter escape")?,
                    );
                    self.cursor += 1;
                }
                '$' => {
                    if !text.is_empty() {
                        nodes.push(Node::Text(std::mem::take(&mut text)));
                    }
                    self.cursor += 1;
                    let start = self.cursor;
                    while self
                        .chars
                        .get(self.cursor)
                        .is_some_and(|value| value.is_ascii_alphanumeric() || *value == '_')
                    {
                        self.cursor += 1;
                    }
                    if start == self.cursor {
                        return Err("empty format variable".into());
                    }
                    nodes.push(Node::Variable(
                        self.chars[start..self.cursor].iter().collect(),
                    ));
                }
                '(' => {
                    if !text.is_empty() {
                        nodes.push(Node::Text(std::mem::take(&mut text)));
                    }
                    self.cursor += 1;
                    nodes.push(Node::Optional(self.nodes(Some(')'))?));
                }
                '[' => {
                    if !text.is_empty() {
                        nodes.push(Node::Text(std::mem::take(&mut text)));
                    }
                    self.cursor += 1;
                    let children = self.nodes(Some(']'))?;
                    if self.chars.get(self.cursor) != Some(&'(') {
                        return Err("styled group must be followed by (style)".into());
                    }
                    self.cursor += 1;
                    let mut style = String::new();
                    let mut closed = false;
                    while let Some(&value) = self.chars.get(self.cursor) {
                        self.cursor += 1;
                        if value == ')' {
                            closed = true;
                            break;
                        }
                        if value == '\\' {
                            style
                                .push(*self.chars.get(self.cursor).ok_or("trailing style escape")?);
                            self.cursor += 1;
                        } else {
                            style.push(value);
                        }
                    }
                    if !closed {
                        return Err("unclosed style".into());
                    }
                    nodes.push(Node::Styled(children, style));
                }
                ')' | ']' => return Err(format!("unexpected {ch}")),
                _ => {
                    text.push(ch);
                    self.cursor += 1;
                }
            }
        }
        if let Some(end) = end {
            Err(format!("unclosed {end}"))
        } else {
            if !text.is_empty() {
                nodes.push(Node::Text(text));
            }
            Ok(nodes)
        }
    }
}

#[derive(Clone, Debug)]
enum Value {
    Text(String),
    Spans(Vec<Span>),
}

fn render_format(
    nodes: &[Node],
    values: &BTreeMap<&str, Value>,
    default_style: &str,
    palette: &BTreeMap<String, String>,
) -> Result<Vec<Span>, String> {
    Ok(render_nodes(nodes, values, default_style, palette, None)?.0)
}

fn render_nodes(
    nodes: &[Node],
    values: &BTreeMap<&str, Value>,
    default_style: &str,
    palette: &BTreeMap<String, String>,
    override_style: Option<&str>,
) -> Result<(Vec<Span>, bool), String> {
    let mut spans = Vec::new();
    let mut present = false;
    for node in nodes {
        match node {
            Node::Text(text) => spans.push(Span {
                text: text.clone(),
                style: override_style.unwrap_or(default_style).to_owned(),
            }),
            Node::Variable(name) if name == "style" => {
                return Err("$style may only appear in a style expression".into())
            }
            Node::Variable(name) => {
                let value = values
                    .get(name.as_str())
                    .ok_or_else(|| format!("missing format variable ${name}"))?;
                match value {
                    Value::Text(text) => {
                        present |= !text.is_empty();
                        spans.push(Span {
                            text: text.clone(),
                            style: override_style.unwrap_or(default_style).to_owned(),
                        });
                    }
                    Value::Spans(value) => {
                        present |= value.iter().any(|span| !span.text.is_empty());
                        spans.extend(value.iter().cloned().map(|mut span| {
                            if let Some(style) = override_style {
                                span.style = style.to_owned();
                            }
                            span
                        }));
                    }
                }
            }
            Node::Optional(children) => {
                let (child_spans, child_present) =
                    render_nodes(children, values, default_style, palette, override_style)?;
                if child_present {
                    present = true;
                    spans.extend(child_spans);
                }
            }
            Node::Styled(children, style) => {
                let style = resolve_style(style, default_style, palette)?;
                let (child_spans, child_present) =
                    render_nodes(children, values, default_style, palette, Some(&style))?;
                present |= child_present;
                spans.extend(child_spans);
            }
        }
    }
    Ok((coalesce(spans), present))
}

fn coalesce(spans: Vec<Span>) -> Vec<Span> {
    let mut result: Vec<Span> = Vec::new();
    for span in spans {
        if span.text.is_empty() {
            continue;
        }
        if let Some(previous) = result.last_mut().filter(|value| value.style == span.style) {
            previous.text.push_str(&span.text);
        } else {
            result.push(span);
        }
    }
    result
}

#[derive(Clone, Debug)]
struct Span {
    text: String,
    style: String,
}

#[derive(Clone, Debug)]
struct Row {
    spans: Vec<Span>,
    fill: Option<String>,
    range: Option<Range>,
    focus: bool,
    selected: bool,
}

#[derive(Clone, Debug)]
enum Range {
    Session(String),
    Window(usize),
    ForeignWindow(String),
    PiPane(String),
}

fn escaped(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .replace('#', "##")
}

fn clipped(text: &str, remaining: &mut usize) -> String {
    let mut result = String::new();
    for grapheme in text.graphemes(true) {
        let width = UnicodeWidthStr::width(grapheme);
        if width > *remaining {
            break;
        }
        *remaining -= width;
        result.push_str(grapheme);
    }
    result
}

fn render_rows(rows: &[Row], width: usize) -> String {
    let mut result = String::from(
        "#[list=on]#[list=left-marker]#[acs]-#[noacs]#[nl]#[list=right-marker]#[acs].#[noacs]#[nl]",
    );
    for row in rows {
        let mut remaining = width;
        if let Some(range) = &row.range {
            match range {
                Range::Session(id) => result.push_str(&format!("#[range=session|{id} ]")),
                Range::Window(index) => result.push_str(&format!(
                    "#[range=window|{index} {}]",
                    if row.focus { "list=focus " } else { "" }
                )),
                Range::ForeignWindow(token) | Range::PiPane(token) => {
                    result.push_str(&format!("#[range=user|{token} ]"))
                }
            }
        }
        for span in &row.spans {
            let text = clipped(&span.text, &mut remaining);
            if text.is_empty() {
                continue;
            }
            if span.style == "default" {
                result.push_str("#[default]");
            } else {
                result.push_str(&format!("#[{}]", span.style));
            }
            result.push_str(&escaped(&text));
        }
        if let Some(fill) = &row.fill {
            result.push_str(&format!("#[bg={fill}]"));
            result.push_str(&" ".repeat(remaining.saturating_sub(1)));
        }
        match row.range {
            Some(Range::Session(_)) => result.push_str("#[norange default]"),
            Some(Range::Window(_) | Range::ForeignWindow(_) | Range::PiPane(_)) if row.selected => {
                result.push_str("#[norange]#[list=on default]")
            }
            Some(Range::Window(_) | Range::ForeignWindow(_) | Range::PiPane(_)) => {
                result.push_str("#[norange default]")
            }
            None => {}
        }
        if let Some(fill) = &row.fill {
            result.push_str(&format!("#[fill={fill}]"));
        }
        result.push_str("#[nl]");
        if row.fill.is_some() {
            result.push_str("#[fill=default]");
        }
    }
    result
}
