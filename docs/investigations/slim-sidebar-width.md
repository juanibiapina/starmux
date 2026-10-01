# Slim sidebar clipping

The second glyph was clipped because Starmux counted tmux's border as a content column. Usage also retained a redundant provider header above its window rows.

## Evidence

At `side-status-width 2`, `starmux render-query --width=2` emitted `○`. An isolated attached tmux client with the same literal row emitted `│`: the border replaced the pie. The same probe clipped the second glyph of `A◑`. Tmux reported both glyphs as one column each.

The generated adapter in `src/main.rs` passed the entire sidebar width to rendering and interactions. `ProcessTmux::sidebar_width` in `src/tmux.rs` returned that same width. `Sidebar::slim_rows` in `src/sidebar/slim.rs` retained usage provider rows above valid windows.

## Correction and verification

The adapter and implicit width query now subtract the border column. Two content columns require `side-status-width 3`. Slim usage omits the provider header when valid windows exist; cached failures put their status beside the pie. A fallback row remains when no valid readings exist.

`clicking_usage_opens_the_provider_page` in `tests/tmux.rs` now asserts that `◔` reaches the attached terminal at two content columns, then verifies each window's click action. Run it with:

```sh
STARMUX_REQUIRE_SIDE_STATUS=1 cargo test --locked --test tmux clicking_usage_opens_the_provider_page
```
