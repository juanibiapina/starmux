# Starmux

**A fast and configurable sidebar for tmux.**

- Navigate: click a session or window to switch to it.
- Stay oriented: follow the active window when the list exceeds the screen height.
- Compose: choose and order built-in sidebar modules.
- Customize: format rows and colors without allowing tmux-format injection.

[Configuration](docs/configuration.md)

## Requirements

- A build of [tmux PR #5468: Add a vertical (side) status line](https://github.com/tmux/tmux/pull/5468).

## Installation

Install Starmux:

```sh
brew install juanibiapina/taps/starmux
```

Generate the tmux adapter:

```sh
mkdir -p "$HOME/.config/tmux"
starmux init tmux > "$HOME/.config/tmux/starmux.conf"
```

Choose the sidebar geometry and outer style in `.tmux.conf`, then load the adapter:

```tmux
set -g side-status left
set -g side-status-width 30
set -g side-status-style default
source-file ~/.config/tmux/starmux.conf
```

The generated adapter reads `side-status-width` at render time, so tmux remains the source of truth for the width. Regenerate the adapter after installing a new Starmux version.

The defaults need no Starmux configuration file. Run `starmux check-config` to validate a custom configuration and `starmux print-config` to inspect its effective values.

Starmux is [MIT licensed](LICENSE).
