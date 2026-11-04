# futronic-fs80h-rs

Pure Rust userspace USB capture for Futronic FS80H/FS81H optical fingerprint
scanners. The crate communicates through `libusb-1.0` using the internal
`rusb` dependency; it does not link to Futronic's proprietary ScanAPI.

## Status and compatibility

The intended supported identifiers are vendor `0x1491`, products `0x0020`
(FS80/FS80H) and `0x0025` (FS81H). Frames are returned as a complete, linear,
8-bit grayscale image of 320×480 pixels at 500 DPI.

The scanner has a vendor-specific Bulk OUT/IN protocol. Its descriptor exposes
Bulk OUT `0x01` and Bulk IN `0x82`. The image request (`0x6A`, exposure/dose)
was recovered through static analysis of the vendor SDK and validated on
physical hardware. The optional binary green/red diode control is available
through `FutronicDevice::set_diodes`; it is intentionally not enabled during a
normal capture because its optical role differs across hardware revisions.

## Install

```toml
[dependencies]
futronic-fs80h-rs = { path = "../futronic-fs80h-rs" }
```

`rusb` requires a system `libusb-1.0` installation:

- macOS: `brew install libusb pkg-config`
- Debian/Ubuntu: `sudo apt-get install libusb-1.0-0-dev pkg-config`

On Linux, grant the application access to the USB device, for example with a
udev rule containing `SUBSYSTEM=="usb", ATTR{idVendor}=="1491", MODE="0660",
GROUP="plugdev"`, then reload udev rules and reconnect the scanner.

## Usage

```no_run
use futronic_fs80h_rs::FutronicDevice;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut scanner = FutronicDevice::open_first()?;
let frame = scanner.capture_frame()?;
assert_eq!(frame.as_bytes().len(), 320 * 480);
# Ok(())
# }
```

The captured [`RawFrame`] provides direct methods for FBI-compliant WSQ compression
and decompression:

```no_run
# use futronic_fs80h_rs::{FutronicDevice, RawFrame};
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let mut scanner = FutronicDevice::open_first()?;
# let frame = scanner.capture_frame()?;
// Compress the 153,600-byte frame to FBI WSQ format (~12 KB, 15:1 ratio)
let wsq_bytes = frame.to_wsq(Some(RawFrame::DEFAULT_WSQ_BITRATE))?;

// Decompress WSQ bytes back to a calibrated 320x480 RawFrame
let reconstructed = RawFrame::from_wsq(&wsq_bytes)?;
assert_eq!(reconstructed.as_bytes().len(), 320 * 480);
# Ok(())
# }
```

## Hardware validation

With a connected scanner and suitable USB permission, run:

```sh
cargo run --example capture_to_png
```

The expected result is `fingerprint_calibrated.png`, an 8-bit 320×480 image. Its USB
interface is released when the program exits, including after a capture error.
The default capture uses the calibrated dose measured on the official vendor SDK
(`166` / `0xA6`), with an analog gain setup and optical settling time.

The example writes four views of the biometric capture:
- `fingerprint_calibrated.png`: Direct calibrated optical sensor frame (dose 166).
- `fingerprint_inverted.png`: Inverted grayscale intensities (`255 - pixel`), matching standard AFIS and `FTRSDK_Coca` views (dark papillary ridges on light background).
- `fingerprint_normalized.png`: Non-contact glass prism background whitened to pure `#FFFFFF` with bezel shadows suppressed and ridge contrast expanded.
- `fingerprint_enhanced.png`: Biometric ridge-sharpened view for high-confidence minutiae extraction and pore detection.

To capture, compress directly to FBI WSQ, and verify reconstructed fidelity:

```sh
cargo run --example capture_to_wsq
```

To capture and store only an ISO/IEC 19794-2:2011 minutiae template (no image file):

```sh
cargo run --example capture_to_iso_template
```

This writes `fingerprint-2011.iso19794` and `fingerprint-2005.iso19794`, containing the same
quality-filtered ridge endings and bifurcations in both interoperable layouts.
It may also include experimental core/delta estimates when the orientation field is sufficiently
confident. The template is still sensitive biometric data and should be protected by the consuming
application.

The default export is ISO/IEC 19794-2:2011. For legacy systems, call
`frame.extract_iso_template_as(IsoFormat::Iso2005, &options)` to emit
ISO/IEC 19794-2:2005 + Cor.1:2009. `IsoTemplate::from_bytes` detects and validates either
normal/off-card ISO revision automatically.

To inspect connected Futronic USB devices and endpoint descriptors without capturing:

```sh
cargo run --example inspect_usb
```

To capture with the green status diode enabled:

```sh
cargo run --example capture_with_green_diode
```

## Development checks

```sh
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
cargo doc --no-deps
```

Licensed under the [MIT license](LICENSE).
