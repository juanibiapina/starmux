# Configuration

The CLI reads `$XDG_CONFIG_HOME/starmux.toml`. If `XDG_CONFIG_HOME` is unset, it reads `~/.config/starmux.toml`. Set `STARMUX_CONFIG` to select another file. An absent or empty file uses the built-in defaults. The CLI merges a file with those defaults.

Run `starmux print-config` to see the effective values. Run `starmux check-config` to find an invalid field or module reference.

## Tmux integration

`starmux init tmux` generates both `side-status-format` and the `MouseDown1Status` binding used to switch to windows in other sessions. Other status clicks retain tmux's default action.

Regenerate the adapter after installing a new Starmux version. If you define `side-status-format` or `MouseDown1Status` yourself, load the generated adapter last or copy its foreign-window handling into your configuration. Add the generated config to a shared tmux setup only after Starmux is installed on every affected host.

## Module order

`format` is a sequence of `$module` references. It accepts no text outside those references. The default is:

```toml
format = "$sessions$divider"
```

The built-in modules are `sessions` and `divider`. No command providers run by default. Add a named module by linking it to a provider:

```toml
format = "$sessions$divider$clock"

[provider.clock]
command = ["date", "+%H:%M"]
decoder = "plain"
dependencies = []

[module.clock]
provider = "clock"
```

## Window markers

A window shows `@window_icon`, or `|` if that option is empty. Set `@pi_win_state` to `working` or `notify` to replace the icon with a colored dot:

```sh
tmux set-option -w -t 'main:1' @window_icon ''
tmux set-option -w -t 'main:1' @pi_win_state working
```

Unset `@pi_win_state` to show the icon again. These values do not change the window name or its click target.

## Providers

| Field | Meaning | Default |
| --- | --- | --- |
| `command` | Executable and arguments as an array. Required for a new provider. | None |
| `cwd` | Working directory for the command. | `"{pane.path}"` |
| `decoder` | `plain` or `json-v1`. | `"plain"` |
| `timeout_ms` | Kill the provider after 1–60000 ms. | `150` |
| `cache` | Store valid results for the next status interval. | `true` |
| `dependencies` | Context fields in the cache key. Use `"pane.path"` and `"session.id"`. | `["pane.path"]` |
| `failure` | `preserve` keeps the last cached result. `hide` removes it. | `"preserve"` |
| `disabled` | Skip the provider. | `false` |
| `shell` | Execute the joined command array with `SHELL -c`. | Not set |

`command` arguments and `cwd` accept `{pane.path}` and `{session.id}`. The process receives `STARMUX_PANE_PATH`, `STARMUX_SESSION_ID`, and `STARMUX_PROVIDER`.

The CLI runs `command` as direct arguments unless you set `shell`. **Do not use untrusted values with `shell`**. The CLI joins command arguments with spaces before it starts the shell.

The generated adapter exits after its first snapshot. It refreshes providers in a separate process for the next tmux status interval. Keep `cache = true` for a provider that appears in this adapter. With `cache = false`, the background refresh cannot supply the next snapshot.

The cache is under `$XDG_CACHE_HOME/starmux/providers/` or `~/.cache/starmux/providers/`. The key includes the provider configuration and its declared context dependencies. Invalid cache entries are ignored. Providers run again when tmux updates the status, even after a cache hit.

## Output formats

- `plain`: UTF-8 text. Each line becomes one sidebar row. The renderer treats `#[]` and other tmux syntax as literal text.
- `json-v1`: UTF-8 JSON rows with semantic styles and optional fills. A provider cannot set tmux colors or click targets directly.

A `json-v1` result looks like this:

```json
{
  "version": 1,
  "rows": [
    { "segments": [
      { "text": "ready", "style": "detail", "fill": "highlight" }
    ] }
  ],
  "metadata": null
}
```

`style` accepts `default`, `footer`, or `detail`. `fill` accepts `background`, `highlight`, `border`, or `current`. The renderer limits each provider's stdout to 65,536 bytes. It does not show provider stderr in the sidebar. A provider error leaves session and window navigation intact.
