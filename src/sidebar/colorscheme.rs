use std::collections::BTreeMap;

const STYLES: &str = r#"
[top]
heading_style = "fg=accent,bold"
value_style = "fg=text"
warning_style = "fg=orange"
critical_style = "fg=danger"
track_style = "fg=muted"

[sessions]
current_session_style = "fg=background,bg=accent,bold"
other_session_style = "fg=text,bg=highlight,bold"
active_window_style = "fg=accent,bg=border,bold,nounderscore,noitalics"
selected_window_style = "fg=muted,bg=surface,bold,nounderscore,noitalics"
other_window_style = "fg=muted,bg=surface,nobold,nounderscore,noitalics"
current_session_fill = "accent"
other_session_fill = "highlight"
active_window_fill = "border"
selected_window_fill = "background"
other_window_fill = "background"

[divider]
style = "fg=border,nobold"

[pi-workbench]
heading_style = "fg=accent,bold"
project_style = "fg=text,bold"
idle_style = "fg=muted"
working_style = "fg=warning"
notify_style = "fg=accent"
selected_style = "fg=accent,bg=border,bold"
selected_fill = "border"

[pi-context]
heading_style = "fg=accent,bold"
category_style = "fg=muted"
text_style = "fg=text"
plan_style = "fg=accent"
skill_style = "fg=warning"
open_style = "fg=green"
draft_style = "fg=muted"
merged_style = "fg=accent"
closed_style = "fg=danger"
unknown_style = "fg=muted"
build_success_style = "fg=green"
build_failure_style = "fg=danger"
build_pending_style = "fg=warning"

[usage]
provider_style = "fg=accent,bold"
window_style = "fg=text"
warning_bar_style = "fg=orange"
critical_bar_style = "fg=danger"
bar_track_color = "border"
stale_style = "fg=muted"
unavailable_style = "fg=muted"

[gob]
heading_style = "fg=text,bold"
running_style = "fg=green"
progress_style = "fg=green"
overdue_style = "fg=warning"
label_style = "fg=muted"
bar_track_color = "border"

[git]
branch_style = "fg=text,bold"
upstream_style = "fg=accent"
divergence_style = "fg=warning"
staged_style = "fg=green,bold"
modified_style = "fg=danger,bold"
untracked_style = "fg=accent,bold"
conflicts_style = "fg=danger,bold"
stash_style = "fg=accent"
added_style = "fg=green"
deleted_style = "fg=danger"
clean_style = "fg=green,bold"
state_style = "fg=danger,bold"

[debug]
style = "fg=muted"
"#;

const TOKYO_NIGHT: &str = r##"
background = "#1b1d2b"
surface = "#1e2030"
highlight = "#292e42"
border = "#3b4261"
text = "#82aaff"
muted = "#828bb8"
accent = "#c099ff"
warning = "#ffc777"
green = "#c3e88d"
orange = "#ff9e64"
danger = "#f7768e"
"##;

const CATPPUCCIN_MOCHA: &str = r##"
background = "#1e1e2e"
surface = "#313244"
highlight = "#45475a"
border = "#585b70"
text = "#cdd6f4"
muted = "#a6adc8"
accent = "#cba6f7"
warning = "#f9e2af"
green = "#a6e3a1"
orange = "#fab387"
danger = "#f38ba8"
"##;

const GITHUB_DARK: &str = r##"
background = "#0d1117"
surface = "#161b22"
highlight = "#21262d"
border = "#30363d"
text = "#c9d1d9"
muted = "#8b949e"
accent = "#a371f7"
warning = "#d29922"
green = "#3fb950"
orange = "#db6d28"
danger = "#f85149"
"##;

// Role assignments use colors from the upstream palettes linked in docs/configuration.md.
const GRUVBOX_DARK: &str = r##"
background = "#282828"
surface = "#3c3836"
highlight = "#504945"
border = "#665c54"
text = "#ebdbb2"
muted = "#928374"
accent = "#d3869b"
warning = "#fabd2f"
green = "#b8bb26"
orange = "#fe8019"
danger = "#fb4934"
"##;

const NORD: &str = r##"
background = "#2e3440"
surface = "#3b4252"
highlight = "#434c5e"
border = "#4c566a"
text = "#d8dee9"
muted = "#81a1c1"
accent = "#b48ead"
warning = "#ebcb8b"
green = "#a3be8c"
orange = "#d08770"
danger = "#bf616a"
"##;

const DRACULA: &str = r##"
background = "#282a36"
surface = "#44475a"
highlight = "#44475a"
border = "#6272a4"
text = "#f8f8f2"
muted = "#6272a4"
accent = "#bd93f9"
warning = "#f1fa8c"
green = "#50fa7b"
orange = "#ffb86c"
danger = "#ff5555"
"##;

const SOLARIZED_DARK: &str = r##"
background = "#002b36"
surface = "#073642"
highlight = "#073642"
border = "#586e75"
text = "#839496"
muted = "#586e75"
accent = "#6c71c4"
warning = "#b58900"
green = "#859900"
orange = "#cb4b16"
danger = "#dc322f"
"##;

const ONE_DARK: &str = r##"
background = "#282c34"
surface = "#2c323c"
highlight = "#3e4452"
border = "#4b5263"
text = "#abb2bf"
muted = "#5c6370"
accent = "#c678dd"
warning = "#e5c07b"
green = "#98c379"
orange = "#d19a66"
danger = "#e06c75"
"##;

const ROSE_PINE_MOON: &str = r##"
background = "#232136"
surface = "#2a273f"
highlight = "#393552"
border = "#56526e"
text = "#e0def4"
muted = "#6e6a86"
accent = "#c4a7e7"
warning = "#f6c177"
green = "#9ccfd8"
orange = "#ea9a97"
danger = "#eb6f92"
"##;

const KANAGAWA_WAVE: &str = r##"
background = "#1f1f28"
surface = "#2a2a37"
highlight = "#363646"
border = "#54546d"
text = "#dcd7ba"
muted = "#727169"
accent = "#957fb8"
warning = "#e6c384"
green = "#98bb6c"
orange = "#ffa066"
danger = "#e46876"
"##;

const ROLES: [&str; 11] = [
    "background",
    "surface",
    "highlight",
    "border",
    "text",
    "muted",
    "accent",
    "warning",
    "green",
    "orange",
    "danger",
];

pub(super) fn is_builtin(name: &str) -> bool {
    built_in(name).is_some()
}

pub(super) fn validate_colors(name: &str, colors: &BTreeMap<String, String>) -> Result<(), String> {
    for role in ROLES {
        let color = colors
            .get(role)
            .ok_or_else(|| format!("colorscheme {name} missing color {role}"))?;
        super::validate_color(color, &BTreeMap::new())
            .map_err(|error| format!("colorscheme {name} {role}: {error}"))?;
    }
    for role in colors.keys() {
        if !ROLES.contains(&role.as_str()) {
            return Err(format!("colorscheme {name} unknown color {role}"));
        }
    }
    Ok(())
}

fn built_in(name: &str) -> Option<&'static str> {
    match name {
        "tokyo-night" => Some(TOKYO_NIGHT),
        "catppuccin-mocha" => Some(CATPPUCCIN_MOCHA),
        "github-dark" => Some(GITHUB_DARK),
        "gruvbox-dark" => Some(GRUVBOX_DARK),
        "nord" => Some(NORD),
        "dracula" => Some(DRACULA),
        "solarized-dark" => Some(SOLARIZED_DARK),
        "one-dark" => Some(ONE_DARK),
        "rose-pine-moon" => Some(ROSE_PINE_MOON),
        "kanagawa-wave" => Some(KANAGAWA_WAVE),
        _ => None,
    }
}

pub(super) fn load(
    name: &str,
    custom: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(toml::Value, BTreeMap<String, String>), String> {
    let colors = if let Some(colors) = built_in(name) {
        toml::from_str(colors).map_err(|error| error.to_string())?
    } else {
        custom
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown colorscheme {name}"))?
    };
    validate_colors(name, &colors)?;
    let styles = toml::from_str(STYLES).map_err(|error| error.to_string())?;
    Ok((styles, colors))
}
