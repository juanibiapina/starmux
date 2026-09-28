# Releasing Starmux

Starmux uses cargo-dist to create GitHub releases, shell installers, and a formula in `juanibiapina/homebrew-taps`. `Cargo.toml` sets `publish = false`, so releases do not publish to crates.io.

The GitHub repository must have a `HOMEBREW_TAP_TOKEN` Actions secret. Use a fine-grained personal access token with read/write access to the contents of `juanibiapina/homebrew-taps`.

## Local checks

From this repository:

```sh
cargo fmt --check
cargo clippy --quiet --locked --all-targets -- -D warnings -W clippy::fn_params_excessive_bools
cargo test --locked
cargo build --release --locked
package_target="$(mktemp -d)"
CARGO_TARGET_DIR="$package_target" cargo package --allow-dirty --locked
rm -rf "$package_target"
cargo run --release --example render_bench -- 200 1000
```

On a Mac with the [required tmux build](../README.md#requirements), require the attached-client test:

```sh
STARMUX_REQUIRE_SIDE_STATUS=1 cargo test --locked --test tmux
```

The sibling dotfiles configuration must pass `starmux check-config`. The attached-client tests in this repository cover rendering, focus, clicks, and literal window names.

## Before a release

- Validate the sibling dotfiles configuration and run the attached-client tests with the required tmux build on Linux. CI uses stock tmux and cannot establish Linux sidebar parity.
- Move the `[Unreleased]` notes in `CHANGELOG.md` into a dated version section and leave an empty `[Unreleased]` section above it.
- Update the version in `Cargo.toml`, run `cargo check` to refresh `Cargo.lock`, then run `cargo check --locked` to confirm it is current.
- Commit the release changes and repeat the local checks from a clean checkout.
- Push the release commit to `main`, then push a matching `vX.Y.Z` tag. The Release workflow creates the GitHub release and publishes `Formula/starmux.rb` to the tap.

For the first release, verify the workflow on a pull request first. Its release job runs `dist plan` without publishing. Confirm that `HOMEBREW_TAP_TOKEN` exists before pushing the tag.

To check that generated CI is current:

```sh
dist generate --check
dist plan
```

After publishing, test the formula on macOS with:

```sh
brew update
brew install juanibiapina/taps/starmux
starmux --version
```
