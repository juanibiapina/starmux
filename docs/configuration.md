# Configuration

Starmux reads `$XDG_CONFIG_HOME/starmux.toml`, falling back to `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses portable defaults.

Run `starmux print-config` to print the complete configuration. Run `starmux check-config` to validate module names, formats, styles, palettes, and tmux option aliases.

## Modules

`modules` is the ordered list of sidebar modules:

```toml
modules = ["sessions", "divider"]
```

The built-ins are `sessions`, `divider`, `pi-live`, and `usage`. Unknown names and duplicate names other than `divider` are errors. Repeat `divider` to separate multiple sections, for example `modules = ["sessions", "divider", "pi-live", "divider", "usage"]`. Each divider uses the same `[divider]` settings. `pi-live` and `usage` are not enabled by default.

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

Set `disabled = true` in `[sessions]`, `[divider]`, `[pi-live]`, or `[usage]` to omit that module without changing the shared module order.

## Provider usage

Add `usage` to `modules` to show the current usage windows for selected providers:

```toml
modules = ["sessions", "divider", "usage"]

[usage]
providers = ["anthropic", "codex", "gemini"]
format = "  $name( $remaining) $bar $percent"
provider_style = "bold"
window_style = "default"
warning_bar_style = "fg=colour208"
critical_bar_style = "fg=red"
bar_track_color = "colour238"
stale_style = "dim"
unavailable_style = "dim"
```

Available providers are `anthropic`, `copilot`, `gemini`, `antigravity`, `codex`, `kiro`, `zai`, and `xai`. By default, none are selected. Set `providers` explicitly to start fetching usage. Providers appear in the listed order. Each provider gets a heading and one row per available usage window. When no cached usage exists, its heading shows `(unavailable)`. The default window row shows its duration, time until reset, progress bar, and used percentage. When multiple windows are visible, these fields align across providers. In a narrow sidebar, the bars lose their spaces first so the percentage remains visible. Custom `format` values render as written. For windows lasting more than 24 hours and at most seven days, `$name` shows the duration in days and `$bar` has one block per day. The blocks divide the total window usage into visual steps; they do not represent usage on individual calendar days. Partial blocks fill from the bottom over the same background track as empty blocks. Only the bar turns red at 80% used. For multiday plans, it turns orange when usage exceeds the allocation through the current day (one day's share per day). Red takes priority. The day comparison requires both a known window duration and a future reset time. Set `critical_bar_style`, `warning_bar_style`, and `bar_track_color` to change the colors. Shorter windows use five progress blocks. Windows without a known duration use their provider label and a ten-cell bar. `$remaining` shows the time until reset when the provider supplies a reset timestamp; otherwise the optional group omits it. Window formats support `$name`, `$remaining`, `$percent`, and `$bar`, plus optional and styled groups. Cached usage older than 30 minutes shows its age, such as `(31m old)`, and uses `stale_style`. More recent cached usage keeps the normal colors and format, including during a failed refresh or retry delay. An unavailable provider is marked `(unavailable)` and uses `unavailable_style`. Usage text is escaped and clipped; these rows have no click targets.

For GPT subscription usage alone, set `providers = ["codex"]`. Starmux reads Pi's `~/.pi/agent/auth.json` or Codex's auth file for Codex credentials; it does not require Pi or pi-usage to run. A foreground sidebar render returns cached usage immediately and starts a refresh worker when data is absent or at least 60 seconds old. One Starmux worker per provider holds the lease, so concurrent clients do not multiply requests. Failed requests preserve last-good usage and delay retries using the provider's `Retry-After` header or a 60-second fallback. The next tmux redraw shows a completed refresh. Starmux's cache and locks are separate from pi-usage; if both run, each may make a request within the same minute.

Set `cache_dir` to an absolute path under `[usage]` to change the usage cache location. The default is `~/Library/Caches/starmux/usage` on macOS or `${XDG_CACHE_HOME:-~/.cache}/starmux/usage` elsewhere. Styles accept palette colors and the same validated attributes as other modules.

## Live Pi sessions

Add `pi-live` to `modules` to show a state icon and the name of each reachable Pi session published by pi-live:

```toml
modules = ["sessions", "divider", "pi-live"]

[pi-live]
format = "  $state $name"
project_style = "bold"
idle_style = "fg=brightblack"
working_style = "fg=yellow"
notify_style = "fg=magenta"
selected_style = "reverse,bold"
selected_fill = "default"
```

Pi sessions are grouped by project. Starmux finds the nearest Git root above each published cwd; when there is no Git root, the cwd is the project. Each project gets one heading styled by `project_style`. Groups sort by their highest priority session: attention, working, selected idle, then idle. Sessions within a group use the same priority order, then name. A project heading adds one sidebar row.

The row format supports `$state` (a `●` icon) and `$name`, optional groups, and validated styled groups. The three state styles color the icon; the name uses normal text styling unless its pane is selected. A Pi pane marked `@pi_state=notify` shows the magenta icon. The selected Pi pane uses `selected_style` and `selected_fill`. When a session has no name, `$name` shows the first eight characters of its session ID. Styles can use colors from the selected palette. Names and project headings are escaped as text and clipped to the sidebar width.

Only Pi sessions published with the current tmux server's socket path appear in its sidebar. Sessions outside tmux and older records without the server socket path are hidden until Pi restarts with an updated pi-live publisher. A row with a matching pane on that server is clickable. Starmux resolves the pane's current window on click and selects the pane. A record without a matching pane remains visible without a click target or selected styling. With a custom tmux mouse binding, route `sp` user ranges to `starmux activate` as well as `sw` ranges; `starmux init tmux` emits the required binding.

Starmux reads version 1 JSON records from `~/.local/share/pi/status` and checks each session's published Unix socket with pi-live's version 1 ping request. An absent directory, invalid records, unsupported versions, and unreachable sessions produce no rows for those records. A bad record does not hide healthy sessions. Starmux scans at most 256 directory entries and checks at most 32 records matching the current tmux socket per render, with a 200 ms total query budget and a 40 ms ping timeout per session. Set `data_dir` to an absolute path under `[pi-live]` when pi-live publishes elsewhere; Starmux looks for `status/` and `sockets/` in that directory. Reading the files does not remove stale records.

## Tmux integration

`starmux init tmux` generates the side-status render command and the `MouseDown1Status` binding used for foreign-window navigation. Position, width, and outer style remain ordinary tmux options.

The adapter passes `#{side-status-width}` as the explicit `--width` argument. Valid widths are 1–300 columns. Other status clicks retain tmux's default action.
