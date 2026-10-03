# Configuration

Starmux reads `$XDG_CONFIG_HOME/starmux.toml`, falling back to `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses portable defaults.

Run `starmux print-config` to print the complete configuration. Run `starmux check-config` to validate module names, formats, styles, palettes, and tmux option aliases. If a sidebar render fails, Starmux shows the error in red, including configuration parse details. It wraps long lines to the sidebar width; rows beyond the available height may be hidden. Run `starmux check-config` for the full configuration error, or run the failing `starmux render-query` command in a terminal for other errors.

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

`starmux init tmux` selects `default`. For another tmux render area, copy its generated render invocation and add `--config=right` after `render-query`. Pass the same `--config=right` to custom `starmux activate`, `starmux scroll`, and `starmux scroll-event` bindings for that area. Scroll positions are separate for each list on a client. Tmux controls the placement and width of each area; Starmux renders its selected rows within the supplied width and available height.

## Slim sidebar

Content width two automatically uses the slim layout. Content widths of three or more columns use the full layout. The minimum content width is two. Tmux reserves one sidebar column for its border: set `side-status-width` to 3 for two content columns or 31 for thirty. The generated adapter subtracts the border from the width passed to Starmux. Slim rows have no leading indent and show one or two glyphs per source item.

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

The default slim list inherits the top-level `modules`. Each named list inherits its own `modules` unless it defines `slim_modules`; it does not inherit `[slim].modules`. All lists share the same mode, window visibility, module settings, styles, colorscheme, and disabled flags. Only modules in the effective list run their source queries. Commands referenced exclusively by a slim list are supported.

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
| Job progress | `↳` | Estimated elapsed-time gauge |
| Git | `` conflicts, `↻` operation, `` dirty, `✓` clean, in priority order | `↕` diverged, `↑` ahead, `↓` behind, `·` neither |
| Named command | Configured `slim_icon`, default `` | One glyph only |
| Debug timing | `◷` previous timing available, `󰋗` unavailable | One glyph only |
| Divider | Configured glyph; `─` when it needs two columns | Repeat across the second column |

Provider icons are Anthropic `󰚩`, Codex ``, Copilot ``, Gemini `✦`, Antigravity `◎`, Kiro `󰊠`, z.ai `󰘦`, and xAI `󰖟`. The rail uses Nerd Font icons, as do the full-layout host and PR rows. Usage and host pies use `○ ◔ ◑ ◕ ●`, rounded to the nearest 25%. Usage pies show quota consumed; host pies show CPU/memory used and battery charge remaining. Job gauges use `░` for zero and `▁` through `█` for eight increasing bands. Missing data is not zero.

Host metrics retain their existing critical thresholds. Unavailable readings use dim metric icons. Critical readings use bold critical styling and retain their pie. Battery icons show normal `󰁹`, low `󰁺`, charging `󰂄`, or full `󱟢` state. Full takes priority over charging. Cached host readings use the stale style. Cached usage after a failed refresh remains dimmed.

Slim mode omits usage provider headers when valid windows are available, names, initials, numeric indices, system/job headings, Pi project headings, Pi context headings, plan/PR category labels, usage cache-age rows and reset counts, and detailed debug rows. Plans and PRs keep individual click targets and original selector metadata. Skills and their category are omitted from slim mode. Click rules for skills and their category apply to full-layout rows. Job progress retains its separate `part = "progress"` row. Git uses one `kind = "summary"` row; full-layout `kind = "line"` and `slot` rules apply only to full rows. Hidden headings have no row for a heading click rule to match.

Full layout supplies names, individual skills, exact readings, counts, reset times, and timings. Use `starmux explain` for source details and `starmux timings` for measurements. Debug markers convey availability only, so omit `debug` from slim lists when it adds no useful information.

Named commands show an icon only for successful, nonempty output. Starmux does not infer status from arbitrary output text:

```toml
[commands.build]
argv = ["my-status", "--short"]
slim_icon = ""
```

`slim_icon` must be one visible, one-column grapheme without whitespace or control characters. Full formats remain full-layout settings; slim rows use the representations above.

Full and slim layouts keep separate scroll positions for each client and named list. Regenerate and source `starmux init tmux` to install bindings that pass the render area's width. Custom named-area bindings should pass the same `--config=NAME` and `--width=COLUMNS` to rendering, activation, and scrolling. If activation or scrolling omits `--width`, it queries the client's side-status width and subtracts the border column. Custom clicks from a previous layout, width, or list are rejected.

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
| `pi-context` | `heading`, `category`, `plan`, `skill`, `pr` | Categories: `category` (`plans`, `skills`, `prs`); plans: `file`, `title`; skills: `name`, optional `file`; PRs: `url` |
| `sessions` | `session`, `window` | `target_session`, `name`; windows also have `target_window`, `index` |
| `pi-workbench` | `heading`, `project`, `session` | `project`; sessions also have `name`, optional `target_pane`, `target_window` |
| `git` | `line`, `summary` | Full-layout lines have `slot`: entry in the configured `git.lines` list; the slim summary has no slot |
| `command.NAME` | `output` | Select the command through its full module name |
| `debug` | `total`, `stage` | `stage`: `tmux`, `pi`, `pr`, `usage`, `gob`, `commands`, `git`, `format`, `top` |
| `divider` | `divider` | Select repeated dividers through `instance` |
| `blank` | `blank` | Select repeated blanks through `instance` |
| `spacer` | `padding` | `slot`: position within the spacer allocation |

`part` is `main` for ordinary rows. Gob uses `name` and `progress`. `occurrence` distinguishes otherwise identical descriptors in source order. `instance`, `slot`, and `occurrence` start at 1. Window `index` starts at 0. These values and `duration_seconds` are integers. Other selectors are strings.

Git `slot` includes configured lines that currently produce no output. Spacer slots follow the current allocation. They do not identify a screen row after scrolling.

### Command arguments and errors

Common placeholders are `{socket}`, `{client}`, `{session}`, `{window}`, `{pane}`, `{path}`, `{module}`, `{kind}`, and `{part}`. Focus placeholders refer to the originating client. `{path}` is its selected pane's directory.

Source placeholders use the selector names in the catalog, including `{occurrence}`. Foreign session, window, and pane rows expose their targets through `{target_session}`, `{target_window}`, and `{target_pane}`. Optional metadata is available only when the source provides it. File placeholders require an existing regular file.

Placeholders can occur inside an argument, such as `{session}:4`. Doubled braces produce literal braces: `{{literal}}` becomes `{literal}`. Inserted values remain literal and receive no further substitution.

If an action requires unavailable metadata, the row retains its built-in action. `starmux explain` reports the unavailable placeholder. An earlier rule does not replace the winning rule.

Starmux runs argument arrays directly in the originating pane's directory. It does not interpret a shell, tmux formats, environment variables, or `~`. Scripts support more complex commands. Interactive programs need a terminal, such as the pane in the monitor example.

Action definitions accept 1–16 arguments, each at most 1024 bytes, without NUL bytes. The executable must be a nonempty literal. Expanded arguments use the same limits. The configuration accepts at most 64 actions and 128 rules.

Commands have a two-second deadline and a 16 KiB stderr limit. Starmux kills and reaps the direct child after a timeout. Scripts own the lifetime of their descendants. Missing executables, nonzero exits, and invalid directories return an activation error. Errors do not run a fallback action.

Custom clicks validate the current source identity, module list, action definition, and pane context. Removed items and changed context invalidate old clicks. Usage clicks read cached data without a provider refresh. Command rows use the configured command identity without rerunning the status command. Other dynamic rows must still exist during validation.

Regenerate and source `starmux init tmux` after installing this feature. Custom mouse and wheel bindings must route `sc` ranges with the existing Starmux ranges.

## Host status and persistent state

Add `top` to a module list to show host CPU, memory, and battery status. Put it after `spacer` to keep it near the bottom:

```toml
modules = ["sessions", "spacer", "divider", "top", "divider", "git"]

[top]
metrics = ["cpu", "memory", "battery"]
```

The module shows a `SYSTEM` heading and compact rows with Nerd Font icons, percentages, and aligned CPU, memory, and battery bars. CPU is whole-host usage measured over an interval; memory is used physical memory. A battery row appears when a battery is detected, with its icon showing normal, low (at or below 20%), charging (lightning bolt), or full (check mark) state. Full state takes priority over charging. Hosts without a battery show only CPU and memory. An unavailable metric shows `--`; cached data older than one minute is dimmed and marked with its age, and expires after five minutes. Short widths keep the numeric value and reduce or omit the bar. Rows have no click action by default. See [click actions](#click-actions) for overrides. Set `disabled = true` to hide the section. Use `heading_style`, `value_style`, `warning_style`, `critical_style`, and `track_style` in `[top]` to override colorscheme defaults. CPU and memory turn red at 80%; battery turns red at or below 20%. Sampling happens in a background process roughly every ten seconds; a completed sample appears on the next tmux redraw.

Starmux stores persistent state beneath `$XDG_CACHE_HOME/starmux` when `XDG_CACHE_HOME` is set, `~/Library/Caches/starmux` by default on macOS, and `~/.cache/starmux` by default on Linux. Separate `top/`, `usage/`, `pr-state/`, `debug/`, and `scroll/` directories keep their records apart. Set a top-level absolute `cache_dir` to change the root. Existing `[usage].cache_dir` and `[debug].cache_dir` settings take priority for their respective modules. Older macOS PR, debug, and scroll caches under `~/.cache/starmux` are disposable and expire in place.

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

Each scheme supplies default styles and fills for top, sessions, dividers, Pi Workbench, Pi context, git, usage, gob, debug, and text-mode named commands. The scheme colors ordinary text in these modules too. `spacer` and `blank` contain no styled content. A scheme does not set tmux's outer `side-status-style`, change tmux's other status options, or recolor styles supplied by an external command in `tmux-styles` mode.

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

Available providers are `anthropic`, `copilot`, `gemini`, `antigravity`, `codex`, `kiro`, `zai`, and `xai`. By default, none are selected. Set `providers` explicitly to start fetching usage. Providers appear in the listed order. Each provider gets a heading and one row per available usage window. When no cached usage exists, its heading shows `(unavailable)`. The default window row shows its duration, time until reset, progress bar, and used percentage. When multiple windows are visible, these fields align across providers. In a narrow sidebar, the bars lose their spaces first so the percentage remains visible. Custom `format` values render as written. For windows lasting more than 24 hours and at most seven days, `$name` shows the duration in days and `$bar` has one block per day. The blocks divide the total window usage into visual steps; they do not represent usage on individual calendar days. Partial blocks fill from the bottom over the same background track as empty blocks. Only the bar turns red at 80% used. For multiday plans, it turns orange when usage exceeds the allocation through the current day (one day's share per day). Red takes priority. The day comparison requires both a known window duration and a future reset time. Set `critical_bar_style`, `warning_bar_style`, and `bar_track_color` to change the colors. Shorter windows use five progress blocks. Windows without a known duration use their provider label and a ten-cell bar. `$remaining` shows the time until reset when the provider supplies a reset timestamp; otherwise the optional group omits it. Window formats support `$name`, `$remaining`, `$percent`, and `$bar`, plus optional and styled groups. Cached usage older than 30 minutes shows its age, such as `(31m old)`, and uses `stale_style`. After a failed refresh, the heading shows `sign in again` for HTTP 401 or `refresh failed` for other errors, with a separate `cached 1m old` row when an earlier reading exists, even if it is less than 30 minutes old. A failed refresh keeps the last good usage, dims the heading, and hides any cached Codex reset count until a successful refresh. Without cached usage, the heading shows the failure status; before the first result, it shows `(unavailable)`. A successful refresh clears the failure status. Usage text is escaped and clipped. Click a heading or plan row to open the provider's usage page for Claude, Codex, GitHub Copilot premium requests, or the z.ai personal Coding Plan. Gemini CLI, Antigravity, Kiro, and Grok rows have no browser click target because a direct page showing the fetched usage has not been established. The z.ai link opens the personal plan page; team plans have a separate page.

For GPT subscription usage alone, set `providers = ["codex"]`. When Codex reports its available reset count, the heading shows a muted label, such as `Codex Plan · 1 reset` or `Codex Plan · 0 resets`. A missing count leaves the heading unchanged. Starmux reads Pi's `~/.pi/agent/auth.json` or Codex's auth file for Codex credentials; it does not require Pi or pi-usage to run. A foreground sidebar render returns cached usage immediately and starts a refresh worker when data is absent or at least 60 seconds old. One Starmux worker per provider holds the lease, so concurrent clients do not multiply requests. Failed requests preserve last-good usage and delay retries using the provider's `Retry-After` header or a 60-second fallback. The next tmux redraw shows a completed refresh. Starmux's cache and locks are separate from pi-usage; if both run, each may make a request within the same minute.

Set `cache_dir` to an absolute path under `[usage]` to change the usage cache location. The default is the `usage/` directory under the shared Starmux cache root. Styles accept palette colors and the same validated attributes as other modules.

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

When Pi sessions are available, full layout shows a `π Sessions` heading styled by `heading_style` above the project groups. Slim layout omits this heading. Pi sessions are grouped by project. Starmux finds the nearest Git root above each published cwd; when there is no Git root, the cwd is the project. Each project gets one heading styled by `project_style`. Groups sort alphabetically by project name, with the full path distinguishing projects with the same name. Sessions within a group sort alphabetically by session name. State and selection changes update styling without changing the order. Renaming a session or changing its project can move its row. A project heading adds one sidebar row.

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

[Start the Pi plan browser](https://github.com/juanibiapina/pi-workbench/tree/main/packages/pi-plans#open-and-review-a-plan), then set its URL to open plans from the sidebar:

```toml
[pi-context]
plan_server_url = "http://127.0.0.1:19433"
```

Starmux passes each array item as a separate process argument without a shell. `{file}` is required exactly once; `{pane}` is the selected tmux pane ID and `{socket}` is the current tmux server socket. This example opens the file in that pane's tmux session's Neovim editor window. The command runs only for plan and skill rows. Clicks resolve the selected Pi session again and reject stale or missing targets. The `pi-skills` namespace records paths for loaded skills, including cached GitHub skills. A skill without a recorded path uses `~/.agents/skills/<name>/SKILL.md` or `~/.pi/agent/skills/<name>/SKILL.md` when present. Text is escaped and clipped to the sidebar width. Styles accept palette colors. Use `disabled = true` to hide the module.

The module reads the selected reachable session's version 2 namespaced context file through its Pi status record. Switching panes updates the rows. No selected Pi session, an empty context, a missing file, or an invalid file produces no context rows. The Pi status scan is shared with `pi-workbench` when both modules are enabled. It shows at most 16 entries in each category.

PR states come from authenticated `gh api` requests in a background worker. A sidebar render uses the cached result immediately and refreshes eligible PRs after five minutes. At most 16 PRs can start a refresh per render, matching the displayed PR limit. Concurrent clients share a lease per PR. Failed lookups retry after one minute, preserve the last known state for up to 30 minutes, then show a muted ``. A missing `gh` command or GitHub authentication leaves the association visible with unknown state. The cache lives under `pr-state/` in the shared Starmux cache root.

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

The section shows `Jobs`, then a green dot and description for each running job. If a job has no description, `$name` shows its command or job ID. The format also accepts `$id`. A job with a previous successful-run average gets a second row with a horizontal progress track below its name. The track fills the space after its indentation and before a four-column percentage field that fits `100%`, with two more spaces to its right; filled cells show elapsed time divided by the previous average, capped at 100%. A job can still be running when the track is full. When history or a valid start time is absent, there is no progress row. Narrow widths reduce the indentation to keep a track cell when both the track and percentage fit; the percentage takes priority at smaller widths. Rows have no click action by default. See [click actions](#click-actions) for overrides. All job text is escaped and clipped.

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

The default shows only `last 12.34 ms`; `details = true` adds rows for nonzero `tmux`, `pi`, `pr`, `usage`, `gob`, `commands`, `git`, `format`, and `top` stages. These are source query and formatting times: several sidebar modules share a source, and `commands` combines configured external commands. The total is the sum of the stages, subject to rounding when displayed. The interval begins before the tmux snapshot and ends after formatting the sidebar. It excludes process startup, reading and writing the debug cache, and tmux's evaluation of the resulting status text. Background refresh workers are not included; starting them during a redraw is included. `starmux timings` includes `top_us` for host cache lookup and runs its own query without starting refresh workers.

The first redraw shows `last --`. Each completed redraw records its measurement for the next one; the value expires after five minutes. Cache errors also show `last --` and do not interrupt the sidebar. The default cache is `debug/` under the shared Starmux cache root; set an absolute `cache_dir` under `[debug]` to change it. Rows have no click action by default and use the configured validated style. `disabled = true` omits the rows and cache access. The default module list does not include `debug`.

## Tmux integration

`starmux init tmux` generates the side-status render command, click binding, and wheel bindings. Position, width, and outer style remain ordinary tmux options.

The adapter passes `#{e|-:#{side-status-width},1}` as the explicit `--width` argument, excluding tmux's border column. Valid content widths are 2–300 columns; set `side-status-width` to at least 3. If the client switches sessions or windows during a redraw, Starmux discards that stale render; the next redraw shows the new focus. Other status clicks retain tmux's default action.

With `mouse on`, wheel up and down over the sidebar scroll the complete list one row at a time, including rows from every configured module. The selected tmux window stays unchanged; click a visible row to activate it. Each attached client keeps its own position. Scrolling stops at the first and last page and adjusts to changes in content or client height. Wheel events over the ordinary horizontal status keep tmux's window selection behavior. Regenerate and source `starmux init tmux` after upgrading to install the wheel bindings.
