# Configuration

Starmux reads `$XDG_CONFIG_HOME/starmux.toml`, falling back to `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses portable defaults.

Run `starmux print-config` to print the complete configuration. Run `starmux check-config` to validate module names, formats, styles, palettes, and tmux option aliases.

## Modules

`modules` is the ordered list of sidebar modules:

```toml
modules = ["sessions", "divider"]
```

The built-ins are `sessions` and `divider`. Unknown and duplicate names are errors.

## Row formats

Session and window rows use a safe formatter rather than raw tmux syntax:

- `$variable` inserts escaped module data.
- `(text $variable)` is included only when one of its variables is non-empty.
- `[text $variable](fg=accent,bg=surface,bold)` applies a validated style.
- `\` escapes the next formatter character.

Available session variables are `$id` and `$name`. Window rows also provide `$index` and `$indicator`. The special `$style` value is valid only inside a styled group's style expression, where it selects the semantic style for that session or window state.

```toml
[sessions]
session_format = "[ $name ]($style)"
window_format = "[ $index: $name]($style)"
```

The styles accepted by Starmux are tmux color assignments (`fg=COLOR`, `bg=COLOR`) and supported text attributes such as `bold`, `dim`, `reverse`, and their `no…` forms. Data variables cannot emit tmux styles or click ranges.

## Palettes

Styles may refer to colors in the selected palette:

```toml
palette = "mine"

[palettes.mine]
text = "#c0caf5"
accent = "#bb9af7"
surface = "#24283b"

[sessions]
current_session_style = "fg=surface,bg=accent,bold"
other_session_style = "fg=text,bold"
```

Without a selected palette, styles use terminal colors directly.

## Window indicators

The default sessions module does not request or display custom tmux options. A configuration can give user options safe aliases and derive one indicator from them:

```toml
[sessions]
window_format = "[ $index ]($style)$indicator[ $name]($style)"

[sessions.window_options]
icon = "@project_icon"
state = "@agent_state"

[sessions.indicator]
source = "icon"
fallback = "|"
style = "$style"

[[sessions.indicator.rules]]
when = { state = "working" }
text = "*"
style = "fg=yellow"

[[sessions.indicator.rules]]
when = { state = "notify" }
text = "!"
style = "fg=magenta"
```

Rules are checked in order. The first rule whose conditions all match wins. If no rule matches, Starmux uses the source option and then the fallback. Option names must start with `@`; option values are always escaped as data.

## Divider

The divider uses one visible grapheme and fills the available row width:

```toml
[divider]
character = "-"
style = "dim"
```

Set `disabled = true` in either `[sessions]` or `[divider]` to omit that module without changing the shared module order.

## Tmux integration

`starmux init tmux` generates the side-status render command and the `MouseDown1Status` binding used for foreign-window navigation. Position, width, and outer style remain ordinary tmux options.

The adapter passes `#{side-status-width}` as the explicit `--width` argument. Valid widths are 1–300 columns. Other status clicks retain tmux's default action.
