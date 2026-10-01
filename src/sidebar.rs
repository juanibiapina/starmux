mod colorscheme;
mod slim;

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
    pub status_lines: usize,
    pub current_session: String,
    pub current_pane: String,
    pub pane_path: String,
    pub sessions: Vec<Session>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    modules: Vec<String>,
    slim: SlimConfig,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    actions: BTreeMap<String, crate::actions::Action>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    clicks: Vec<crate::actions::Rule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_dir: Option<String>,
    top: TopConfig,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    configs: BTreeMap<String, ModuleList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    colorscheme: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    colorschemes: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    palettes: BTreeMap<String, BTreeMap<String, String>>,
    sessions: SessionsConfig,
    divider: DividerConfig,
    #[serde(rename = "pi-workbench")]
    pi_workbench: PiWorkbenchConfig,
    usage: UsageConfig,
    gob: GobConfig,
    git: GitConfig,
    debug: DebugConfig,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    commands: BTreeMap<String, CommandConfig>,
    #[serde(rename = "pi-context")]
    pi_context: PiContextConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ModuleList {
    modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slim_modules: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum LayoutMode {
    #[default]
    Auto,
    Full,
    Slim,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct SlimConfig {
    mode: LayoutMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    modules: Option<Vec<String>>,
    show_windows: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct TopConfig {
    disabled: bool,
    metrics: Vec<String>,
    heading_style: String,
    value_style: String,
    warning_style: String,
    critical_style: String,
    track_style: String,
}

impl Default for TopConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            metrics: vec!["cpu".into(), "memory".into(), "battery".into()],
            heading_style: "bold".into(),
            value_style: "default".into(),
            warning_style: "fg=yellow".into(),
            critical_style: "fg=red".into(),
            track_style: "dim".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct DebugConfig {
    disabled: bool,
    details: bool,
    style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_dir: Option<String>,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            details: false,
            style: "dim".into(),
            cache_dir: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct PiContextConfig {
    disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_command: Option<Vec<String>>,
    heading_style: String,
    category_style: String,
    text_style: String,
    plan_style: String,
    skill_style: String,
    open_style: String,
    draft_style: String,
    merged_style: String,
    closed_style: String,
    unknown_style: String,
}

impl Default for PiContextConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            open_command: None,
            heading_style: "fg=magenta,bold".into(),
            category_style: "dim".into(),
            text_style: "default".into(),
            plan_style: "fg=magenta".into(),
            skill_style: "fg=magenta".into(),
            open_style: "fg=green".into(),
            draft_style: "fg=brightblack".into(),
            merged_style: "fg=magenta".into(),
            closed_style: "fg=red".into(),
            unknown_style: "fg=brightblack".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct GitConfig {
    disabled: bool,
    lines: Vec<String>,
    branch_style: String,
    upstream_style: String,
    divergence_style: String,
    staged_style: String,
    modified_style: String,
    untracked_style: String,
    conflicts_style: String,
    stash_style: String,
    added_style: String,
    deleted_style: String,
    clean_style: String,
    state_style: String,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            lines: vec![
                " $branch( $upstream)( $divergence)".into(),
                " ( $conflicts)( $staged)( $modified)( $untracked)( $stash)( $added)( $deleted)( $clean)".into(),
                " ( $state)".into(),
            ],
            branch_style: "bold".into(),
            upstream_style: "fg=cyan".into(),
            divergence_style: "fg=yellow".into(),
            staged_style: "fg=green".into(),
            modified_style: "fg=red".into(),
            untracked_style: "fg=magenta".into(),
            conflicts_style: "fg=red,bold".into(),
            stash_style: "fg=cyan".into(),
            added_style: "fg=green".into(),
            deleted_style: "fg=red".into(),
            clean_style: "fg=green".into(),
            state_style: "fg=red,bold".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct GobConfig {
    disabled: bool,
    format: String,
    heading_style: String,
    running_style: String,
    progress_style: String,
    bar_track_color: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct CommandConfig {
    argv: Vec<String>,
    output: String,
    style: String,
    prefix: String,
    slim_icon: String,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            output: "text".into(),
            style: "default".into(),
            prefix: String::new(),
            slim_icon: "".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct UsageConfig {
    disabled: bool,
    providers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_dir: Option<String>,
    format: String,
    provider_style: String,
    window_style: String,
    warning_bar_style: String,
    critical_bar_style: String,
    bar_track_color: String,
    stale_style: String,
    unavailable_style: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct SessionsConfig {
    disabled: bool,
    show_windows: bool,
    fold_inactive: bool,
    active_only: bool,
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
struct PiWorkbenchConfig {
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
            slim: SlimConfig::default(),
            actions: BTreeMap::new(),
            clicks: Vec::new(),
            cache_dir: None,
            top: TopConfig::default(),
            configs: BTreeMap::new(),
            colorscheme: None,
            colorschemes: BTreeMap::new(),
            palette: None,
            palettes: BTreeMap::new(),
            sessions: SessionsConfig::default(),
            divider: DividerConfig::default(),
            pi_workbench: PiWorkbenchConfig::default(),
            usage: UsageConfig::default(),
            gob: GobConfig::default(),
            git: GitConfig::default(),
            debug: DebugConfig::default(),
            commands: BTreeMap::new(),
            pi_context: PiContextConfig::default(),
        }
    }
}

impl Default for GobConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            format: "  $state $name".into(),
            heading_style: "bold".into(),
            running_style: "fg=green".into(),
            progress_style: "fg=green".into(),
            bar_track_color: "colour238".into(),
        }
    }
}

const DEFAULT_USAGE_FORMAT: &str = "  $name( $remaining) $bar $percent";

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            providers: Vec::new(),
            cache_dir: None,
            format: DEFAULT_USAGE_FORMAT.into(),
            provider_style: "bold".into(),
            window_style: "default".into(),
            warning_bar_style: "fg=colour208".into(),
            critical_bar_style: "fg=red".into(),
            bar_track_color: "colour238".into(),
            stale_style: "dim".into(),
            unavailable_style: "dim".into(),
        }
    }
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            show_windows: true,
            fold_inactive: false,
            active_only: false,
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

impl Default for PiWorkbenchConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            data_dir: None,
            format: "  $state $name".into(),
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

#[derive(Clone, Copy)]
pub struct RenderInputs<'a> {
    pub top: Option<&'a crate::top::HostStatus>,
    pub pi_sessions: &'a [crate::PiSession],
    pub usage_rows: &'a [crate::usage::UsageRow],
    pub gob_jobs: &'a [crate::gob::GobJob],
    pub context: Option<&'a crate::PiContext>,
    pub states: &'a [crate::pr_state::PrState],
    pub commands: &'a BTreeMap<String, String>,
    pub git: Option<&'a crate::GitStatus>,
    pub debug: Option<&'a crate::debug::Diagnostics>,
}

#[derive(Clone, Debug)]
pub struct Sidebar {
    config: Config,
    selected: String,
    width: usize,
    palette: BTreeMap<String, String>,
    sessions: CompiledSessions,
    pi_workbench_format: Vec<Node>,
    usage_format: Vec<Node>,
    gob_format: Vec<Node>,
    git_lines: Vec<Vec<Node>>,
    actions: crate::actions::Actions,
}

fn merge_missing(input: &mut toml::Value, defaults: toml::Value) {
    if let (Some(input), toml::Value::Table(defaults)) = (input.as_table_mut(), defaults) {
        for (key, value) in defaults {
            if let Some(existing) = input.get_mut(&key) {
                merge_missing(existing, value);
            } else {
                input.insert(key, value);
            }
        }
    }
}

impl Sidebar {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        if text.trim().is_empty() {
            return Self::defaults();
        }
        let mut input: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
        let selected: Config = input
            .clone()
            .try_into()
            .map_err(|error: toml::de::Error| error.to_string())?;
        if let Some(name) = &selected.colorscheme {
            let (defaults, _) = colorscheme::load(name, &selected.colorschemes)?;
            merge_missing(&mut input, defaults);
            // Named text commands also inherit the scheme unless explicitly styled.
            if let Some(commands) = input
                .get_mut("commands")
                .and_then(toml::Value::as_table_mut)
            {
                for (_, command) in commands.iter_mut() {
                    if let Some(table) = command.as_table_mut() {
                        table
                            .entry("style")
                            .or_insert_with(|| toml::Value::String("fg=text".into()));
                    }
                }
            }
        }
        let config = input
            .try_into()
            .map_err(|error: toml::de::Error| error.to_string())?;
        Self::compile(config)
    }

    pub fn defaults() -> Result<Self, String> {
        Self::compile(Config::default())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Configuration validation and compiled formats form one atomic construction pass"
    )]
    fn compile(config: Config) -> Result<Self, String> {
        let actions = crate::actions::Actions::compile(
            &config.actions,
            &config.clicks,
            &config.configs.keys().cloned().collect(),
            &config.commands.keys().cloned().collect(),
        )?;
        let known = [
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
        let mut used = BTreeSet::new();
        if config.commands.len() > 4 {
            return Err("at most 4 commands may be configured".into());
        }
        for name in config.configs.keys() {
            if !valid_command_name(name) || name == "default" {
                return Err(format!("invalid config name {name}"));
            }
        }
        for (label, modules) in std::iter::once(("default", &config.modules))
            .chain(config.configs.iter().flat_map(|(name, list)| {
                std::iter::once((name.as_str(), &list.modules)).chain(
                    list.slim_modules
                        .as_ref()
                        .map(|modules| (name.as_str(), modules)),
                )
            }))
            .chain(
                config
                    .slim
                    .modules
                    .as_ref()
                    .map(|modules| ("default slim", modules)),
            )
        {
            let mut seen = BTreeSet::new();
            for name in modules {
                if let Some(id) = name.strip_prefix("command.") {
                    if !valid_command_name(id) || !config.commands.contains_key(id) {
                        return Err(format!("config {label}: unknown command module {name}"));
                    }
                } else if !known.contains(&name.as_str()) {
                    return Err(format!("config {label}: unknown module {name}"));
                }
                if name != "divider" && name != "blank" && !seen.insert(name) {
                    return Err(format!("config {label}: duplicate module {name}"));
                }
                used.insert(name.as_str());
            }
        }
        if config
            .cache_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("cache_dir must be an absolute path".into());
        }
        let mut metrics = BTreeSet::new();
        for metric in &config.top.metrics {
            if !matches!(metric.as_str(), "cpu" | "memory" | "battery") || !metrics.insert(metric) {
                return Err(format!("invalid or duplicate top metric {metric}"));
            }
        }
        for (name, command) in &config.commands {
            if !valid_command_name(name) || !used.contains(format!("command.{name}").as_str()) {
                return Err(format!("unused or invalid command {name}"));
            }
            if command.argv.is_empty()
                || command.argv.len() > 16
                || command
                    .argv
                    .iter()
                    .any(|arg| arg.len() > 1024 || arg.contains('\0'))
                || command.argv[0].is_empty()
            {
                return Err(format!("command.{name} argv must have 1–16 nonempty executable arguments of at most 1024 bytes"));
            }
            if !matches!(command.output.as_str(), "text" | "tmux-styles") {
                return Err(format!("invalid command.{name} output mode"));
            }
            if command.slim_icon.graphemes(true).count() != 1
                || UnicodeWidthStr::width(command.slim_icon.as_str()) != 1
                || command
                    .slim_icon
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(format!(
                    "invalid command.{name} slim_icon: expected one visible column"
                ));
            }
            if command.prefix.len() > 64 || command.prefix.chars().any(char::is_control) {
                return Err(format!("invalid command.{name} prefix"));
            }
        }
        for (name, colors) in &config.colorschemes {
            if !valid_command_name(name) || colorscheme::is_builtin(name) {
                return Err(format!("invalid or reserved colorscheme name {name}"));
            }
            colorscheme::validate_colors(name, colors)?;
        }
        let mut palette = match &config.colorscheme {
            Some(name) => colorscheme::load(name, &config.colorschemes)?.1,
            None => BTreeMap::new(),
        };
        if let Some(name) = &config.palette {
            let custom = config
                .palettes
                .get(name)
                .ok_or_else(|| format!("unknown palette {name}"))?;
            palette.extend(custom.clone());
        }
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
        let pi_workbench_format = Parser::parse(&config.pi_workbench.format)?;
        validate_format(&pi_workbench_format, &["name", "state"], &palette)?;
        let gob_format = Parser::parse(&config.gob.format)?;
        validate_format(&gob_format, &["id", "name", "state"], &palette)?;
        if config.git.lines.len() > 12 {
            return Err("git lines may contain at most 12 rows".into());
        }
        let git_lines = config
            .git
            .lines
            .iter()
            .map(|line| {
                let nodes = Parser::parse(line)?;
                validate_format(
                    &nodes,
                    &[
                        "branch",
                        "upstream",
                        "ahead",
                        "behind",
                        "divergence",
                        "staged",
                        "modified",
                        "untracked",
                        "conflicts",
                        "stash",
                        "added",
                        "deleted",
                        "clean",
                        "state",
                    ],
                    &palette,
                )?;
                Ok(nodes)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let usage_format = Parser::parse(&config.usage.format)?;
        validate_format(
            &usage_format,
            &["name", "remaining", "percent", "bar"],
            &palette,
        )?;
        let mut providers = BTreeSet::new();
        for provider in &config.usage.providers {
            if !crate::usage::PROVIDERS.contains(&provider.as_str()) {
                return Err(format!("unknown usage provider {provider}"));
            }
            if !providers.insert(provider) {
                return Err(format!("duplicate usage provider {provider}"));
            }
        }
        if config
            .usage
            .cache_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("usage cache_dir must be an absolute path".into());
        }
        if config
            .pi_workbench
            .data_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("pi-workbench data_dir must be an absolute path".into());
        }
        if config
            .debug
            .cache_dir
            .as_deref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err("debug cache_dir must be an absolute path".into());
        }
        if let Some(command) = &config.pi_context.open_command {
            if command.is_empty()
                || command.len() > 16
                || command[0].is_empty()
                || command[0].contains('{')
                || command[0].contains('}')
                || command
                    .iter()
                    .any(|arg| arg.len() > 1024 || arg.contains('\0'))
                || command
                    .iter()
                    .filter(|arg| arg.as_str() == "{file}")
                    .count()
                    != 1
                || command.iter().skip(1).any(|arg| {
                    (arg.contains('{') || arg.contains('}'))
                        && !matches!(arg.as_str(), "{file}" | "{pane}" | "{socket}")
                })
            {
                return Err("pi-context open_command must be argv with one {file} argument and optional {pane} and {socket} arguments".into());
            }
        }
        for style in [
            &config.top.heading_style,
            &config.top.value_style,
            &config.top.warning_style,
            &config.top.critical_style,
            &config.top.track_style,
            &config.sessions.current_session_style,
            &config.sessions.other_session_style,
            &config.sessions.active_window_style,
            &config.sessions.selected_window_style,
            &config.sessions.other_window_style,
            &config.divider.style,
            &config.pi_workbench.project_style,
            &config.pi_workbench.idle_style,
            &config.pi_workbench.working_style,
            &config.pi_workbench.notify_style,
            &config.pi_workbench.selected_style,
            &config.usage.provider_style,
            &config.usage.window_style,
            &config.usage.warning_bar_style,
            &config.usage.critical_bar_style,
            &config.usage.stale_style,
            &config.usage.unavailable_style,
            &config.pi_context.heading_style,
            &config.pi_context.category_style,
            &config.pi_context.text_style,
            &config.pi_context.plan_style,
            &config.pi_context.skill_style,
            &config.pi_context.open_style,
            &config.pi_context.draft_style,
            &config.pi_context.merged_style,
            &config.pi_context.closed_style,
            &config.pi_context.unknown_style,
            &config.gob.heading_style,
            &config.gob.running_style,
            &config.gob.progress_style,
            &config.git.branch_style,
            &config.git.upstream_style,
            &config.git.divergence_style,
            &config.git.staged_style,
            &config.git.modified_style,
            &config.git.untracked_style,
            &config.git.conflicts_style,
            &config.git.stash_style,
            &config.git.added_style,
            &config.git.deleted_style,
            &config.git.clean_style,
            &config.git.state_style,
            &config.debug.style,
        ] {
            resolve_style(style, "default", &palette)?;
        }
        for (name, command) in &config.commands {
            resolve_style(&command.style, "default", &palette)
                .map_err(|error| format!("command.{name} style: {error}"))?;
        }
        validate_color(&config.usage.bar_track_color, &palette)?;
        validate_color(&config.gob.bar_track_color, &palette)?;
        for fill in [
            &config.sessions.current_session_fill,
            &config.sessions.other_session_fill,
            &config.sessions.active_window_fill,
            &config.sessions.selected_window_fill,
            &config.sessions.other_window_fill,
            &config.pi_workbench.selected_fill,
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
            selected: "default".into(),
            width: 30,
            palette,
            sessions,
            pi_workbench_format,
            usage_format,
            actions,
            gob_format,
            git_lines,
        })
    }

    pub fn select(mut self, name: &str) -> Result<Self, String> {
        if name != "default" && !self.config.configs.contains_key(name) {
            return Err(format!("unknown config {name}"));
        }
        self.selected = name.to_owned();
        Ok(self)
    }

    pub fn selected_config(&self) -> &str {
        &self.selected
    }

    pub(crate) fn for_width(&self, width: usize) -> Self {
        let mut view = self.clone();
        view.width = width;
        view
    }

    fn is_slim(&self) -> bool {
        match self.config.slim.mode {
            LayoutMode::Auto => self.width == 2,
            LayoutMode::Full => false,
            LayoutMode::Slim => true,
        }
    }

    pub(crate) fn layout_name(&self) -> &'static str {
        if self.is_slim() {
            "slim"
        } else {
            "full"
        }
    }

    pub(crate) fn scroll_config(&self) -> String {
        if self.is_slim() {
            format!("{}/slim", self.selected)
        } else {
            self.selected.clone()
        }
    }

    fn show_windows(&self) -> bool {
        if self.is_slim() {
            self.config.slim.show_windows
        } else {
            self.sessions.config.show_windows
        }
    }

    fn modules(&self) -> &[String] {
        if self.selected == "default" {
            if self.is_slim() {
                if let Some(modules) = &self.config.slim.modules {
                    return modules;
                }
            }
            &self.config.modules
        } else {
            let list = &self.config.configs[&self.selected];
            if self.is_slim() {
                if let Some(modules) = &list.slim_modules {
                    return modules;
                }
            }
            &list.modules
        }
    }

    pub fn requested_window_options(&self) -> Vec<(String, String)> {
        if self.sessions.config.disabled
            || !self.modules().iter().any(|name| name == "sessions")
            || !self.show_windows()
            || self.is_slim()
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

    pub(crate) fn pi_workbench_data_dir(&self) -> Option<&str> {
        if (self.config.pi_workbench.disabled
            || !self.modules().iter().any(|name| name == "pi-workbench"))
            && !self.pi_context_enabled()
        {
            return None;
        }
        Some(self.config.pi_workbench.data_dir.as_deref().unwrap_or(""))
    }

    pub(crate) fn pi_context_enabled(&self) -> bool {
        !self.config.pi_context.disabled && self.modules().iter().any(|name| name == "pi-context")
    }

    pub(crate) fn file_open_args(
        &self,
        file: &std::path::Path,
        pane: &str,
        socket: &str,
    ) -> Option<Vec<std::ffi::OsString>> {
        self.config.pi_context.open_command.as_ref().map(|command| {
            command
                .iter()
                .map(|arg| match arg.as_str() {
                    "{file}" => file.as_os_str().to_owned(),
                    "{pane}" => pane.into(),
                    "{socket}" => socket.into(),
                    _ => arg.into(),
                })
                .collect()
        })
    }

    pub fn cache_dir_for(&self, name: &str) -> Option<std::path::PathBuf> {
        if !matches!(name, "top" | "usage" | "pr-state" | "debug" | "scroll") {
            return None;
        }
        crate::cache::root(self.config.cache_dir.as_deref()).map(|root| root.join(name))
    }

    pub(crate) fn cache_dir(&self, name: &str) -> Option<std::path::PathBuf> {
        self.cache_dir_for(name)
    }

    pub(crate) fn top_enabled(&self) -> bool {
        !self.config.top.disabled && self.modules().iter().any(|name| name == "top")
    }

    pub(crate) fn debug_cache_dir(&self) -> Option<Option<&str>> {
        (!self.config.debug.disabled && self.modules().iter().any(|name| name == "debug"))
            .then_some(self.config.debug.cache_dir.as_deref())
    }

    pub(crate) fn git_enabled(&self) -> bool {
        !self.config.git.disabled && self.modules().iter().any(|name| name == "git")
    }

    pub(crate) fn gob_enabled(&self) -> bool {
        !self.config.gob.disabled && self.modules().iter().any(|name| name == "gob")
    }

    pub(crate) fn usage_options(&self) -> Option<(&[String], Option<&str>)> {
        if self.config.usage.disabled || !self.modules().iter().any(|name| name == "usage") {
            return None;
        }
        Some((
            &self.config.usage.providers,
            self.config.usage.cache_dir.as_deref(),
        ))
    }

    pub(crate) fn command_options(&self) -> Vec<(&str, &[String])> {
        self.modules()
            .iter()
            .filter_map(|name| {
                let id = name.strip_prefix("command.")?;
                Some((name.as_str(), self.config.commands.get(id)?.argv.as_slice()))
            })
            .collect()
    }

    pub fn render(&self, snapshot: &Snapshot) -> Result<String, String> {
        self.render_with_usage(snapshot, &[], &[])
    }

    pub fn render_with_pi_workbench(
        &self,
        snapshot: &Snapshot,
        pi_sessions: &[crate::PiSession],
    ) -> Result<String, String> {
        self.render_with_usage(snapshot, pi_sessions, &[])
    }

    pub fn render_with_usage(
        &self,
        snapshot: &Snapshot,
        pi_sessions: &[crate::PiSession],
        usage_rows: &[crate::usage::UsageRow],
    ) -> Result<String, String> {
        self.render_with_modules(snapshot, pi_sessions, usage_rows, &[])
    }

    pub fn render_with_modules(
        &self,
        snapshot: &Snapshot,
        pi_sessions: &[crate::PiSession],
        usage_rows: &[crate::usage::UsageRow],
        gob_jobs: &[crate::gob::GobJob],
    ) -> Result<String, String> {
        self.render_with_inputs(
            snapshot,
            RenderInputs {
                top: None,
                pi_sessions,
                usage_rows,
                gob_jobs,
                context: None,
                states: &[],
                commands: &BTreeMap::new(),
                git: None,
                debug: None,
            },
        )
    }

    pub fn render_with_inputs(
        &self,
        snapshot: &Snapshot,
        input: RenderInputs<'_>,
    ) -> Result<String, String> {
        self.render_scrolled(snapshot, input, 0)
            .map(|(text, _, _)| text)
    }

    pub fn render_scrolled(
        &self,
        snapshot: &Snapshot,
        input: RenderInputs<'_>,
        offset: usize,
    ) -> Result<(String, usize, usize), String> {
        if self.width != snapshot.width {
            return self
                .for_width(snapshot.width)
                .render_scrolled(snapshot, input, offset);
        }
        let height = snapshot.client_height.saturating_sub(snapshot.status_lines);
        let rows = self.rows(snapshot, input)?;
        let text_color = self
            .config
            .colorscheme
            .as_ref()
            .and_then(|_| self.palette.get("text"));
        let max_offset = rows.len().saturating_sub(height);
        let offset = offset.min(max_offset);
        Ok((
            render_rows(
                &rows[offset..rows.len().min(offset.saturating_add(height))],
                snapshot.width,
                text_color.map(String::as_str),
            ),
            offset,
            max_offset,
        ))
    }

    fn rows(&self, snapshot: &Snapshot, input: RenderInputs<'_>) -> Result<Vec<Row>, String> {
        if self.width != snapshot.width {
            return self.for_width(snapshot.width).rows(snapshot, input);
        }
        let RenderInputs {
            top,
            pi_sessions,
            usage_rows,
            gob_jobs,
            context,
            states,
            commands,
            git,
            debug,
        } = input;
        validate_snapshot(snapshot)?;
        let mut rows = Vec::new();
        let mut spacer = None;
        let mut instances = BTreeMap::<String, usize>::new();
        for name in self.modules() {
            let instance = instances.entry(name.clone()).or_default();
            *instance += 1;
            let before = rows.len();
            match name.as_str() {
                "top" if !self.config.top.disabled => {
                    rows.extend(self.render_top(top, snapshot.width)?)
                }
                "sessions" if !self.sessions.config.disabled => {
                    rows.extend(self.render_sessions(snapshot)?)
                }
                "divider" if !self.config.divider.disabled => {
                    rows.push(self.render_divider(snapshot.width)?)
                }
                "pi-workbench" if !self.config.pi_workbench.disabled => {
                    rows.extend(self.render_pi_workbench(pi_sessions)?)
                }
                "usage" if !self.config.usage.disabled => {
                    rows.extend(self.render_usage(usage_rows, snapshot.width)?)
                }
                "pi-context" if !self.config.pi_context.disabled => {
                    if let Some(context) = context {
                        rows.extend(
                            self.render_pi_context(
                                context,
                                pi_sessions
                                    .iter()
                                    .find(|session| session.selected)
                                    .map(|session| session.name.as_str()),
                                states,
                                snapshot.width,
                                &snapshot.current_pane,
                            )?,
                        );
                    }
                }
                "gob" if !self.config.gob.disabled => {
                    rows.extend(self.render_gob(gob_jobs, snapshot.width)?)
                }
                "git" if !self.config.git.disabled => {
                    if let Some(status) = git {
                        rows.extend(self.render_git(status)?);
                    }
                }
                name if name.starts_with("command.") => {
                    if let Some(text) = commands.get(name) {
                        rows.push(self.render_command(name, text)?);
                    }
                }
                "debug" if !self.config.debug.disabled => {
                    rows.extend(self.render_debug(debug)?);
                }
                "spacer" => spacer = Some(rows.len()),
                "blank" => rows.push(Row::blank()),
                "sessions" | "divider" | "pi-workbench" | "usage" | "gob" | "git"
                | "pi-context" | "debug" | "top" => {}
                _ => return Err(format!("unknown module {name}")),
            }
            if self.is_slim() {
                let module_rows = rows.split_off(before);
                rows.extend(self.slim_rows(name, module_rows, snapshot, input)?);
            }
            for row in &mut rows[before..] {
                row.identity.module = name.clone();
                row.identity.instance = *instance;
            }
        }
        // The two list marker newlines precede the visible side status rows.
        let height = snapshot.client_height.saturating_sub(snapshot.status_lines);
        let mut rows = layout_rows(rows, spacer, height);
        self.apply_actions(&mut rows, snapshot)?;
        Ok(rows)
    }

    fn apply_actions(&self, rows: &mut [Row], snapshot: &Snapshot) -> Result<(), String> {
        let mut padding_slot = 0;
        let mut occurrences = BTreeMap::<String, usize>::new();
        for row in rows {
            if row.identity.module.is_empty() {
                padding_slot += 1;
                row.identity = crate::actions::Identity::new("padding").field("slot", padding_slot);
                row.identity.module = "spacer".into();
                row.identity.instance = 1;
            }
            row.identity.presentation = format!(
                "{}:{}:{}:{:?}",
                self.layout_name(),
                self.width,
                self.show_windows(),
                self.modules()
            );
            let key = format!("{:?}", row.identity);
            let occurrence = occurrences.entry(key).or_default();
            *occurrence += 1;
            row.identity = row.identity.clone().field("occurrence", *occurrence);
            match self
                .actions
                .resolve(&row.identity, snapshot, &self.selected)?
            {
                crate::actions::Choice::Default | crate::actions::Choice::Unavailable(_) => {}
                crate::actions::Choice::None => row.range = None,
                crate::actions::Choice::Command { token, .. } => {
                    row.range = Some(Range::Action(token))
                }
            }
        }
        Ok(())
    }

    pub(crate) fn action_warnings(
        &self,
        snapshot: &Snapshot,
        input: RenderInputs<'_>,
    ) -> Result<Vec<String>, String> {
        self.rows(snapshot, input)?
            .iter()
            .filter_map(|row| {
                match self
                    .actions
                    .resolve(&row.identity, snapshot, &self.selected)
                {
                    Ok(crate::actions::Choice::Unavailable(reason)) => Some(Ok(format!(
                        "{} {:?}: {reason}",
                        row.identity.module, row.identity
                    ))),
                    Err(error) => Some(Err(error)),
                    _ => None,
                }
            })
            .collect()
    }

    pub(crate) fn action_args(
        &self,
        snapshot: &Snapshot,
        input: RenderInputs<'_>,
        request: (&str, &str, &str),
    ) -> Result<Vec<String>, String> {
        let (token, socket, client) = request;
        let rows = self.rows(snapshot, input)?;
        let mut matches = rows
            .iter()
            .filter(|row| matches!(&row.range, Some(Range::Action(current)) if current == token));
        let row = matches
            .next()
            .ok_or("action click target is no longer present")?;
        if matches.next().is_some() {
            return Err("ambiguous action click target".into());
        }
        self.actions
            .instantiate(&row.identity, snapshot, (&self.selected, socket, client))?
            .ok_or_else(|| "action is no longer available".into())
    }

    pub fn print_config(&self) -> Result<String, String> {
        toml::to_string_pretty(&self.config).map_err(|error| error.to_string())
    }

    pub(crate) fn module_names(&self) -> &[String] {
        self.modules()
    }

    fn render_top(
        &self,
        status: Option<&crate::top::HostStatus>,
        width: usize,
    ) -> Result<Vec<Row>, String> {
        let config = &self.config.top;
        if config.metrics.is_empty() {
            return Ok(Vec::new());
        }
        let style = |value: &str| resolve_style(value, "default", &self.palette);
        let mut rows = vec![Row {
            divider: false,
            identity: crate::actions::Identity::new("heading"),
            spans: vec![Span {
                text: " SYSTEM".into(),
                style: style(&config.heading_style)?,
            }],
            fill: None,
            range: None,
            focus: false,
            selected: false,
        }];
        let stale = status.and_then(|status| status.age_seconds);
        for metric in &config.metrics {
            let battery = status.and_then(|status| status.battery.as_ref());
            if metric == "battery" && battery.is_none() && status.is_some() {
                continue;
            }
            let (icon, label, value) = match metric.as_str() {
                "cpu" => ("󰻠", "CPU", status.and_then(|s| s.cpu)),
                "memory" => ("󰍛", "MEM", status.and_then(|s| s.memory)),
                "battery" => (
                    battery.map_or("󰁹", |b| {
                        if b.full {
                            "󱟢"
                        } else if b.charging {
                            "󰂄"
                        } else if b.percent <= 20 {
                            "󰁺"
                        } else {
                            "󰁹"
                        }
                    }),
                    "BAT",
                    battery.map(|b| b.percent),
                ),
                _ => continue,
            };
            let alert = match metric.as_str() {
                "battery" => value.is_some_and(|value| value <= 20),
                _ => value.is_some_and(|value| value >= 80),
            };
            let color = if stale.is_some() || value.is_none() {
                &config.track_style
            } else if alert {
                &config.critical_style
            } else {
                &config.value_style
            };
            let percent = value.map_or_else(|| "--".into(), |v| format!("{v}%"));
            let label = format!(" {icon} {label}   {percent:>4}");
            let mut spans = vec![Span {
                text: label.clone(),
                style: style(color)?,
            }];
            if let Some(value) = value {
                let available = width.saturating_sub(UnicodeWidthStr::width(label.as_str()) + 2);
                let length = 8.min(available);
                let filled = (usize::from(value) * length).div_ceil(13 * 8).min(length);
                spans.push(Span {
                    text: format!("  {}", "▰".repeat(filled)),
                    style: style(color)?,
                });
                spans.push(Span {
                    text: "▱".repeat(length - filled),
                    style: style(&config.track_style)?,
                });
            }
            rows.push(Row {
                divider: false,
                identity: crate::actions::Identity::new("metric").field("metric", metric),
                spans,
                fill: None,
                range: None,
                focus: false,
                selected: false,
            });
        }
        if let Some(age) = stale {
            rows[0].spans.push(Span {
                text: format!(" · {age}s old"),
                style: style(&config.track_style)?,
            });
        }
        Ok(rows)
    }

    fn render_debug(&self, previous: Option<&crate::Diagnostics>) -> Result<Vec<Row>, String> {
        let style = resolve_style(&self.config.debug.style, "default", &self.palette)?;
        let mut lines = Vec::new();
        match previous {
            Some(previous) => {
                lines.push((
                    crate::actions::Identity::new("total"),
                    format!(" last {}", debug_duration(previous.total())),
                ));
                if self.config.debug.details {
                    for (name, micros) in crate::debug::STAGES.iter().zip(previous.stages) {
                        if micros > 0 {
                            lines.push((
                                crate::actions::Identity::new("stage").field("stage", name),
                                format!("  {name} {}", debug_duration(micros)),
                            ));
                        }
                    }
                }
            }
            None => lines.push((crate::actions::Identity::new("total"), " last --".into())),
        }
        Ok(lines
            .into_iter()
            .map(|(identity, text)| Row {
                divider: false,
                identity,
                spans: vec![Span {
                    text,
                    style: style.clone(),
                }],
                ..Row::blank()
            })
            .collect())
    }

    fn render_sessions(&self, snapshot: &Snapshot) -> Result<Vec<Row>, String> {
        let mut rows = Vec::new();
        for session in &snapshot.sessions {
            let current = session.id == snapshot.current_session;
            if self.sessions.config.active_only && !current {
                continue;
            }
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
                divider: false,
                identity: crate::actions::Identity::new("session")
                    .field("target_session", &session.id)
                    .field("name", &session.name),
                spans: render_format(
                    &self.sessions.session_format,
                    &values,
                    &style,
                    &self.palette,
                )?,
                fill: Some(resolve_color(fill, &self.palette)?),
                range: Some(Range::Session(crate::navigation::session_token(
                    &session.id,
                )?)),
                focus: current && !self.show_windows(),
                selected: false,
            });

            if !self.show_windows() || (self.sessions.config.fold_inactive && !current) {
                continue;
            }
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
                    divider: false,
                    identity: crate::actions::Identity::new("window")
                        .field("target_session", &session.id)
                        .field("target_window", &window.id)
                        .field("index", window.index)
                        .field("name", &window.name),
                    spans: render_format(
                        &self.sessions.window_format,
                        &values,
                        &style,
                        &self.palette,
                    )?,
                    fill: Some(resolve_color(fill, &self.palette)?),
                    range: Some(Range::Window(crate::navigation::token(
                        &session.id,
                        &window.id,
                    )?)),
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

    fn render_pi_workbench(&self, sessions: &[crate::PiSession]) -> Result<Vec<Row>, String> {
        let mut ordered = sessions.to_vec();
        crate::pi_workbench::sort_sessions(&mut ordered);
        let mut labels: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for session in &ordered {
            labels
                .entry(crate::pi_workbench::project_label(&session.project))
                .or_default()
                .insert(&session.project);
        }
        let mut rows = Vec::new();
        let mut previous = None;
        for session in &ordered {
            if previous != Some(session.project.as_str()) {
                let mut title = crate::pi_workbench::project_label(&session.project).to_owned();
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
                    divider: false,
                    identity: crate::actions::Identity::new("project")
                        .field("project", &session.project),
                    spans: vec![Span {
                        text: format!(" {title}"),
                        style: resolve_style(
                            &self.config.pi_workbench.project_style,
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
            "notify" => &self.config.pi_workbench.notify_style,
            "working" => &self.config.pi_workbench.working_style,
            _ => &self.config.pi_workbench.idle_style,
        };
        let icon_style = resolve_style(icon_style, "default", &self.palette)?;
        let selected_fill = resolve_color(&self.config.pi_workbench.selected_fill, &self.palette)?;
        let icon_style = if session.selected {
            format!("default,{icon_style},bg={selected_fill}")
        } else {
            icon_style
        };
        let row_style = if session.selected {
            format!(
                "default,{}",
                resolve_style(
                    &self.config.pi_workbench.selected_style,
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
            divider: false,
            identity: {
                let mut identity = crate::actions::Identity::new("session")
                    .field("project", &session.project)
                    .field("name", &session.name);
                if let Some(target) = &session.target {
                    identity = identity
                        .field("target_pane", &target.pane)
                        .field("target_window", &target.window);
                }
                identity
            },
            spans: render_format(
                &self.pi_workbench_format,
                &values,
                &row_style,
                &self.palette,
            )?,
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

    #[expect(
        clippy::too_many_lines,
        reason = "Usage rows share formatting and width decisions across providers"
    )]
    fn render_usage(
        &self,
        usage_rows: &[crate::usage::UsageRow],
        width: usize,
    ) -> Result<Vec<Row>, String> {
        let mut rows = Vec::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let columns = (self.config.usage.format == DEFAULT_USAGE_FORMAT)
            .then(|| {
                let values: Vec<_> = self
                    .config
                    .usage
                    .providers
                    .iter()
                    .filter_map(|provider| usage_rows.iter().find(|row| &row.provider == provider))
                    .flat_map(|row| row.windows.iter())
                    .filter(|window| {
                        window.used_percent.is_finite()
                            && (0.0..=100.0).contains(&window.used_percent)
                    })
                    .map(|window| usage_values(window, now.as_secs(), "default"))
                    .collect();
                (values.len() > 1).then(|| UsageColumns::from_values(&values, width))
            })
            .flatten();
        for provider in &self.config.usage.providers {
            let Some(usage) = usage_rows.iter().find(|row| &row.provider == provider) else {
                continue;
            };
            let heading_style = if usage.unavailable {
                &self.config.usage.unavailable_style
            } else if usage.stale || usage.refresh_failure.is_some() {
                &self.config.usage.stale_style
            } else {
                &self.config.usage.provider_style
            };
            let failure = usage.refresh_failure.map(|kind| match kind {
                crate::usage::RefreshFailure::SignInAgain => "sign in again",
                crate::usage::RefreshFailure::RefreshFailed => "refresh failed",
            });
            let age = if usage.stale || failure.is_some() {
                usage.fetched_at.map(|fetched_at| {
                    format!(
                        "{} old",
                        format_usage_age((now.as_millis() as u64).saturating_sub(fetched_at))
                    )
                })
            } else {
                None
            };
            let suffix = match (failure, age.as_deref()) {
                (Some(failure), _) => format!(" ({failure})"),
                (None, Some(age)) => format!(" ({age})"),
                (None, None) if usage.unavailable => " (unavailable)".to_owned(),
                (None, None) => String::new(),
            };
            let heading_style =
                usage_normal_style(&resolve_style(heading_style, "default", &self.palette)?);
            let mut spans = vec![Span {
                text: format!(" {}{suffix}", usage.display_name),
                style: heading_style,
            }];
            if provider == "codex" && !usage.unavailable && failure.is_none() {
                if let Some(count) = usage.available_resets {
                    let noun = if count == 1 { "reset" } else { "resets" };
                    spans.push(Span {
                        text: format!(" · {count} {noun}"),
                        style: usage_normal_style(&resolve_style(
                            &self.config.usage.stale_style,
                            "default",
                            &self.palette,
                        )?),
                    });
                }
            }
            let page_range =
                crate::usage::page_token(provider).map(|token| Range::UsagePage(token.into()));
            rows.push(Row {
                divider: false,
                identity: crate::actions::Identity::new("provider").field("provider", provider),
                spans,
                fill: None,
                range: page_range.clone(),
                focus: false,
                selected: false,
            });
            if failure.is_some() {
                if let Some(age) = age {
                    rows.push(Row {
                        divider: false,
                        identity: crate::actions::Identity::new("cache-age")
                            .field("provider", provider),
                        spans: vec![Span {
                            text: format!("  cached {age}"),
                            style: usage_normal_style(&resolve_style(
                                &self.config.usage.stale_style,
                                "default",
                                &self.palette,
                            )?),
                        }],
                        fill: None,
                        range: page_range.clone(),
                        focus: false,
                        selected: false,
                    });
                }
            }
            let window_style = if usage.stale {
                &self.config.usage.stale_style
            } else {
                &self.config.usage.window_style
            };
            let window_style = resolve_style(window_style, "default", &self.palette)?;
            let window_style = usage_normal_style(&window_style);
            for window in &usage.windows {
                if !window.used_percent.is_finite() || !(0.0..=100.0).contains(&window.used_percent)
                {
                    continue;
                }
                let bar_style = if window.used_percent >= 80.0 {
                    &self.config.usage.critical_bar_style
                } else if quota_behind_time(window, now.as_secs()) {
                    &self.config.usage.warning_bar_style
                } else {
                    &window_style
                };
                let bar_style = resolve_style(bar_style, &window_style, &self.palette)?;
                let mut values = usage_values(window, now.as_secs(), &bar_style);
                if let Some(columns) = &columns {
                    columns.align(&mut values, &window_style);
                }
                let track = resolve_color(&self.config.usage.bar_track_color, &self.palette)?;
                shade_usage_bar(&mut values, &bar_style, &track, &window_style);
                rows.push(Row {
                    divider: false,
                    identity: {
                        let mut identity = crate::actions::Identity::new("window")
                            .field("provider", provider)
                            .field("label", &window.label);
                        if let Some(duration) = window.duration_seconds {
                            identity = identity.field("duration_seconds", duration);
                        }
                        identity
                    },
                    spans: render_format(
                        &self.usage_format,
                        &values,
                        &window_style,
                        &self.palette,
                    )?,
                    fill: None,
                    range: page_range.clone(),
                    focus: false,
                    selected: false,
                });
            }
        }
        Ok(rows)
    }

    fn render_command(&self, name: &str, text: &str) -> Result<Row, String> {
        let config = &self.config.commands[name.strip_prefix("command.").unwrap()];
        let style = resolve_style(&config.style, "default", &self.palette)?;
        let mut spans = Vec::new();
        if !config.prefix.is_empty() {
            spans.push(Span {
                text: config.prefix.clone(),
                style: style.clone(),
            });
        }
        if config.output == "tmux-styles" {
            spans.extend(command_spans(text, &style, &self.palette));
        } else {
            spans.push(Span {
                text: text.into(),
                style,
            });
        }
        Ok(Row {
            divider: false,
            identity: crate::actions::Identity::new("output"),
            spans,
            fill: None,
            range: None,
            focus: false,
            selected: false,
        })
    }

    fn render_git(&self, status: &crate::GitStatus) -> Result<Vec<Row>, String> {
        let config = &self.config.git;
        let mut values = BTreeMap::new();
        let fields = [
            ("branch", status.branch.clone(), &config.branch_style),
            ("upstream", status.upstream.clone(), &config.upstream_style),
            ("ahead", count("↑", status.ahead), &config.divergence_style),
            (
                "behind",
                count("↓", status.behind),
                &config.divergence_style,
            ),
            (
                "divergence",
                format!("{}{}", count("↓", status.behind), count("↑", status.ahead)),
                &config.divergence_style,
            ),
            ("staged", count("●", status.staged), &config.staged_style),
            (
                "modified",
                count("✚", status.modified),
                &config.modified_style,
            ),
            (
                "untracked",
                count("…", status.untracked),
                &config.untracked_style,
            ),
            (
                "conflicts",
                count("✖", status.conflicts),
                &config.conflicts_style,
            ),
            ("stash", count("⚑", status.stash), &config.stash_style),
            ("added", count("+", status.added), &config.added_style),
            ("deleted", count("-", status.deleted), &config.deleted_style),
            (
                "clean",
                if status.clean() {
                    "✔".into()
                } else {
                    String::new()
                },
                &config.clean_style,
            ),
            ("state", status.state.clone(), &config.state_style),
        ];
        for (key, text, style) in fields {
            values.insert(
                key,
                Value::Spans(vec![Span {
                    text,
                    style: resolve_style(style, "default", &self.palette)?,
                }]),
            );
        }
        let mut rows = Vec::new();
        for (slot, line) in self.git_lines.iter().enumerate() {
            let (spans, present) = render_nodes(line, &values, "default", &self.palette, None)?;
            if present {
                rows.push(Row {
                    divider: false,
                    identity: crate::actions::Identity::new("line").field("slot", slot + 1),
                    spans: coalesce(spans),
                    fill: None,
                    range: None,
                    focus: false,
                    selected: false,
                });
            }
        }
        Ok(rows)
    }

    fn render_gob(&self, jobs: &[crate::gob::GobJob], width: usize) -> Result<Vec<Row>, String> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let heading = resolve_style(&self.config.gob.heading_style, "default", &self.palette)?;
        let green = resolve_style(&self.config.gob.running_style, "default", &self.palette)?;
        let progress = resolve_style(&self.config.gob.progress_style, "default", &self.palette)?;
        let track = resolve_color(&self.config.gob.bar_track_color, &self.palette)?;
        let mut rows = vec![Row {
            divider: false,
            identity: crate::actions::Identity::new("heading"),
            spans: vec![Span {
                text: " Jobs".into(),
                style: heading,
            }],
            fill: None,
            range: None,
            focus: false,
            selected: false,
        }];
        let mut ordered = jobs.to_vec();
        ordered.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        for job in ordered {
            let values = BTreeMap::from([
                ("id", Value::Text(job.id.clone())),
                ("name", Value::Text(job.name.clone())),
                (
                    "state",
                    Value::Spans(vec![Span {
                        text: "●".into(),
                        style: green.clone(),
                    }]),
                ),
            ]);
            rows.push(Row {
                divider: false,
                identity: crate::actions::Identity::new("job")
                    .field("job_id", &job.id)
                    .field("name", &job.name)
                    .field("part", "name"),
                spans: render_format(&self.gob_format, &values, "default", &self.palette)?,
                fill: None,
                range: None,
                focus: false,
                selected: false,
            });
            if let Some(percent) = job.percent(time::OffsetDateTime::now_utc()) {
                let label = format!("{:>4}", format!("{percent:.0}%"));
                let right_pad = if width >= label.len() + 2 { 2 } else { 0 };
                let gap = usize::from(width > label.len() + right_pad + 1);
                let indent = 4.min(width.saturating_sub(label.len() + right_pad + gap + 1));
                let length = width.saturating_sub(indent + gap + label.len() + right_pad);
                let filled = ((percent / 100.0 * length as f64).round() as usize).min(length);
                let mut bar = BTreeMap::from([(
                    "bar",
                    Value::Spans(vec![Span {
                        text: format!("{}{}", "█".repeat(filled), "░".repeat(length - filled)),
                        style: progress.clone(),
                    }]),
                )]);
                shade_usage_bar(&mut bar, &progress, &track, "default");
                let mut spans = vec![Span {
                    text: " ".repeat(indent),
                    style: "default".into(),
                }];
                if let Some(Value::Spans(glyphs)) = bar.remove("bar") {
                    spans.extend(glyphs);
                }
                spans.push(Span {
                    text: format!("{}{label}{}", " ".repeat(gap), " ".repeat(right_pad)),
                    style: "default".into(),
                });
                rows.push(Row {
                    divider: false,
                    identity: crate::actions::Identity::new("job")
                        .field("job_id", &job.id)
                        .field("name", &job.name)
                        .field("part", "progress"),
                    spans: coalesce(spans),
                    fill: None,
                    range: None,
                    focus: false,
                    selected: false,
                });
            }
        }
        Ok(rows)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Context rows share icon, style, and click range assembly"
    )]
    fn render_pi_context(
        &self,
        context: &crate::PiContext,
        selected_name: Option<&str>,
        states: &[crate::pr_state::PrState],
        width: usize,
        pane: &str,
    ) -> Result<Vec<Row>, String> {
        if context.plans.is_empty() && context.pull_requests.is_empty() && context.skills.is_empty()
        {
            return Ok(Vec::new());
        }
        let cfg = &self.config.pi_context;
        let mut rows = Vec::new();
        let mut push = |text: String,
                        style: &str,
                        icon: Option<(&str, &str)>,
                        range: Option<Range>,
                        identity: crate::actions::Identity|
         -> Result<(), String> {
            let mut spans = Vec::new();
            if let Some((glyph, icon_style)) = icon {
                spans.push(Span {
                    text: "  ".into(),
                    style: "default".into(),
                });
                spans.push(Span {
                    text: glyph.into(),
                    style: resolve_style(icon_style, "default", &self.palette)?,
                });
                spans.push(Span {
                    text: " ".into(),
                    style: "default".into(),
                });
            }
            spans.push(Span {
                text,
                style: resolve_style(style, "default", &self.palette)?,
            });
            rows.push(Row {
                divider: false,
                identity,
                spans,
                fill: None,
                range,
                focus: false,
                selected: false,
            });
            Ok(())
        };
        let heading = selected_name.map_or_else(|| " π".to_owned(), |name| format!(" π {name}"));
        push(
            heading,
            &cfg.heading_style,
            None,
            None,
            crate::actions::Identity::new("heading"),
        )?;
        if !context.plans.is_empty() {
            push(
                " Plans".into(),
                &cfg.category_style,
                None,
                None,
                crate::actions::Identity::new("category").field("category", "plans"),
            )?;
            for (index, plan) in context.plans.iter().enumerate() {
                let range = plan
                    .path
                    .is_file()
                    .then(|| crate::navigation::file_token("sl", pane, &plan.path, index))
                    .transpose()?
                    .map(Range::File);
                push(
                    plan.title.clone(),
                    &cfg.text_style,
                    Some(("◇", &cfg.plan_style)),
                    range,
                    {
                        let identity =
                            crate::actions::Identity::new("plan").field("title", &plan.title);
                        if plan.path.is_file() {
                            identity.field("file", plan.path.display())
                        } else {
                            identity
                        }
                    },
                )?;
            }
        }
        if !context.pull_requests.is_empty() {
            push(
                " PRs".into(),
                &cfg.category_style,
                None,
                None,
                crate::actions::Identity::new("category").field("category", "prs"),
            )?;
            for (index, url) in context.pull_requests.iter().enumerate() {
                let Some((label, _)) = crate::pr_state::parse_url(url) else {
                    continue;
                };
                let (glyph, style) = match states
                    .get(index)
                    .copied()
                    .unwrap_or(crate::pr_state::PrState::Unknown)
                {
                    crate::pr_state::PrState::Open => ("\u{ea64}", &cfg.open_style),
                    crate::pr_state::PrState::Draft => ("\u{ebdb}", &cfg.draft_style),
                    crate::pr_state::PrState::Merged => ("\u{eafe}", &cfg.merged_style),
                    crate::pr_state::PrState::Closed => ("\u{ebda}", &cfg.closed_style),
                    crate::pr_state::PrState::Unknown => ("\u{ea64}", &cfg.unknown_style),
                };
                let state_word = match states
                    .get(index)
                    .copied()
                    .unwrap_or(crate::pr_state::PrState::Unknown)
                {
                    crate::pr_state::PrState::Draft => " draft",
                    crate::pr_state::PrState::Closed => " closed",
                    _ => "",
                };
                let available = width.saturating_sub(4);
                let label = if label.len() + state_word.len() <= available {
                    format!("{label}{state_word}")
                } else if label.len() <= available {
                    label
                } else if let Some((repo, number)) = label.split_once('#') {
                    let suffix = format!("#{number}");
                    let prefix = available.saturating_sub(suffix.len() + 1);
                    if prefix == 0 {
                        suffix
                    } else {
                        format!("{}…{suffix}", &repo[..prefix.min(repo.len())])
                    }
                } else {
                    label
                };
                let token = crate::navigation::pr_token(pane, url, index)?;
                push(
                    label,
                    &cfg.text_style,
                    Some((glyph, style)),
                    Some(Range::PullRequest(token)),
                    crate::actions::Identity::new("pr").field("url", url),
                )?;
            }
        }
        if !context.skills.is_empty() {
            push(
                " Skills".into(),
                &cfg.category_style,
                None,
                None,
                crate::actions::Identity::new("category").field("category", "skills"),
            )?;
            for (index, skill) in context.skills.iter().enumerate() {
                let range = skill
                    .path
                    .as_ref()
                    .filter(|path| path.is_file())
                    .map(|path| crate::navigation::file_token("ss", pane, path, index))
                    .transpose()?
                    .map(Range::File);
                push(
                    skill.name.clone(),
                    &cfg.text_style,
                    Some(("✦", &cfg.skill_style)),
                    range,
                    {
                        let mut identity =
                            crate::actions::Identity::new("skill").field("name", &skill.name);
                        if let Some(path) = skill.path.as_ref().filter(|path| path.is_file()) {
                            identity = identity.field("file", path.display());
                        }
                        identity
                    },
                )?;
            }
        }
        Ok(rows)
    }

    fn render_divider(&self, width: usize) -> Result<Row, String> {
        let glyph = self.config.divider.character.as_str();
        let glyph_width = UnicodeWidthStr::width(glyph);
        let count = width.saturating_sub(2) / glyph_width;
        Ok(Row {
            divider: true,
            identity: crate::actions::Identity::new("divider"),
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
    if !(2..=MAX_WIDTH).contains(&snapshot.width) {
        return Err(format!("width must be 2..{MAX_WIDTH}"));
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

fn valid_command_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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

fn format_usage_age(age_ms: u64) -> String {
    let minutes = age_ms / 60_000;
    if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 1440 {
        let hours = minutes / 60;
        let rest = minutes % 60;
        if rest == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h{rest}m")
        }
    } else {
        let days = minutes / 1440;
        let hours = minutes % 1440 / 60;
        if hours == 0 {
            format!("{days}d")
        } else {
            format!("{days}d{hours}h")
        }
    }
}

fn quota_behind_time(window: &crate::usage::UsageWindow, now: u64) -> bool {
    let (Some(duration), Some(reset)) = (window.duration_seconds, window.reset_at) else {
        return false;
    };
    if duration <= 86_400 || reset <= now {
        return false;
    }
    let total_days = duration.div_ceil(86_400);
    let time_left = reset.saturating_sub(now).min(duration);
    let elapsed_days = (duration - time_left) / 86_400;
    let current_day = (elapsed_days + 1).min(total_days);
    window.used_percent > current_day as f64 / total_days as f64 * 100.0
}

fn usage_normal_style(style: &str) -> String {
    if style.split(',').any(|part| part.starts_with("bg=")) {
        style.to_owned()
    } else if style == "default" {
        "default".into()
    } else {
        format!("{style},bg=default")
    }
}

fn shade_usage_bar(values: &mut BTreeMap<&str, Value>, color: &str, track: &str, normal: &str) {
    let Some(Value::Spans(spans)) = values.get_mut("bar") else {
        return;
    };
    let Some(first) = spans.first() else {
        return;
    };
    let glyphs = first.text.clone();
    let foreground = color
        .split(',')
        .filter(|part| !part.starts_with("bg="))
        .collect::<Vec<_>>()
        .join(",");
    let used_style = if foreground.is_empty() {
        format!("bg={track}")
    } else {
        format!("{foreground},bg={track}")
    };
    let empty_style = format!("bg={track}");
    let mut shaded = Vec::new();
    for glyph in glyphs.chars() {
        let (text, style) = match glyph {
            ' ' => (" ".to_owned(), normal.to_owned()),
            '░' => (" ".to_owned(), empty_style.clone()),
            _ => (glyph.to_string(), used_style.clone()),
        };
        shaded.push(Span { text, style });
    }
    shaded.extend(spans.drain(1..));
    *spans = shaded;
}

fn usage_field_text<'a>(value: &'a BTreeMap<&str, Value>, key: &str) -> &'a str {
    match value.get(key) {
        Some(Value::Text(text)) => text,
        Some(Value::Spans(spans)) => spans.first().map_or("", |span| span.text.as_str()),
        None => "",
    }
}

struct UsageColumns {
    name: usize,
    remaining: usize,
    bar: usize,
    percent: usize,
    compact_bar: bool,
}

impl UsageColumns {
    fn from_values(values: &[BTreeMap<&str, Value>], width: usize) -> Self {
        let maximum = |key| {
            values
                .iter()
                .map(|value| UnicodeWidthStr::width(usage_field_text(value, key)))
                .max()
                .unwrap_or(0)
        };
        let mut columns = Self {
            name: maximum("name").min(width.saturating_sub(16).max(2)).max(2),
            remaining: maximum("remaining").min(7),
            bar: maximum("bar"),
            percent: maximum("percent"),
            compact_bar: false,
        };
        if columns.total() > width {
            columns.compact_bar = true;
            columns.bar = values
                .iter()
                .map(|value| {
                    UnicodeWidthStr::width(usage_field_text(value, "bar").replace(' ', "").as_str())
                })
                .max()
                .unwrap_or(0);
        }
        while columns.total() > width && columns.name > 2 {
            columns.name -= 1;
        }
        while columns.total() > width && columns.remaining > 2 {
            columns.remaining -= 1;
        }
        while columns.total() > width && columns.bar > 1 {
            columns.bar -= 1;
        }
        while columns.total() > width && columns.remaining > 0 {
            columns.remaining -= 1;
        }
        while columns.total() > width && columns.name > 1 {
            columns.name -= 1;
        }
        columns
    }

    fn total(&self) -> usize {
        2 + self.name
            + 1
            + self.bar
            + 1
            + self.percent
            + if self.remaining > 0 {
                1 + self.remaining
            } else {
                0
            }
    }

    fn align(&self, values: &mut BTreeMap<&str, Value>, window_style: &str) {
        let format_cell = |value: &mut Value, width: usize, right: bool| {
            let Value::Text(text) = value else {
                return;
            };
            let mut remaining = width;
            let clipped = clipped(text, &mut remaining);
            *text = if right {
                format!("{}{}", " ".repeat(remaining), clipped)
            } else {
                format!("{}{}", clipped, " ".repeat(remaining))
            };
        };
        format_cell(values.get_mut("name").unwrap(), self.name, false);
        if self.remaining > 0 {
            format_cell(values.get_mut("remaining").unwrap(), self.remaining, true);
        } else {
            values.insert("remaining", Value::Text(String::new()));
        }
        format_cell(values.get_mut("percent").unwrap(), self.percent, true);
        if let Some(Value::Spans(spans)) = values.get_mut("bar") {
            let text = &mut spans[0].text;
            if self.compact_bar {
                text.retain(|character| character != ' ');
            }
            let mut remaining = self.bar;
            *text = clipped(text, &mut remaining);
            if remaining > 0 {
                spans.push(Span {
                    text: " ".repeat(remaining),
                    style: window_style.to_owned(),
                });
            }
        }
    }
}

fn usage_values(
    window: &crate::usage::UsageWindow,
    now: u64,
    bar_style: &str,
) -> BTreeMap<&'static str, Value> {
    let duration = window.duration_seconds;
    let name = match duration {
        Some(seconds) if seconds > 86_400 => {
            let days = seconds as f64 / 86_400.0;
            if seconds % 86_400 == 0 {
                format!("{}d", seconds / 86_400)
            } else {
                format!("{days:.1}d")
            }
        }
        _ => window.label.clone(),
    };
    let remaining = window.reset_at.map_or_else(String::new, |reset| {
        let minutes = reset.saturating_sub(now) / 60;
        if reset <= now {
            "now".into()
        } else if minutes < 60 {
            format!("{minutes}m")
        } else if minutes < 1440 {
            format!(
                "{}h{}",
                minutes / 60,
                if minutes % 60 == 0 {
                    String::new()
                } else {
                    format!("{}m", minutes % 60)
                }
            )
        } else {
            format!(
                "{}d{}",
                minutes / 1440,
                if minutes % 1440 < 60 {
                    String::new()
                } else {
                    format!("{}h", minutes % 1440 / 60)
                }
            )
        }
    });
    let bar = match duration {
        Some(seconds) if seconds > 86_400 && seconds <= 7 * 86_400 => {
            let count = seconds.div_ceil(86_400) as usize;
            progress_blocks(window.used_percent, count).join(" ")
        }
        Some(seconds) if seconds <= 86_400 => progress_blocks(window.used_percent, 5).join(" "),
        _ => progress_blocks(window.used_percent, 10).concat(),
    };
    BTreeMap::from([
        ("name", Value::Text(name)),
        ("remaining", Value::Text(remaining)),
        (
            "bar",
            Value::Spans(vec![Span {
                text: bar,
                style: bar_style.to_owned(),
            }]),
        ),
        (
            "percent",
            Value::Text(format!("{:.0}%", window.used_percent)),
        ),
    ])
}

fn progress_blocks(percent: f64, count: usize) -> Vec<&'static str> {
    let filled = percent.clamp(0.0, 100.0) / 100.0 * count as f64;
    const PARTIAL: [&str; 9] = ["░", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    (0..count)
        .map(|index| {
            let eighths = ((filled - index as f64).clamp(0.0, 1.0) * 8.0).round() as usize;
            PARTIAL[eighths]
        })
        .collect()
}

#[derive(Clone, Debug)]
enum Value {
    Text(String),
    Spans(Vec<Span>),
}

fn count(prefix: &str, value: u64) -> String {
    if value == 0 {
        String::new()
    } else {
        format!("{prefix}{value}")
    }
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
    divider: bool,
    identity: crate::actions::Identity,
    spans: Vec<Span>,
    fill: Option<String>,
    range: Option<Range>,
    focus: bool,
    selected: bool,
}

impl Row {
    fn blank() -> Self {
        Self {
            identity: crate::actions::Identity::new("blank"),
            divider: false,
            spans: Vec::new(),
            fill: None,
            range: None,
            focus: false,
            selected: false,
        }
    }
}

fn layout_rows(mut rows: Vec<Row>, spacer: Option<usize>, height: usize) -> Vec<Row> {
    let spacer_end = spacer.map(|index| {
        let padding = height.saturating_sub(rows.len());
        rows.splice(index..index, (0..padding).map(|_| Row::blank()));
        index + padding
    });
    let mut previous_divider = false;
    let mut row_index = 0;
    let mut spacer_end_after_collapse = 0;
    rows.retain(|row| {
        let keep = !row.divider || !previous_divider;
        previous_divider = row.divider;
        if keep && spacer_end.is_some_and(|end| row_index < end) {
            spacer_end_after_collapse += 1;
        }
        row_index += 1;
        keep
    });
    if spacer_end.is_some() {
        let padding = height.saturating_sub(rows.len());
        rows.splice(
            spacer_end_after_collapse..spacer_end_after_collapse,
            (0..padding).map(|_| Row::blank()),
        );
    }
    rows
}

#[derive(Clone, Debug)]
enum Range {
    Session(String),
    Window(String),
    PiPane(String),
    UsagePage(String),
    PullRequest(String),
    File(String),
    Action(String),
}

fn command_spans(text: &str, base: &str, palette: &BTreeMap<String, String>) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut literal = String::new();
    let mut style = base.to_owned();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with("##") {
            literal.push_str("##");
            rest = &rest[2..];
            continue;
        }
        if let Some(tail) = rest.strip_prefix("#[") {
            if let Some(end) = tail.find(']') {
                let directive = &tail[..end];
                let next = if directive == "none" {
                    Some(base.to_owned())
                } else if directive.contains('#') || directive.contains('[') {
                    None
                } else {
                    resolve_style(directive, base, palette).ok()
                };
                if let Some(next) = next {
                    if !literal.is_empty() {
                        spans.push(Span {
                            text: std::mem::take(&mut literal),
                            style,
                        });
                    }
                    style = next;
                    rest = &tail[end + 1..];
                    continue;
                }
            }
        }
        let character = rest.chars().next().unwrap();
        literal.push(character);
        rest = &rest[character.len_utf8()..];
    }
    if !literal.is_empty() {
        spans.push(Span {
            text: literal,
            style,
        });
    }
    coalesce(spans)
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

fn debug_duration(micros: u64) -> String {
    if micros < 1000 {
        format!("{:.3} ms", micros as f64 / 1000.0)
    } else {
        format!("{:.2} ms", micros as f64 / 1000.0)
    }
}

fn render_rows(rows: &[Row], width: usize, text_color: Option<&str>) -> String {
    let mut result = String::from(
        "#[list=on]#[list=left-marker]#[acs]-#[noacs]#[nl]#[list=right-marker]#[acs].#[noacs]#[nl]",
    );
    for (index, row) in rows.iter().enumerate() {
        let mut remaining = width;
        if row.spans.is_empty() && row.range.is_none() && row.fill.is_none() {
            result.push_str("#[default]");
        }
        let token = match &row.range {
            Some(
                Range::Session(token)
                | Range::Window(token)
                | Range::PiPane(token)
                | Range::UsagePage(token)
                | Range::PullRequest(token)
                | Range::File(token)
                | Range::Action(token),
            ) => token.as_str(),
            None => "sv",
        };
        result.push_str(&format!(
            "#[range=user|{token} {}]",
            if row.focus { "list=focus " } else { "" }
        ));
        for span in &row.spans {
            let text = clipped(&span.text, &mut remaining);
            if text.is_empty() {
                continue;
            }
            if span.style == "default" {
                if let Some(color) = text_color {
                    result.push_str(&format!("#[default,fg={color}]"));
                } else {
                    result.push_str("#[default]");
                }
            } else {
                result.push_str(&format!("#[{}]", span.style));
            }
            result.push_str(&escaped(&text));
        }
        if let Some(fill) = &row.fill {
            result.push_str(&format!("#[bg={fill}]"));
        }
        let padding = if width <= 2 && row.spans.is_empty() {
            remaining
        } else {
            remaining.saturating_sub(1)
        };
        result.push_str(&" ".repeat(padding));
        if row.selected {
            result.push_str("#[norange]#[list=on default]");
        } else {
            result.push_str("#[norange default]");
        }
        if let Some(fill) = &row.fill {
            result.push_str(&format!("#[fill={fill}]"));
        }
        result.push_str("#[nl]");
        if row.fill.is_some() && index + 1 < rows.len() {
            result.push_str("#[fill=default]");
        }
    }
    result
}
