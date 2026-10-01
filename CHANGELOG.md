# Changelog

## [Unreleased]

### Added

- Show a `π Sessions` heading above the Pi session list in full layout.
- Install Starmux through a Nix flake on Linux and macOS.
- Hide inactive tmux sessions with `[sessions].active_only`, or hide only their windows with `[sessions].fold_inactive`.

## [0.6.0] - 2026-10-01

### Added

- Add slim mode with a two-column sidebar of icons and progress gauges.
- Configure click actions for any sidebar row, with shared defaults and overrides for individual jobs, metrics, usage windows, plans, and skills.

### Fixed

- Keep sidebar content and click actions on the correct client when multiple tmux clients are attached.

## [0.5.1] - 2026-09-30

### Fixed

- Show a sign-in warning when a usage credential is rejected, with the age of the last cached reading.
- Align host status bars across percentages and show a battery bar, with battery state in its icon instead of clipped text.
- Show the specific error and configuration parse details in the sidebar when a render fails.

## [0.5.0] - 2026-09-28

### Added

- Show host CPU, memory, and battery status in an optional themed sidebar section.
- Keep sidebar cache files under one configurable directory, with separate folders for each feature.
- Select different module lists from one configuration file for separate tmux render areas.

### Changed

- Show the selected Pi session's name above its plans, pull requests, and skills in the sidebar.
- Scroll the complete sidebar with the mouse wheel, showing progress during rapid gestures and keeping direction changes in order without selecting another window.
- Show Gob job progress as a horizontal track below each job name, with the percentage on the right.

### Fixed

- Show one divider when empty sidebar sections leave dividers next to each other.
- Keep the sidebar visible when Gob fails, and avoid input errors during session switches.

## [0.4.0] - 2026-09-27

### Added

- Color sidebar modules with ten bundled colorschemes or define your own.
- Add an optional `debug` module that shows the previous sidebar redraw time and a breakdown by stage.
- Show available Codex resets beside the usage heading.
- Show output from multiple configured commands in the sidebar, including colored gitmux status for the selected pane's directory.
- Show configurable Git status rows with branch, upstream, changed files and lines, stashes, and active operations.

### Changed

- Configure live Pi sessions with `pi-workbench` and `[pi-workbench]` instead of `pi-live` and `[pi-live]`.

## [0.3.0] - 2026-09-27

### Added

- Show the selected Pi session's plans, pull requests, and loaded skills in the sidebar. Click rows to open their links, with a configurable command for plan and skill files.
- Show running gob jobs for the current pane's directory with green status dots and progress bars when past run times are available.
- Show subscription usage for selected providers in aligned columns, with quota warning colors, reset countdowns, and cache age after 30 minutes. Click Claude, Codex, GitHub Copilot, or z.ai usage rows to open the provider's usage page.
- Place dividers between multiple sidebar sections by listing `divider` more than once.
- Add the `spacer` module to push following sidebar sections to the bottom.
- Add the `blank` module to insert one empty row wherever it appears.

### Fixed

- Show live Pi sessions only in the sidebar of their own tmux server.

## [0.2.0] - 2026-09-26

### Added

- Configure built-in sidebar modules with safe row formats, named palettes, and window-option indicator rules.

### Changed

- Use portable colors and ASCII output by default; personal themes, icons, and status markers now live in user configuration.
- Read the sidebar width explicitly from tmux and leave sidebar geometry and outer styling in `.tmux.conf`.
- Remove command-backed providers and the legacy argv adapter in favor of built-in modules.

## [0.1.0] - 2026-09-25

### Added

- Show clickable sessions and windows in a left sidebar that follows the current window.
- Show working and notification markers.
- Configure sidebar modules and command-backed rows with TOML.
- Install prebuilt binaries on macOS and Linux with Homebrew or the shell installer.
