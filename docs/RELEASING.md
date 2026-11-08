# Release pipeline

The GitHub Actions pipeline has two non-publishing gates:

- **CI** runs on every push and pull request. It checks formatting, Clippy,
  tests, and a `cargo publish --dry-run` package verification.
- **Release Check** runs when a `v*` tag is pushed or when it is started
  manually from the Actions tab. It additionally runs doctests and a release
  build.

Neither workflow can upload a crate to crates.io.

## Before creating a release tag

1. Update the version in `Cargo.toml`.
2. Move the relevant entries from `Unreleased` to a dated version section in
   `CHANGELOG.md`.
3. Run `make validate`, `cargo test --doc`, and `cargo build --release`.
4. Commit the release preparation and push the branch. Require the CI workflow
   to pass in the pull request.
5. Create and push an annotated tag matching the manifest version, for example
   `v0.3.1`. Confirm that **Release Check** passes.

## Publishing after approval

Publishing must remain a separate, manually approved workflow. Create a
protected GitHub Environment named `crates-io`, add a required reviewer, and
store a crates.io token as its `CARGO_REGISTRY_TOKEN` secret. A publish workflow
may then use that environment and run `cargo publish --locked`; it must be
triggered manually after the release check for the matching tag succeeds.

Do not add the token to the repository, logs, or local configuration committed
to source control.
