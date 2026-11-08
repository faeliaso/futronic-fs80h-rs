# Changelog

All notable changes to this project are documented in this file.

## Unreleased

## 0.1.1 - 2026-11-08

### Fixed

- Restored 384-column physical CMOS sensor line width and 307,200-byte stream reading for FS80H devices, fixing the 64-pixel row offset and diagonal distortion.

### Added

- Contributor and release-process documentation for the protected GitHub
  Actions publication workflow.

## 0.1.0 - 2026-11-08

### Added

- Tests for complete, short and empty frame-read chunks, partial read timeouts,
  and incomplete streams.
- Conversion coverage for the FS80H (320×800) and FS81H (384×800) raw sensor
  streams, plus predictable bilinear-resampling cases.
- Hardware-free USB capture-interface selection tests.
- Tests ensuring a frame-mode restore failure never hides the original capture
  failure.
- A `make validate` routine and CI checks for formatting, Clippy and tests.
- A manually confirmed, tag-only crates.io publish workflow protected by the
  `crates-io` GitHub Environment.

### Changed

- Frame-mode restoration now emits a warning when it fails after a failed
  capture, while preserving the capture error returned to the caller.
- The crate declares Rust 1.71 as its minimum supported Rust version (MSRV).
