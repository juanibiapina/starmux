# Configuration

This file documents user-facing configuration.

Starmux reads `$XDG_CONFIG_HOME/starmux.toml`, falling back to `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses portable defaults.

Run `starmux print-config` to print the complete configuration. Run `starmux check-config` to validate module names, formats, styles, palettes, and tmux option aliases.

## Modules

`modules` is the ordered list of sidebar modules:

```toml
modules = ["sessions", "divider"]
```

The built-ins are `top`, `sessions`, `divider`, `pi-workbench`, `pi-context`, `usage`, `gob`, `git`, `debug`, `spacer`, and `blank`. Named external commands use `command.<name>`. Unknown names and duplicate names other than `divider` and `blank` are errors. Repeat `divider` to separate multiple sections, for example `modules = ["sessions", "divider", "pi-workbench", "divider", "usage", "divider", "gob"]`. Each divider uses the same `[divider]` settings. `top`, `pi-workbench`, `pi-context`, `usage`, `gob`, `git`, and `debug` are not enabled by default.

Place one `spacer` between modules to push the following rows to the bottom of the sidebar. For example, `modules = ["sessions", "spacer", "divider", "usage"]` keeps sessions at the top and usage at the bottom. The spacer takes only the rows left after all other modules render. It adds no rows if the sidebar is full, and it has no style or default click action. Its size updates when the client is resized. Add `blank` after `usage` to leave one empty row below it: `modules = ["sessions", "spacer", "divider", "usage", "blank"]`. Each `blank` entry adds one empty row without a style or default click action; repeat it for more space.

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

`starmux init tmux` selects `default`. For another tmux render area, copy its generated render invocation and add `--config=right` after `render-query`. Pass the same `--config=right` to custom `starmux activate`, `starmux scroll`, and `starmux scroll-event` bindings for that area. Tmux controls the placement and width of each area.

## Slim sidebar

Content width two automatically uses the slim layout. Content widths of three or more columns use the full layout. The minimum content width is two. Tmux reserves one sidebar column for its border: set `side-status-width` to 3 for two content columns or 31 for thirty.

```toml
[slim]
mode = "auto" # auto, full, or slim
show_windows = false
# Optional: omit to inherit the default modules list. [] hides all content.
modules = ["sessions", "divider", "pi-workbench", "spacer", "top"]

[sessions]
show_windows = true # Full-layout window visibility

[configs.right]
modules = ["usage", "spacer", "git"]
# Optional: omit to inherit this named list's modules.
slim_modules = ["usage", "spacer", "git"]
```

`mode = "full"` keeps full formats at every width. `mode = "slim"` uses at most two content columns at every width. Slim mode hides tmux windows by default; `[slim].show_windows = true` shows them. Full and slim window visibility are independent. The current session remains the focus row when its windows are hidden.

The default slim list inherits the top-level `modules`. Each named list inherits its own `modules` unless it defines `slim_modules`; it does not inherit `[slim].modules`. All lists share the same mode, window visibility, module settings, styles, colorscheme, and disabled flags. Only modules in the effective list are enabled. Commands referenced exclusively by a slim list are supported.

### Glyph legend

| Content | First cell | Second cell |
| --- | --- | --- |
| Tmux session | `󰆍` terminal icon | `●` current, `○` other session |
| Optional tmux window | `` window icon | `●` selected, `○` other window |
| Pi session | `󰂚` attention, `▶` working, `○` idle | `●` selected, `○` other pane |
| Pi plan | `◇` | One glyph only |
| Pi pull request | Existing open, draft, merged, or closed icon | `󰋗` when its state is unknown |
| Usage without valid windows | Provider icon | `󰌾` sign-in failure, `` refresh failure, `󰋗` unavailable, `◷` stale |
| Usage window | Provider icon; cached failures use `󰌾` or `` | Pie of used quota, such as `◑` for roughly 50% used |
| Host metric | `󰻠` CPU, `󰍛` memory, battery-state icon | Pie of CPU/memory used or battery charge remaining; `󰋗` unavailable |
| Running job | `` | `▶` running |
| Job progress | `↳` | Elapsed share of the typical duration; `█` in `overdue_style` once a job passes it |
| Git | `` conflicts, `↻` operation, `` dirty, `✓` clean, in priority order | `↕` diverged, `↑` ahead, `↓` behind, `·` neither |
| Named command | Configured `slim_icon`, default `` | One glyph only |
| Debug timing | `◷` previous timing available, `󰋗` unavailable | One glyph only |
| Divider | Configured glyph; `─` when it needs two columns | Repeat across the second column |

Provider icons are Anthropic `󰚩`, Codex ``, Copilot ``, Gemini `✦`, Antigravity `◎`, Kiro `󰊠`, z.ai `󰘦`, and xAI `󰖟`. The rail uses Nerd Font icons, as do the full-layout host and PR rows. Usage and host pies use `○ ◔ ◑ ◕ ●`, rounded to the nearest 25%. Usage pies show quota consumed; host pies show CPU/memory used and battery charge remaining. Job gauges use `░` for zero and `▁` through `█` for eight increasing bands. Missing data is not zero.

Slim mode omits usage provider headers when valid windows are available, names, initials, numeric indices, system/job headings, Pi project headings, Pi context headings, plan/PR category labels, usage cache-age rows and reset counts, and detailed debug rows. Plans and PRs keep individual click targets and the selectors listed in the [selector catalog](#selector-catalog). Skills and their category are omitted from slim mode. Click rules for skills and their category apply to full-layout rows. Job progress retains its separate `part = "progress"` row. Git uses one `kind = "summary"` row; full-layout `kind = "line"` and `slot` rules apply only to full rows. Hidden headings have no row for a heading click rule to match.

Full layout supplies names, individual skills, exact readings, counts, reset times, and timings. Debug markers convey availability only.

Named commands show an icon only for successful, nonempty output. Starmux does not infer status from arbitrary output text:

```toml
[commands.build]
argv = ["my-status", "--short"]
slim_icon = ""
```

`slim_icon` must be one visible, one-column grapheme without whitespace or control characters. Full formats remain full-layout settings; slim rows use the representations above.

Regenerate and source `starmux init tmux` to install bindings that pass the render area's width. Custom named-area bindings should pass the same `--config=NAME` and `--width=COLUMNS` to rendering, activation, and scrolling. If activation or scrolling omits `--width`, it uses the client's side-status content width.

## Click actions

Every row supports a configurable click action. This includes headings, dividers, blanks, and spacer padding. Existing navigation remains the default.

Define named commands under `[actions.NAME]`. Use ordered `[[clicks]]` rules to select rows:

```toml
[actions.monitor]
argv = ["tmux", "-S", "{socket}", "split-window", "-t", "{pane}", "-c", "{path}", "htop"]

[actions.jobs]
argv = ["tmux", "-S", "{socket}", "switch-client", "-c", "{client}", "-t", "{session}:4"]

[[clicks]]
module = "top"
match = { kind = "metric" }
action = "monitor"

[[clicks]]
module = "gob"
match = { kind = "job" }
action = "jobs"
```

Each metric click creates a new pane. Each job click selects window index 4 in the originating client's session. A missing window produces an error. Gob does not provide a job's tmux location.

### Rule order and defaults

The last matching rule wins. Put broad defaults before individual overrides. All fields in `match` use exact equality. Rules do not support expressions, patterns, or screen row numbers.

- Omit `module` for a default across all modules.
- Omit `match` for a default across all rows in that module.
- Set `config = "NAME"` to restrict a rule to one named module list.
- Set `instance = 2` to select the second occurrence of a module, such as a repeated divider.
- Set `action = "none"` to disable the row's click action.
- Set `action = "default"` to restore the row's built-in action.

`none` and `default` are reserved action names. An unmatched row retains its built-in action. Module settings and unscoped rules apply to all named lists.

### Individual rows

These overrides follow the defaults in the first example:

```toml
[actions.build]
argv = ["/absolute/path/to/open-build", "{job_id}", "{socket}", "{client}"]

[actions.usage-details]
argv = ["/absolute/path/to/show-usage", "{provider}", "{label}"]

[actions.edit]
argv = ["/absolute/path/to/edit-in-tmux", "{file}", "{pane}", "{socket}"]

[[clicks]]
module = "gob"
match = { kind = "job", name = "Build assets" }
action = "build"

[[clicks]]
module = "top"
match = { metric = "battery" }
action = "none"

[[clicks]]
module = "usage"
match = { kind = "window", provider = "anthropic", duration_seconds = 18000 }
action = "usage-details"

[[clicks]]
module = "pi-context"
match = { kind = "plan", file = "/absolute/path/to/plan.md" }
action = "edit"

[[clicks]]
module = "pi-context"
match = { kind = "skill", name = "documentation" }
action = "edit"
```

Gob `name` is the raw name before clipping: description, command fallback, or ID fallback. A name rule also applies to future jobs with that name. Use `job_id` for one particular job. Job name and progress rows share a rule unless `part` selects one of them.

Usage windows expose their raw provider labels and optional durations. The example selects a five-hour duration without assuming its label. File selectors use absolute paths. A skill can have no file path.

### Selector catalog

`kind`, `part`, and `occurrence` are available in every `match` table. Other selectors depend on the row kind:

| Module | `kind` | Additional selectors |
| --- | --- | --- |
| `top` | `heading`, `metric` | `metric`: `cpu`, `memory`, `battery` |
| `gob` | `heading`, `job` | `job_id`, `name` |
| `usage` | `provider`, `cache-age`, `window` | `provider`; windows also have `label`, optional `duration_seconds` |
| `pi-context` | `heading`, `category`, `plan`, `skill`, `pr`, `build` | Categories: `category` (`plans`, `prs`, `builds`, `skills`); plans: `file`, `title`; skills: `name`, optional `file`; PRs: `url`; builds: `repository`, `branch` |
| `sessions` | `session`, `window` | `target_session`, `name`; windows also have `target_window`, `index` |
| `pi-workbench` | `heading`, `project`, `session` | `project`; sessions also have `name`, optional `target_pane`, `target_window` |
| `git` | `line`, `summary` | Full-layout lines have `slot`: entry in the configured `git.lines` list; the slim summary has no slot |
| `command.NAME` | `output` | Select the command through its full module name |
| `debug` | `total`, `stage` | `stage`: `tmux`, `pi`, `pr`, `usage`, `gob`, `commands`, `git`, `format`, `top` |
| `divider` | `divider` | Select repeated dividers through `instance` |
| `blank` | `blank` | Select repeated blanks through `instance` |
| `spacer` | `padding` | `slot`: position within the spacer allocation |

`part` is `main` for ordinary rows. Gob uses `name` and `progress`. `occurrence` distinguishes otherwise identical rows in their original order before scrolling. `instance`, `slot`, and `occurrence` start at 1. Window `index` starts at 0. These values and `duration_seconds` are integers. Other selectors are strings.

Git `slot` includes configured lines that currently produce no output. Spacer slots follow the current allocation. They do not identify a screen row after scrolling.

### Command arguments and errors

Common placeholders are `{socket}`, `{client}`, `{session}`, `{window}`, `{pane}`, `{path}`, `{module}`, `{kind}`, and `{part}`. Focus placeholders refer to the originating client. `{path}` is its selected pane's directory.

Source placeholders use the selector names in the catalog, including `{occurrence}`. Foreign session, window, and pane rows expose their targets through `{target_session}`, `{target_window}`, and `{target_pane}`. Optional metadata is available only when the source provides it. File placeholders require an existing regular file.

Placeholders can occur inside an argument, such as `{session}:4`. Doubled braces produce literal braces: `{{literal}}` becomes `{literal}`. Inserted values remain literal and receive no further substitution.

If an action requires unavailable metadata, the row retains its built-in action. `starmux explain` reports the unavailable placeholder. An earlier rule does not replace the winning rule.

Starmux runs argument arrays directly in the originating pane's directory. It does not interpret a shell, tmux formats, environment variables, or `~`. Scripts support more complex commands. Interactive programs need a terminal, such as the pane in the monitor example.

Action definitions accept 1–16 arguments, each at most 1024 bytes, without NUL bytes. The executable must be a nonempty literal. Expanded arguments use the same limits. The configuration accepts at most 64 actions and 128 rules.

Commands have a two-second deadline and a 16 KiB stderr limit. A timed-out command is stopped; scripts must stop any processes they start. Missing executables, nonzero exits, and invalid directories return an activation error. Errors do not run a fallback action.

Regenerate and source `starmux init tmux` after installing this feature. Custom mouse and wheel bindings must route `sc` ranges with the existing Starmux ranges.

## Host status

Add `top` to a module list to show host CPU, memory, and battery status. Put it after `spacer` to keep it near the bottom:

```toml
modules = ["sessions", "spacer", "divider", "top", "divider", "git"]

[top]
metrics = ["cpu", "memory", "battery"]
```

The `metrics` list selects which host readings to show: `cpu` is whole-host CPU usage, `memory` is used physical memory, and `battery` is charge remaining when a battery is present. Set `disabled = true` to hide the section.

Use `heading_style`, `value_style`, `warning_style`, `critical_style`, and `track_style` in `[top]` to override colorscheme defaults. `heading_style` styles the `SYSTEM` heading; `value_style` styles normal readings; `track_style` styles the bar track. CPU and memory use `critical_style` at 80% or more; battery uses it at 20% or less. Rows have no click action by default. See [click actions](#click-actions) for overrides.

## Session visibility

Show only the active tmux session's windows while keeping every session heading:

```toml
[sessions]
fold_inactive = true
```

`fold_inactive` defaults to `false`. Switching tmux sessions expands the new active session and folds the previous one. Sidebar height and Pi content do not affect folding. The setting applies to all named lists and both layouts. Window visibility still follows `[sessions].show_windows` in full layout and `[slim].show_windows` in slim layout; when windows are disabled, only session headings appear. Folding reduces the row count; content that still exceeds the available height remains scrollable.

To hide inactive tmux sessions entirely, including their headings:

```toml
[sessions]
active_only = true
```

`active_only` defaults to `false` and applies to all named lists and both layouts. The active session follows tmux focus, independent of sidebar height and Pi content. Its windows follow the layout's window visibility setting. With `active_only = true`, `fold_inactive` has no additional effect because inactive sessions are hidden.

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

Bundled schemes and their source palettes:

| Name | Palette source |
| --- | --- |
| `tokyo-night` | [Tokyo Night](https://github.com/folke/tokyonight.nvim) |
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

Each scheme supplies default styles and fills for top, sessions, dividers, Pi Workbench, Pi context, git, usage, gob, debug, and text-mode named commands. The scheme colors ordinary text in these modules too. `spacer` and `blank` contain no styled content. A scheme does not set tmux's outer `side-status-style`, change tmux's other status options, or recolor styles supplied by an external command in `tmux-styles` mode.

Explicit module styles and fills take priority over scheme defaults. A selected custom `palette` overlays colors with the same names in the scheme, so a module style such as `fg=accent` uses your palette's `accent`. Formats and indicator rules remain under your control; explicit colors in a format or rule are not replaced. Unknown scheme names and invalid colors or styles fail `check-config`. Without a colorscheme, styles use terminal colors directly unless you select a custom palette as above.

## Window indicators

The default sessions module does not display custom tmux options. A configuration can give user options safe aliases and derive one indicator from them:

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

Available providers are `anthropic`, `copilot`, `gemini`, `antigravity`, `codex`, `kiro`, `zai`, and `xai`. By default, none are selected. Set `providers` explicitly to enable usage and choose the provider order.

`format` supports `$name` (window duration or provider label), `$remaining` (time until reset, when available), `$percent` (quota used), and `$bar`, plus optional and styled groups. For windows lasting more than 24 hours and at most seven days, `$bar` has one block per day; these blocks divide total usage, rather than showing usage on individual calendar days. Shorter windows use five blocks. Windows without a known duration use a ten-cell bar. Custom formats render as written.

Use `provider_style` for headings and `window_style` for usage rows. `critical_bar_style` applies at 80% used. For multiday windows with a known reset time, `warning_bar_style` applies when usage exceeds the allocation through the current day. Critical styling takes priority. `bar_track_color` sets the empty bar's background. `stale_style` applies to old cached readings, and `unavailable_style` applies when usage is unavailable.

For GPT subscription usage alone, set `providers = ["codex"]`. Codex credentials come from Pi's `~/.pi/agent/auth.json` or Codex's auth file; Pi and pi-usage do not need to be running.

Set `cache_dir` to an absolute path under `[usage]` to change the usage cache location. The default is `starmux/usage/` under the platform's cache directory. Styles accept palette colors and the same validated attributes as other modules.

## Live Pi sessions

Add `pi-workbench` to `modules` to show a state icon and the name of each reachable Pi session published by pi-workbench:

```toml
modules = ["sessions", "divider", "pi-workbench"]

[pi-workbench]
format = "  $state $name"
heading_style = "bold"
project_style = "bold"
idle_style = "fg=brightblack"
working_style = "fg=yellow"
notify_style = "fg=magenta"
selected_style = "reverse,bold"
selected_fill = "default"
```

In full layout, `heading_style` styles the `π Sessions` heading and `project_style` styles each project heading. Slim layout omits these headings.

The row format supports `$state` (a `●` icon) and `$name`, optional groups, and validated styled groups. `idle_style`, `working_style`, and `notify_style` color the state icon. The selected Pi pane uses `selected_style` and `selected_fill`. When a session has no name, `$name` shows the first eight characters of its session ID. Styles can use colors from the selected palette.

With a custom tmux mouse binding, route `sp` user ranges to `starmux activate` as well as `sw` ranges; `starmux init tmux` emits the required binding.

Pi sessions require pi-workbench to publish session status. The default data directory is `~/.local/share/pi`. Set `data_dir` to an absolute path under `[pi-workbench]` when pi-workbench uses another data directory. If your config uses `pi-live`, rename both the module list entry and `[pi-live]` table to `pi-workbench` and `[pi-workbench]`.

## Selected Pi context

Add `pi-context` to show the plans, pull requests, builds, and loaded skills recorded by pi-workbench for the Pi session in the selected tmux pane. It can be placed independently of `pi-workbench`:

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
build_success_style = "fg=green"
build_failure_style = "fg=red"
build_pending_style = "fg=yellow"
```

Use `heading_style` for the selected session's `π` heading, `category_style` for the Plans, PRs, Builds, and Skills labels, and `text_style` for item text. `plan_style` and `skill_style` style the plan and skill icons. PR icons use `open_style`, `draft_style`, `merged_style`, `closed_style`, or `unknown_style` according to their state.

When a pull request's branch has a build pushed in the session, its icon takes the build's color instead of its state color. Builds lists pushes without a pull request, such as pushes to `main`, as `repo:branch` after `✓` (passed), `✗` (failed), `●` (running), or `·` (no checks). Click a pull request to open it on GitHub, or a build to open its commit's checks. Build icons use `build_success_style`, `build_failure_style`, or `build_pending_style`; builds without checks use `unknown_style`. Builds require the pi-git and pi-github extensions from pi-workbench.

Plans and skills open with the system's file handler by default. Set `open_command` under `[pi-context]` to override that handler:

```toml
open_command = ["dev", "tmux", "edit", "{file}", "{pane}", "{socket}"]
```

[Start the Pi plan browser](https://github.com/juanibiapina/pi-workbench/tree/main/packages/pi-plans#open-and-review-a-plan), then set its URL to open plans from the sidebar:

```toml
[pi-context]
plan_server_url = "http://127.0.0.1:19433"
```

Starmux passes each array item as a separate process argument without a shell. `{file}` is required exactly once; `{pane}` is the selected tmux pane ID and `{socket}` is the current tmux server socket. This example opens the file in that pane's tmux session's Neovim editor window. The command runs only for plan and skill rows. Styles accept palette colors. Use `disabled = true` to hide the module.

Install `gh` and authenticate with GitHub to enable PR state lookups.

## Gob jobs

Add `gob` after a divider to show running jobs for the selected tmux pane's current directory:

```toml
modules = ["sessions", "divider", "gob"]

[gob]
format = "  $state $name"
heading_style = "bold"
running_style = "fg=green"
progress_style = "fg=green"
overdue_style = "fg=yellow"
label_style = "dim"
bar_track_color = "colour238"
```

Gob must be installed to show jobs. `format` accepts `$state`, `$name`, and `$id`. `$name` shows the job description, falling back to its command or ID. Use `heading_style` for the `Jobs` heading, `running_style` for the state icon, `progress_style` for the bar up to the typical run duration, `overdue_style` for the bar past it, `label_style` for the time label, and `bar_track_color` for the empty track.

Jobs that gob has seen finish show a progress bar below their name, followed by the elapsed and typical run time, such as `1m4s / ~20s`. The full bar is the time 90% of runs finish within. Once a job runs past that time, the label switches to `overdue_style`.

Jobs that usually run until stopped, such as servers, show no bar. The bar requires a gob version that reports expected durations. Rows have no click action by default. See [click actions](#click-actions) for overrides.

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

Starmux runs each argument array directly, without a shell, from the selected pane's current directory. A leading `~/` in an argument expands to the user's `HOME` directory; other shell expansions are not performed. Gitmux uses the pane directory by default. Commands run in module order on each sidebar redraw. Each command must finish within 500 ms. Stdout is limited to 16 KiB and only its first line is displayed as one row when nonempty. A missing executable, invalid directory, empty output, failure, or timeout adds no row. `starmux check-config` validates names, references, arguments, modes, and styles. A command may have at most 16 arguments, each at most 1024 bytes.

`prefix` adds literal text before the output; it defaults to empty and accepts up to 64 bytes without control characters. The default `output = "text"` escapes command output and applies `style` (default `default`). Set `output = "tmux-styles"` for gitmux's colored output. This mode accepts only `#[none]` and validated tmux text styles such as `#[fg=green,bold]`. All other directives and tmux formats remain literal text. Command rows have no click action by default. See [click actions](#click-actions) for overrides. Output is clipped to the sidebar width.

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

Each string in `lines` is one sidebar row. An entry with no nonempty variables produces no row. Remove `$upstream` to hide the upstream name, or move it to its own string to give it a separate row. The safe formatter supports optional groups and validated styled groups. Variables are `$branch`, `$upstream`, `$ahead`, `$behind`, `$divergence`, `$staged`, `$modified`, `$untracked`, `$conflicts`, `$stash`, `$added`, `$deleted`, `$clean`, and `$state`. Zero counts are empty. Branch names are escaped and clipped. The rows have no click action by default. See [click actions](#click-actions) for overrides. Set `disabled = true` in `[git]` to omit the section.

`$staged`, `$modified`, `$untracked`, and `$conflicts` count files; one file can be both staged and modified. `$added` and `$deleted` count tracked line changes against HEAD across staged and unstaged edits. Untracked files and binary changes have no line totals. `$clean` shows `✔` when no files have changed and no Git operation is active; stashes do not make the worktree dirty. `$stash` counts repository stashes, shared across linked worktrees. `$upstream` and `$divergence` use local refs without fetching. `$state` shows an active merge, rebase, cherry-pick, or revert. Detached HEAD shows a short commit ID.

The `[git]` styles are `branch_style`, `upstream_style`, `divergence_style`, `staged_style`, `modified_style`, `untracked_style`, `conflicts_style`, `stash_style`, `added_style`, `deleted_style`, `clean_style`, and `state_style`. Each accepts validated tmux styles and palette colors. The default colors make additions green and deletions red.

## Debug timings

Add `debug` to show how long the previous completed redraw took for this tmux socket and client. Place it after `spacer` to keep it at the bottom:

```toml
modules = ["sessions", "spacer", "debug"]

[debug]
details = true
style = "dim"
```

The default shows only `last 12.34 ms`; `details = true` adds rows for nonzero `tmux`, `pi`, `pr`, `usage`, `gob`, `commands`, `git`, `format`, and `top` stages. `style` applies to all timing rows.

Set an absolute `cache_dir` under `[debug]` to change where timings are stored. The default is `starmux/debug/` under the platform's cache directory. Rows have no click action by default and use the configured validated style. `disabled = true` hides the module. The default module list does not include `debug`.

## Tmux integration

`starmux init tmux` generates the side-status render command, click binding, and wheel bindings. Position, width, and outer style remain ordinary tmux options.

Valid content widths are 2–300 columns. The tmux `side-status-width` includes one border column, so valid sidebar widths are 3–301 columns.

Set `mouse on` in tmux to enable sidebar clicks and wheel scrolling. Regenerate and source `starmux init tmux` after upgrading to install the bindings.
