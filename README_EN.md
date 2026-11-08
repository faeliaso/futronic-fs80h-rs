# futronic-fs80h-rs

Rust USB driver for Futronic FS80H and FS81H scanners. It owns discovery,
exclusive USB access, frame capture and scanner LEDs. Image formats, biometrics,
auto-capture and applications live in the `fs80hctl` workspace. The retired C
ABI, LFD research and NFIQ2 are preserved in `archive/ffi-lfd-research`.

The build requires `libusb-1.0` and `pkg-config`; the MSRV is Rust 1.70.

## Compatibility

| Model | PID | Raw stream | Returned frame | Capture modes |
| --- | --- | --- | --- | --- |
| FS80H | `0020` | 320×800 | 320×480, 500 DPI | frame mode and variable dose |
| FS81H | `0025` | 384×800 | 320×480, 500 DPI | frame mode and variable dose |

The driver uses `libusb` on Linux and macOS. Automated checks cover
hardware-free behavior; run the [hardware validation guide](docs/HARDWARE_VALIDATION.md)
for each available OS, model and capture mode. Do not include biometric data in
logs.

## Validation

```sh
make validate
cargo run --example list_devices
```
