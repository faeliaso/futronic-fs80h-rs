# futronic-fs80h-rs

Rust USB driver for Futronic FS80H and FS81H optical scanners. The crate
discovers scanners, claims the USB interface, captures a 320×480 L8 frame at
500 DPI, and controls the status LEDs.

```rust,no_run
use futronic_fs80h_rs::FutronicDevice;

let mut scanner = FutronicDevice::open_first()?;
let frame = scanner.capture_frame()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

This repository is limited to hardware transport and acquisition. WSQ,
PNG/TIFF, templates, matching, auto-capture, quality analysis, and applications
belong to the `fs80hctl` workspace. The previous C ABI, LFD experiments, and
NFIQ2 are preserved in the `archive/ffi-lfd-research` branch.

## Installation

Add the published crate to an application with Cargo:

```sh
cargo add futronic-fs80h-rs
```

## Requirements

Building requires `libusb-1.0` and `pkg-config`. On Linux, grant the
application USB access to VID `1491`.

The MSRV is Rust 1.71.

## Compatibility

| Model | PID | Raw stream | Returned frame | Capture modes |
| --- | --- | --- | --- | --- |
| FS80H | `0020` | 320×800 | 320×480, 500 DPI | frame mode and variable dose |
| FS81H | `0025` | 384×800 | 320×480, 500 DPI | frame mode and variable dose |

The driver uses `libusb` on Linux and macOS. Automated checks cover
hardware-free behavior. FS80H was validated on macOS 27.2 in frame-mode and
variable-dose capture; run the
[hardware validation guide](docs/HARDWARE_VALIDATION.md) for the remaining OS,
model, and capture-mode combinations. Do not include biometric data in logs.

### Pending hardware validation

The following manual-validation coverage remains for a future hardware test
session:

- FS81H in frame-mode and variable-dose capture.
- Linux with each available model in both capture modes.

Record the operating system, crate version, observed geometry, capture mode,
result, and logs without biometric data in the hardware validation guide.

## Validation

```sh
make validate
cargo run --example list_devices
```

See [the release pipeline](docs/RELEASING.md) for the GitHub Actions gates
before publishing to crates.io.

## Contributing and releases

See [CONTRIBUTING.md](CONTRIBUTING.md) for the branch and pull-request model,
and [the release pipeline](docs/RELEASING.md) for versioning, tagging, and the
protected crates.io publishing workflow.
