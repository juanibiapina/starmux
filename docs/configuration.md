# Configuration

Starmux reads `$XDG_CONFIG_HOME/starmux.toml`, falling back to `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses portable defaults.

Run `starmux print-config` to print the complete configuration. Run `starmux check-config` to validate module names, formats, styles, palettes, and tmux option aliases.

## Modules

`modules` is the ordered list of sidebar modules:

```toml
modules = ["sessions", "divider"]
```

The built-ins are `sessions`, `divider`, `pi-workbench`, `pi-context`, `usage`, `gob`, `git`, `debug`, `spacer`, and `blank`. Named external commands use `command.<name>`. Unknown names and duplicate names other than `divider` and `blank` are errors. Repeat `divider` to separate multiple sections, for example `modules = ["sessions", "divider", "pi-workbench", "divider", "usage", "divider", "gob"]`. Each divider uses the same `[divider]` settings. `pi-workbench`, `pi-context`, `usage`, `gob`, `git`, and `debug` are not enabled by default.

Place one `spacer` between modules to push the following rows to the bottom of the sidebar. For example, `modules = ["sessions", "spacer", "divider", "usage"]` keeps sessions at the top and usage at the bottom. The spacer takes only the rows left after all other modules render. It adds no rows if the sidebar is full, and it has no style or click target. Its size updates when the client is resized. Add `blank` after `usage` to leave one empty row below it: `modules = ["sessions", "spacer", "divider", "usage", "blank"]`. Each `blank` entry adds one empty row without a style or click target; repeat it for more space.

### Multiple module lists

Define additional ordered lists in the same file with `[configs.NAME]`. The top-level `modules` list is named `default` and is selected when `--config` is absent:

```toml
modules = ["sessions", "divider", "pi-workbench"]

[configs.right]
modules = ["usage", "spacer", "debug"]

[configs.bottom]
modules = ["git", "command.build"]

[commands.build]
argv = ["my-status", "--short"]
```

Run `starmux render-query --config=right --width=30 --socket=PATH --client=NAME` to select the right list. `explain` and `timings` accept the same selector. All lists use the same module settings, colorscheme, palette, and named commands. A command may be referenced only by a named list. Empty lists produce no content rows. Names use 1–32 ASCII letters, digits, `_`, or `-`; `default` is reserved. Unknown names and invalid lists are errors. `check-config` validates every list; `print-config` prints all of them.

`starmux init tmux` selects `default`. For another tmux render area, copy its generated render invocation and add `--config=right` after `render-query`. Pass the same `--config=right` to custom `starmux activate`, `starmux scroll`, and `starmux scroll-event` bindings for that area. Scroll positions are separate for each list on a client. Tmux controls the placement and width of each area; Starmux renders its selected rows within the supplied width and available height.

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

Select a built-in colorscheme to color the sidebar modules without setting individual styles:

```toml
colorscheme = "tokyo-night"
```

Bundled schemes (colors are mapped from the linked upstream palettes into Starmux's eleven roles):

| Name | Palette source |
| --- | --- |
| `tokyo-night` | [Tokyo Night](https://github.com/folke/tokyonight.nvim) (uses the existing Starmux dotfiles colors) |
| `catppuccin-mocha` | [Catppuccin](https://github.com/catppuccin/palette) |
| `github-dark` | [GitHub Primer](https://github.com/primer/primitives) |
| `gruvbox-dark` | [Gruvbox](https://github.com/morhetz/gruvbox/blob/master/colors/gruvbox.vim) |
| `nord` | [Nord](https://github.com/nordtheme/nord/blob/develop/src/nord.css) |
| `dracula` | [Dracula](https://github.com/dracula/dracula-theme#color-palette) |
| `solarized-dark` | [Solarized](https://github.com/altercation/solarized) |
| `one-dark` | [One Dark](https://github.com/joshdick/onedark.vim/blob/main/autoload/onedark.vim) |
| `rose-pine-moon` | [Rosé Pine](https://github.com/rose-pine/rose-pine-palette/blob/main/dist/css/rose-pine.css) |
| `kanagawa-wave` | [Kanagawa](https://github.com/rebelot/kanagawa.nvim/blob/master/lua/kanagawa/colors.lua) |

Define your own scheme with the same eleven color roles:

```toml
colorscheme = "my-dark"

[colorschemes.my-dark]
background = "#10151c"
surface = "#18212b"
highlight = "#273442"
border = "#405064"
text = "#d9e2ec"
muted = "#91a2b3"
accent = "#a8a0ff"
warning = "#ffd580"
green = "#8fd6a8"
orange = "#ffab70"
danger = "#ff808c"
```

Each role is required. Colors accept the same validated tmux color names, `colour0`–`colour255`, and `#RRGGBB` values as palette colors. Custom names cannot replace bundled names. `check-config` rejects missing or unknown roles and invalid colors.

Each scheme supplies default styles and fills for sessions, dividers, Pi Workbench, Pi context, git, usage, gob, debug, and text-mode named commands. The scheme colors ordinary text in these modules too. `spacer` and `blank` contain no styled content. A scheme does not set tmux's outer `side-status-style`, change tmux's other status options, or recolor styles supplied by an external command in `tmux-styles` mode.

Explicit module styles and fills take priority over scheme defaults. A selected custom `palette` overlays colors with the same names in the scheme, so a module style such as `fg=accent` uses your palette's `accent`. Formats and indicator rules remain under your control; explicit colors in a format or rule are not replaced. Unknown scheme names and invalid colors or styles fail `check-config`. Without a colorscheme, styles use terminal colors directly unless you select a custom palette as above.

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

The divider uses one visible grapheme and fills the available row width. When empty modules leave dividers next to each other, they appear as one line. A single divider remains visible even when nearby modules are empty. An explicit `blank` row separates dividers; a `spacer` separates them when it has room to add rows.

```toml
[divider]
character = "-"
style = "dim"
```

Set `disabled = true` in `[sessions]`, `[divider]`, `[pi-workbench]`, `[pi-context]`, `[usage]`, `[gob]`, `[git]`, or `[debug]` to omit that module without changing the shared module order.

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

Available providers are `anthropic`, `copilot`, `gemini`, `antigravity`, `codex`, `kiro`, `zai`, and `xai`. By default, none are selected. Set `providers` explicitly to start fetching usage. Providers appear in the listed order. Each provider gets a heading and one row per available usage window. When no cached usage exists, its heading shows `(unavailable)`. The default window row shows its duration, time until reset, progress bar, and used percentage. When multiple windows are visible, these fields align across providers. In a narrow sidebar, the bars lose their spaces first so the percentage remains visible. Custom `format` values render as written. For windows lasting more than 24 hours and at most seven days, `$name` shows the duration in days and `$bar` has one block per day. The blocks divide the total window usage into visual steps; they do not represent usage on individual calendar days. Partial blocks fill from the bottom over the same background track as empty blocks. Only the bar turns red at 80% used. For multiday plans, it turns orange when usage exceeds the allocation through the current day (one day's share per day). Red takes priority. The day comparison requires both a known window duration and a future reset time. Set `critical_bar_style`, `warning_bar_style`, and `bar_track_color` to change the colors. Shorter windows use five progress blocks. Windows without a known duration use their provider label and a ten-cell bar. `$remaining` shows the time until reset when the provider supplies a reset timestamp; otherwise the optional group omits it. Window formats support `$name`, `$remaining`, `$percent`, and `$bar`, plus optional and styled groups. Cached usage older than 30 minutes shows its age, such as `(31m old)`, and uses `stale_style`. More recent cached usage keeps the normal colors and format, including during a failed refresh or retry delay. An unavailable provider is marked `(unavailable)` and uses `unavailable_style`. Usage text is escaped and clipped. Click a heading or plan row to open the provider's usage page for Claude, Codex, GitHub Copilot premium requests, or the z.ai personal Coding Plan. Gemini CLI, Antigravity, Kiro, and Grok rows have no browser click target because a direct page showing the fetched usage has not been established. The z.ai link opens the personal plan page; team plans have a separate page.

For GPT subscription usage alone, set `providers = ["codex"]`. When Codex reports its available reset count, the heading shows a muted label, such as `Codex Plan · 1 reset` or `Codex Plan · 0 resets`. A missing count leaves the heading unchanged. Starmux reads Pi's `~/.pi/agent/auth.json` or Codex's auth file for Codex credentials; it does not require Pi or pi-usage to run. A foreground sidebar render returns cached usage immediately and starts a refresh worker when data is absent or at least 60 seconds old. One Starmux worker per provider holds the lease, so concurrent clients do not multiply requests. Failed requests preserve last-good usage and delay retries using the provider's `Retry-After` header or a 60-second fallback. The next tmux redraw shows a completed refresh. Starmux's cache and locks are separate from pi-usage; if both run, each may make a request within the same minute.

Set `cache_dir` to an absolute path under `[usage]` to change the usage cache location. The default is `~/Library/Caches/starmux/usage` on macOS or `${XDG_CACHE_HOME:-~/.cache}/starmux/usage` elsewhere. Styles accept palette colors and the same validated attributes as other modules.

## Live Pi sessions

Add `pi-workbench` to `modules` to show a state icon and the name of each reachable Pi session published by pi-workbench:

```toml
modules = ["sessions", "divider", "pi-workbench"]

[pi-workbench]
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

The `pi-tmux` namespace identifies the tmux server and pane. Sessions with a different tmux server socket do not appear in this sidebar. A row with a matching pane on that server is clickable. Starmux resolves the pane's current window on click and selects the pane. A record without a matching pane remains visible without a click target or selected styling. With a custom tmux mouse binding, route `sp` user ranges to `starmux activate` as well as `sw` ranges; `starmux init tmux` emits the required binding.

Starmux reads version 2 JSON records from `~/.local/share/pi/status`. It gets the endpoint from `pi-socket` and checks each session's Unix socket with the socket protocol's version 1 ping request. An absent directory, invalid records, unsupported versions, and unreachable sessions produce no rows for those records. A bad record does not hide healthy sessions. Starmux scans at most 256 directory entries and checks at most 32 records matching the current tmux socket per render, with a 200 ms total query budget and a 40 ms ping timeout per session. Set `data_dir` to an absolute path under `[pi-workbench]` when the extension publishes elsewhere; Starmux looks for `status/` and `sockets/` in that directory. Reading the files does not remove stale records. If your config uses `pi-live`, rename both the module list entry and `[pi-live]` table to `pi-workbench` and `[pi-workbench]`.

## Selected Pi context

Add `pi-context` to show the plans, pull requests, and loaded skills recorded by pi-workbench for the Pi session in the selected tmux pane. It can be placed independently of `pi-workbench`:

```toml
modules = ["sessions", "divider", "pi-workbench", "divider", "pi-context"]

[pi-context]
heading_style = "fg=magenta,bold"
category_style = "dim"
text_style = "default"
plan_style = "fg=magenta"
skill_style = "fg=magenta"
open_style = "fg=green"
draft_style = "fg=brightblack"
merged_style = "fg=magenta"
closed_style = "fg=red"
unknown_style = "fg=brightblack"
```

When context has entries, a `π` heading shows the selected Pi session's name above the nonempty Plans, PRs, and Skills sections. This heading appears even when `pi-context` is used without `pi-workbench`. Category labels align with the heading; item rows have one indent. `heading_style` controls the heading color and weight. `◇` marks a plan and `✦` marks a loaded skill. A PR row shows `owner/repo#number` with Nerd Font Codicon icons: green `` for open, muted `` for draft, purple `` for merged, and red `` for closed. Unknown state uses a muted ``. Draft and closed rows include a word when space permits. Click a PR row to open its GitHub page. Click a plan to open its Markdown file or a skill to open its `SKILL.md` with the system's file handler. Set `open_command` under `[pi-context]` to override that handler:

```toml
open_command = ["dev", "tmux", "edit", "{file}", "{pane}", "{socket}"]
```

Starmux passes each array item as a separate process argument without a shell. `{file}` is required exactly once; `{pane}` is the selected tmux pane ID and `{socket}` is the current tmux server socket. This example opens the file in that pane's tmux session's Neovim editor window. The command runs only for plan and skill rows. Clicks resolve the selected Pi session again and reject stale or missing targets. The `pi-skills` namespace records paths for loaded skills, including cached GitHub skills. A skill without a recorded path uses `~/.agents/skills/<name>/SKILL.md` or `~/.pi/agent/skills/<name>/SKILL.md` when present. Text is escaped and clipped to the sidebar width. Styles accept palette colors. Use `disabled = true` to hide the module.

The module reads the selected reachable session's version 2 namespaced context file through its Pi status record. Switching panes updates the rows. No selected Pi session, an empty context, a missing file, or an invalid file produces no context rows. The Pi status scan is shared with `pi-workbench` when both modules are enabled. It shows at most 16 entries in each category.

PR states come from authenticated `gh api` requests in a background worker. A sidebar render uses the cached result immediately and refreshes eligible PRs after five minutes. At most 16 PRs can start a refresh per render, matching the displayed PR limit. Concurrent clients share a lease per PR. Failed lookups retry after one minute, preserve the last known state for up to 30 minutes, then show a muted ``. A missing `gh` command or GitHub authentication leaves the association visible with unknown state. The cache lives under `${XDG_CACHE_HOME:-~/.cache}/starmux/pr-state`.

## Gob jobs

Add `gob` after a divider to show running jobs for the selected tmux pane's current directory:

```toml
modules = ["sessions", "divider", "gob"]

[gob]
format = "  $state $name"
heading_style = "bold"
running_style = "fg=green"
progress_style = "fg=green"
bar_track_color = "colour238"
```

The section shows `Jobs`, then a green dot and description for each running job. If a job has no description, `$name` shows its command or job ID. The format also accepts `$id`. A job with a previous successful-run average gets a second row with a horizontal progress track below its name. The track fills the space after its indentation and before a four-column percentage field that fits `100%`, with two more spaces to its right; filled cells show elapsed time divided by the previous average, capped at 100%. A job can still be running when the track is full. When history or a valid start time is absent, there is no progress row. Narrow widths reduce the indentation to keep a track cell when both the track and percentage fit; the percentage takes priority at smaller widths. Rows do not have click targets. All job text is escaped and clipped.

Starmux runs `gob list --json` from the selected pane's `pane_current_path`. Gob matches the workdir exactly: a job started in a nested directory appears only when the pane is in that directory. Switching panes can change the list. If gob is missing or the directory has no running jobs, the section has no rows. Gob starts its daemon when `list` runs and the daemon is absent. Starmux bounds the command to 1.5 seconds and 2 MiB of output. If the query fails or returns invalid data, the Gob rows are omitted for that redraw; `starmux explain` reports the error while the other sidebar modules remain visible.

## External commands

Place up to four named commands anywhere in `modules`. Each `command.<name>` entry needs a matching `[commands.<name>]` table. Names contain only ASCII letters, digits, `_`, or `-` and are at most 32 characters long. For example:

```toml
modules = ["sessions", "divider", "command.gitmux", "divider", "command.build"]

[commands.gitmux]
argv = ["gitmux", "-cfg", "~/.gitmux.conf"]
output = "tmux-styles"
prefix = " "

[commands.build]
argv = ["my-status", "--short"]
style = "fg=green"
```

Starmux runs each argument array directly, without a shell, from the selected pane's current directory. A leading `~/` in an argument expands to the user's `HOME` directory; other shell expansions are not performed. Gitmux uses the pane directory by default. Commands run in module order on each sidebar redraw. Each command has a 500 ms deadline; four commands can take up to two seconds if all stall. Stdout is limited to 16 KiB and only its first line is displayed as one row when nonempty. A missing executable, invalid directory, empty output, failure, or timeout adds no row. `starmux explain` reports command failures; `starmux timings` includes `command_us`. `starmux check-config` validates names, references, arguments, modes, and styles. A command may have at most 16 arguments, each at most 1024 bytes.

`prefix` adds literal text before the output; it defaults to empty and accepts up to 64 bytes without control characters. The default `output = "text"` escapes command output and applies `style` (default `default`). Set `output = "tmux-styles"` for gitmux's colored output. This mode accepts only `#[none]` and validated tmux text styles such as `#[fg=green,bold]`. All other directives and tmux formats remain literal text. Command rows have no click target. Output is clipped to the sidebar width.

## Git status

Add `git` to `modules` to show the repository containing the selected pane's directory:

```toml
modules = ["sessions", "divider", "git"]

[git]
lines = [
  " $branch( $upstream)( $divergence)",
  " ( $conflicts)( $staged)( $modified)( $untracked)( $stash)( $added)( $deleted)( $clean)",
  " ( $state)",
]
```

Each string in `lines` is one sidebar row. An entry with no nonempty variables produces no row. Remove `$upstream` to hide the upstream name, or move it to its own string to give it a separate row. The safe formatter supports optional groups and validated styled groups. Variables are `$branch`, `$upstream`, `$ahead`, `$behind`, `$divergence`, `$staged`, `$modified`, `$untracked`, `$conflicts`, `$stash`, `$added`, `$deleted`, `$clean`, and `$state`. Zero counts are empty. Branch names are escaped and clipped. The rows have no click targets. Set `disabled = true` in `[git]` to omit the section.

`$staged`, `$modified`, `$untracked`, and `$conflicts` count files; one file can be both staged and modified. `$added` and `$deleted` count tracked line changes against HEAD across staged and unstaged edits. Untracked files and binary changes have no line totals. `$clean` shows `✔` when no files have changed and no Git operation is active; stashes do not make the worktree dirty. `$stash` counts repository stashes, shared across linked worktrees. `$upstream` and `$divergence` use local refs without fetching. `$state` shows an active merge, rebase, cherry-pick, or revert. Detached HEAD shows a short commit ID. Outside a repository or when Git is missing, the section has no rows. Queries have a 1.5 second deadline and a 2 MiB output limit; failures omit the section and appear in `starmux explain`.

The `[git]` styles are `branch_style`, `upstream_style`, `divergence_style`, `staged_style`, `modified_style`, `untracked_style`, `conflicts_style`, `stash_style`, `added_style`, `deleted_style`, `clean_style`, and `state_style`. Each accepts validated tmux styles and palette colors. The default colors make additions green and deletions red.

## Debug timings

Add `debug` to show how long the previous completed redraw took for this tmux socket and client. Place it after `spacer` to keep it at the bottom:

```toml
modules = ["sessions", "spacer", "debug"]

[debug]
details = true
style = "dim"
```

The default shows only `last 12.34 ms`; `details = true` adds rows for nonzero `tmux`, `pi`, `pr`, `usage`, `gob`, `commands`, `git`, and `format` stages. These are source query and formatting times: several sidebar modules share a source, and `commands` combines configured external commands. The total is the sum of the stages, subject to rounding when displayed. The interval begins before the tmux snapshot and ends after formatting the sidebar. It excludes process startup, reading and writing the debug cache, and tmux's evaluation of the resulting status text. Background refresh workers are not included; starting them during a redraw is included. `starmux timings` retains its JSON fields but runs its own query without starting refresh workers.

The first redraw shows `last --`. Each completed redraw records its measurement for the next one; the value expires after five minutes. Cache errors also show `last --` and do not interrupt the sidebar. The default cache is `${XDG_CACHE_HOME:-~/.cache}/starmux/debug`; set an absolute `cache_dir` under `[debug]` to change it. Rows have no click targets and use the configured validated style. `disabled = true` omits the rows and cache access. The default module list does not include `debug`.

## Tmux integration

`starmux init tmux` generates the side-status render command, click binding, and wheel bindings. Position, width, and outer style remain ordinary tmux options.

The adapter passes `#{side-status-width}` as the explicit `--width` argument. Valid widths are 1–300 columns. If the client switches sessions or windows during a redraw, Starmux discards that stale render; the next redraw shows the new focus. Other status clicks retain tmux's default action.

With `mouse on`, wheel up and down over the sidebar scroll the complete list one row at a time, including rows from every configured module. The selected tmux window stays unchanged; click a visible row to activate it. Each attached client keeps its own position. Scrolling stops at the first and last page and adjusts to changes in content or client height. Wheel events over the ordinary horizontal status keep tmux's window selection behavior. Regenerate and source `starmux init tmux` after upgrading to install the wheel bindings.
