# Changelog

All notable changes to this project are documented in this file.

## Unreleased

### Added

- Tests for complete, short and empty frame-read chunks, partial read timeouts,
  and incomplete streams.
- Conversion coverage for the FS80H (320×800) and FS81H (384×800) raw sensor
  streams, plus predictable bilinear-resampling cases.
- Hardware-free USB capture-interface selection tests.
- Tests ensuring a frame-mode restore failure never hides the original capture
  failure.
- A `make validate` routine and CI checks for formatting, Clippy and tests.

### Changed

- Frame-mode restoration now emits a warning when it fails after a failed
  capture, while preserving the capture error returned to the caller.
- The crate declares Rust 1.71 as its minimum supported Rust version (MSRV).
