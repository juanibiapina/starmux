# Releasing Starmux

Prepare the release on `main`:

- Update the version in `Cargo.toml` and the Starmux package entry in `Cargo.lock`.
- Move the `[Unreleased]` notes in `CHANGELOG.md` into a dated version section. Leave an empty `[Unreleased]` section above it.

Commit those three files with the message `Release starmux X.Y.Z`. Create the tag `vX.Y.Z` on that commit, then push `main` and the tag together using an atomic push. Replace `X.Y.Z` with the release version.

Wait for the matching Release workflow in Github.

The release is complete when the Release workflow succeeds, including Homebrew publication.
