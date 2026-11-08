# Contributing

## Branching model

Development follows this progression:

```text
feature branch → develop → main → vX.Y.Z tag → crates.io
```

- Create feature and fix branches from `develop`.
- Open pull requests into `develop`; CI must pass before merging.
- Promote tested work with a `develop` to `main` pull request.
- `main` is the only branch from which releases are tagged.

The protected branches require pull requests and these checks:

- `validate (1.71)`
- `validate (stable)`
- `package`

## Local validation

Install `libusb-1.0` and `pkg-config`, then run:

```sh
make validate
cargo test --doc
cargo build --release
cargo package
```

Run the hardware examples only on an authorized machine with a scanner
connected. Do not commit, log, or share biometric images or frame bytes.

## Releases

Use the detailed [release pipeline](docs/RELEASING.md) for version preparation,
tagging, release checks, and the manually confirmed crates.io publication.
