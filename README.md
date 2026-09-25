# Starmux

**A fast and configurable sidebar for tmux.**

- **Navigate:** Click a session or window to switch to it.
- **Stay oriented:** Follow the active window, even when the list exceeds the screen height.
- **See context:** Show working and notification markers.
- **Make it yours:** Order modules and add command-backed rows with TOML.

[Installation](#installation) · [Configuration](docs/configuration.md)

## Installation

### Requirements

- A build of [tmux PR #5468: Add a vertical (side) status line](https://github.com/tmux/tmux/pull/5468).

### 1. Install Starmux

```sh
brew install juanibiapina/taps/starmux
```

### 2. Configure tmux

Create the Starmux configuration for tmux and load it:

```sh
mkdir -p "$HOME/.config/tmux"
starmux init tmux > "$HOME/.config/tmux/starmux.conf"
tmux source-file "$HOME/.config/tmux/starmux.conf"
```

To load Starmux after a restart, add this to `.tmux.conf`:

```tmux
source-file ~/.config/tmux/starmux.conf
```

If the sidebar is empty, make sure that tmux supports `side-status` and that `starmux` is on your `PATH`.

### 3. Configure the sidebar

The defaults work without a configuration file. To add a clock below the divider, create `~/.config/starmux.toml`:

```toml
format = "$sessions$divider$clock"

[provider.clock]
command = ["date", "+%H:%M"]
decoder = "plain"
dependencies = []

[module.clock]
provider = "clock"
```

Use `starmux check-config` to validate the configuration. Use `starmux print-config` to show the complete configuration.

See the [configuration guide](docs/configuration.md) for providers, window markers, and output formats.

Starmux is [MIT licensed](LICENSE).
