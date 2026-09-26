# Plan: live Pi sessions in the Starmux sidebar

## Goal

Add an opt-in `pi-live` sidebar module that shows live Pi sessions published by pi-live. Treat “sections” in the request as “sessions”: pi-live publishes one record per Pi session. Show a recognizable name and a state icon (`●`, gray for idle, yellow for working, magenta for attention). Group sessions by project, order groups by their most urgent session, highlight the selected Pi pane, and navigate to its pane on click; allow the user to place the module among `sessions` and `divider`. An absent publisher or empty store produces no Pi rows. Keep the current default module list and tmux navigation behavior.

## What exists and why the module needs its own data path

Starmux renders an ordered list of `sessions` and `divider` modules from a tmux `Snapshot`. `Application<T: Tmux>` queries tmux once, validates focus, and calls `Sidebar::render`. `src/sidebar.rs` owns configuration, safe formatting, styles, clipping, and row rendering. Text variables are escaped and styles are validated before emitting tmux side-status syntax. `src/tmux.rs` owns the versioned tmux snapshot transport; Pi status is independent of that transport. The generated adapter runs `starmux render-query` from `side-status-format`.

The pi-live publisher lives in the sibling dotfiles repository at `dotfiles/pi/.pi/agent/lib/pi-live/`. Its `status-store.ts` writes atomic, private `~/.local/share/pi/status/<sessionId>.json` records with schema `version: 1`. Fields include `sessionId`, optional `name`, `pid`, `cwd`, `socketPath`, `startedAt`, `updatedAt`, `state` (`idle` or `working`), and optional `tmux` location (`paneId`, `sessionName`, `windowIndex`, `windowName`). `runtime.ts` updates these on lifecycle events and removes them on shutdown. `session-client.ts` lists only live sessions by sending `{ "type": "ping", "protocolVersion": 1 }` to each published Unix socket and requiring `{ "ok": true, "result": { "type": "pong" } }`; it can remove stale records. The status files can survive a crash. The separate `pi-tmux.ts` extension publishes window indicators (`@pi_win_state`); those indicators are not the Pi session records.

## Technical approach

1. Add a small Pi status reader module (for example `src/pi_live.rs`) that owns the version 1 file schema, record validation, directory discovery, liveness checks, and a bounded result. Default to pi-live's data directory; allow an explicit data directory in `[pi-live]` for custom publishers and deterministic integration tests. Only scan regular `.json` files with valid session IDs, bound file count and file/response sizes, reject invalid or future versions, and ignore an individual broken record. Validate status fields used for rendering; treat names and paths as untrusted text. Check sockets with a short deadline and a cap on total query time so a busy or dead publisher cannot stall every redraw. Use the nearest Git root above each cwd as the project key, falling back to cwd outside Git. Join records to current tmux panes, keep projects together, and sort groups and sessions by attention, working, selected idle, then idle. Do not delete another process's status or socket files. Missing directory means no sessions; report unexpected directory errors with context, while one bad file/socket does not blank the rest of the sidebar. Define the exact limits and failure behavior with the reader implementation.
2. Extend configuration in `src/sidebar.rs` with `pi-live` as a known module and `[pi-live]` options: `disabled`, `data_dir`, row `format`, `project_style`, `idle_style`, `working_style`, `notify_style`, `selected_style`, and `selected_fill`. Compile the format once using the existing parser and validate only `$name` and `$state`. Supply a default format that shows state and name (falling back to a short ID when the name is absent). Resolve styles through the existing palette rules. `modules = ["sessions", "divider", "pi-live"]` enables it; preserve `["sessions", "divider"]` as the portable default. `check-config` and `print-config` must include the new settings.
3. Integrate the read at the application render path only when `pi-live` is enabled. Pass the resulting Pi statuses alongside the tmux snapshot into the sidebar through a clear render input, or an explicit Pi-aware render method. Keep tmux querying and its versioned producer/parser untouched. Keep Pi reading outside the pure formatting code; use the actual filesystem with a temporary data directory for tests rather than introducing a new public trait solely for a test adapter. Have `render-query`, `explain`, and `timings` reflect the same enabled data path; measure Pi reading separately in `timings` if useful for diagnosing redraw cost. Ensure the public `Sidebar` interface makes the additional input explicit and update existing callers/tests accordingly.
4. Render one styled heading per project followed by its Pi sessions using the existing safe formatter, clipping, and row layout. Use read-only rows when a published pane has no match on the current tmux server. For matching panes, use a validated pane/window click token and current pane identity for selected styling; resolve the pane again on click. Do not put `sessionId`, names, paths, or socket paths into tmux format strings, styles, or click ranges. Use `●` for all three states, with separate validated styles for idle, working, and attention. If the Pi module has no live records, emit no rows; the configured divider remains independent.

### Alternatives considered

- Reading `@pi_win_state` from tmux would show one rolled-up window state, not distinct Pi sessions or names; keep the existing indicator feature separate.
- Adding Pi fields to the tmux snapshot transport would couple a file/socket publisher to the tmux producer and require changing its lockstep versioned parser. Read pi-live independently.
- Using the publisher's startup window index as a click target would fail after window renumbering. Resolve the pane's current window from tmux.
- Trusting JSON files or PIDs alone would show orphaned sessions after a crash or PID reuse. Use the publisher's ping contract for liveness, with bounded time and partial failure handling.

## Implementation phases

1. **The reader returns only valid, live version 1 records.** Add the schema, location configuration, guarded file reads, socket ping, sorting, and error policy. Exercise it with temporary stores and Unix sockets that respond, refuse connections, time out, or return malformed responses.
2. **The Pi module renders safely in the configured order.** Wire configuration and the read into `Application` and `Sidebar`; add safe rows, selected styles, and pane click tokens. Verify three Pi states, current-pane selection, attached-client click navigation, and two Pi records in different states, missing names, no publisher, module disabled, custom palette, clipping, and injection strings such as `#[range=user|bad]#{pane_id}`. Confirm `Sidebar` or CLI output has no injected style/range and other modules retain their order and navigation.
3. **Dotfiles enables the module with Tokyo Night colors.** In `juanibiapina/dotfiles/dotfiles/tmux/.config/starmux.toml`, append `"pi-live"` to `modules` after `"divider"`. Add `[pi-live]` with `project_style = "fg=text,bold"`, `idle_style = "fg=muted"`, `working_style = "fg=warning"`, `notify_style = "fg=accent"`, `selected_style = "fg=accent,bg=border,bold"`, and `selected_fill = "border"`, using the existing `tokyo-night` palette rather than hardcoded colors. Apply these styles to the state icons; render names in normal text styling. Use the module's default status-and-name format unless visual verification shows a need for an explicit row format. Run `STARMUX_CONFIG=<path-to-dotfiles-starmux.toml> starmux check-config` using the newly built binary and verify live sessions render in the intended order. This is an implementation change in the sibling repository; preserve any unrelated local changes there.
4. **The feature is documented and measured.** Update `docs/configuration.md` with an opt-in example, path and schema expectations, displayed fields, empty/error behavior, and refresh characteristics. Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`; use the supported tmux build with `STARMUX_REQUIRE_SIDE_STATUS=1 cargo test --locked --test tmux` for a live attached-client rendering check. Time `render-query` with several published sessions to ensure the bounded liveness checks remain suitable for sidebar refresh.

## Out of scope

Pi messaging, modifying pi-live's publication schema, modifying `pi-tmux.ts` or its tmux indicators, and making Pi records appear by default. Those require separate behavior and are not needed to display live sessions.

## Risks and dependencies

The file schema and ping request are defined in the sibling dotfiles repository, not Starmux. Pin to schema version 1 and protocol version 1, and use fixtures derived from that publisher so changes cannot silently render incompatible records. Socket ping on each redraw adds latency and file I/O; bound work, measure a realistic session count, and document how quickly state changes become visible. A per-client `#()` render may run often; avoid unbounded serial socket waits. The Pi `tmux` window location is captured at session startup and can become stale. Match the pane ID to the current tmux server; unmatched rows stay read-only. The published session name helps avoid a pane ID collision across tmux servers.

## Skills to use

- `vocabulary` — keep module, interface, and seam names consistent when changing the render interface.
- `deep-modules` — keep Pi file/socket handling behind one reader interface and test through observable behavior.
- `documentation` — update the configuration guide and keep schema details sourced from pi-live.
- `testing` — design reader and render tests around observable liveness and safety.
- `reproducible-locally` — verify the attached-client display on a supported tmux build.

## Acceptance criteria

- With `pi-live` enabled and two reachable published sessions, the sidebar shows both with correct names (or fallbacks) and states in the configured module position; rows do not display cwd.
- With no records, an unreachable socket, malformed JSON, or an unsupported schema, Starmux does not display a false live session; one bad record does not suppress a healthy one.
- Pi text containing tmux syntax stays literal and clipped; only rows matched to current panes have click ranges. A clicked row selects its pane in the correct window, and a stale pane token is rejected.
- A project with a pane marked `notify` sorts ahead of other projects, and that session shows a magenta icon at the top of its project; the selected Pi pane has the selected style.
- Disabled or unconfigured `pi-live` support does not scan the status directory or alter the existing output; `check-config` rejects invalid Pi formats/styles.
- The sibling dotfiles configuration enables `pi-live`, overrides idle, working, attention, and selected styles with Tokyo Night palette names, passes `check-config`, and renders live records.
- The specified Rust checks and the relevant live tmux check pass, and redraw latency is measured with multiple sessions.
