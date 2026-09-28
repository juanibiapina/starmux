# Starmux

A fast and configurable sidebar for tmux.

- View tmux sessions and windows, host CPU, memory, and battery status, live Pi sessions and selected Pi context from [pi-workbench](https://github.com/juanibiapina/pi-workbench), and AI provider usage
- Everything is clickable with a mouse and takes you to the expected place
- Supports colorschemes

![Starmux sidebar showing sessions and windows](docs/images/sidebar.png)

The screenshot uses my [custom Starmux configuration](https://github.com/juanibiapina/dotfiles/blob/main/dotfiles/tmux/.config/starmux.toml).

See [configuration](docs/configuration.md#palettes) for bundled and custom colorschemes.

## Requirements

- A build of [tmux PR #5468: Add a vertical (side) status line](https://github.com/tmux/tmux/pull/5468).

## Installation

Install Starmux:

```sh
brew install juanibiapina/taps/starmux
```

Generate the tmux adapter (may also be required after upgrades).

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
