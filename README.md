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

ANSI INCITS 378 templates are also available in the default feature set. ANSI 378 and ISO
19794-2 are distinct binary formats even where they share the `FMR` signature and revision
markers, so use the dedicated ANSI APIs rather than renaming an ISO file:

```no_run
# use futronic_fs80h_rs::{FutronicDevice, AnsiFormat, TemplateOptions};
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let mut scanner = FutronicDevice::open_first()?;
# let frame = scanner.capture_frame()?;
let template = frame.extract_ansi_template_as(
    AnsiFormat::Ansi3782009Am1,
    &TemplateOptions::default(),
)?;
let parsed = futronic_fs80h_rs::AnsiTemplate::from_bytes(template.as_bytes())?;
assert_eq!(parsed.format(), AnsiFormat::Ansi3782009Am1);
# Ok(())
# }
```

Run `cargo run --example capture_to_ansi_template` to create one ANSI 378-2004, 2009, and
2009/AM1 file from a capture. The implementation emits a strict, one-fingerprint live/plain
profile at 500 DPI with the standard `Vendor Unknown` identifier (`0x0103`); it is not a claim
of ANSI, government, or partner-system certification. Import rejects multi-fingerprint records,
vendor extensions, and unsupported ridge-count extensions.

## NIST NFIQ2 image quality

The optional `nfiq2` feature scores the uncompressed live frame with the official NIST NFIQ2 2.3
library before a template is created or stored. NFIQ2 produces a local 0–100 image-utility score
and actionable image-quality signals; it does not perform liveness detection or establish the
physical cause of a bad image. In particular, applications may translate insufficient foreground
to “reposition the finger”, but must not claim that it proves insufficient pressure.

NFIQ2 is not enabled by default because its native library, OpenCV dependency, and model are
installed separately. Install the NIST NFIQ2 2.3 development package, set `NFIQ2_DIR` to its
installation prefix (containing `include/` and `lib/`), and pass the path to its model-information
file when constructing the assessor. The model-info file identifies the random-forest parameter
file and checksum used for reproducible scoring.

```no_run
use futronic_fs80h_rs::{FutronicDevice, Nfiq2, Nfiq2Options};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let assessor = Nfiq2::new("/opt/nfiq2/model/model.info", Nfiq2Options::default())?;
let mut scanner = FutronicDevice::open_first()?;
let frame = scanner.capture_frame()?;
let quality = frame.assess_nfiq2(&assessor)?;

if !quality.accepted {
    // `feedback` can be empty: a low unified score does not always map to one of NFIQ2's four
    // actionable conditions. Translate it when present, then request another capture either way.
    eprintln!("Capture rejected (score {}): {:?}", quality.score, quality.feedback);
    return Ok(());
}
// Only persist or extract a template from accepted frames.
# Ok(())
# }
```

The default inclusive minimum score is `50`; set `Nfiq2Options::minimum_score` to recalibrate it
for the deployment. The library never writes the fingerprint frame to disk and does not transmit
it. Run the connected-device example with:

```sh
NFIQ2_DIR=/usr/local/nfiq2 cargo run --features nfiq2 --example capture_with_nfiq2_quality -- /path/to/model.info
```

## Local AFIS matching

The default features include a local minutiae matcher for 1:1 verification and linear 1:N search.
It compares parsed ISO/IEC 19794-2 and ANSI INCITS 378 templates directly; it can also extract a
probe template from a captured `RawFrame` or WSQ image. No image or template is sent to a service.

```no_run
use futronic_fs80h_rs::{FutronicDevice, Matcher, MatcherOptions, TemplateOptions};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let matcher = Matcher::new(MatcherOptions { threshold: 0.285 })?;
let mut scanner = FutronicDevice::open_first()?;
let live = scanner.capture_frame()?.extract_iso_template(&TemplateOptions::default())?;

// `enrolled` can be an IsoTemplate or an AnsiTemplate loaded from protected storage.
# let enrolled = live.clone();
let result = matcher.verify(&enrolled, &live)?;
println!("score={:.3}, matched={}", result.score, result.is_match);
# Ok(())
# }
```

`Matcher::identify` receives application-owned `(id, &template)` pairs and returns matching
candidates in descending score order. Base64 is available through explicit methods
(`verify_iso_base64`, `verify_ansi_base64`, and `verify_wsq_base64`) so byte formats are never
guessed. Scores and thresholds require calibration on representative data; this is not liveness
detection and does not provide a universal FAR/FRR guarantee. The `0.285` value in the example
comes from the local scanner test data; recalibrate it for every deployment.

With a connected scanner, a live 1:1 smoke test captures the same finger twice:

```sh
cargo run --release --example verify_live_match
```

To collect repeated scores and obtain a data-driven test threshold, run:

```sh
cargo run --release --example calibrate_live_match -- calibration.csv
```

Use `g` for repeated captures of the enrolled finger and `i` for a different finger. The optional
CSV file is written incrementally as `kind,score`; the example reports its observed false-accept
rate (FAR), false-reject rate (FRR), and the threshold with the lowest combined observed error.

To inspect connected Futronic USB devices and endpoint descriptors without capturing:

```sh
cargo run --example inspect_usb
```

To capture with the green status diode enabled:

```sh
cargo run --example capture_with_green_diode
```

## Experimental LFD research

The `lfd-research` feature is an FS80H-only data-collection aid for researching
the vendor's proprietary Traditional Live Finger Detection (TLFD). It does not
enable the proprietary LFD mode, return a liveness score, or accept/reject a
finger as live. In particular, it must not be used as an anti-spoof control.

It records a short, timestamped sequence of unresampled 384x800 CMOS streams
using the recovered frame-mode acquisition path, the known control/descriptor
responses, host-side timing for every capture phase, and descriptive per-stream
intensity and adjacent-frame-difference measurements. FS81H is reported as
unsupported because the model has no manufacturer LFD capability.

The default performs two warm-up captures without retaining their streams, then
records four samples. This reduces gain/illumination settling from contaminating
the retained sequence; it is not a liveness check.

```sh
cargo run --features lfd-research --example capture_lfd_research -- lfd-research
```

The output directory contains raw fingerprint streams and is ignored by Git.
These files are sensitive biometric data: obtain subject consent, use encrypted
storage where appropriate, and delete experimental captures when no longer
needed.

The recovered colored-CMOS setup used by the vendor's TLFD can be reviewed
offline without opening a scanner:

```sh
cargo run --features lfd-research --example probe_lfd_colored_cmos -- --dry-run
```

It documents the inferred register order but does not transmit it: the
device-specific activation bytes remain unresolved and the project has no
colored-CMOS apply mode yet.

To query and print the device-specific values without writing CMOS registers:

```sh
cargo run --features lfd-research \
  --example probe_lfd_colored_cmos -- --read-firmware-config
```

For an isolated test reader only, the verified profile can be applied for one
auxiliary triplet; the driver restores `4A 03` before returning from either a
successful or failed capture:

```sh
cargo run --features lfd-research \
  --example capture_lfd_colored_cmos -- \
  --apply-verified-cmos /private/tmp/lfd-colored-cmos
```

See [`docs/lfd-research.md`](docs/lfd-research.md) for the static-analysis
evidence and the next investigation milestones.

### Opt-in experimental biometric-signal gate

The separate `lfd-experimental-decision` feature adds an FS80H-only,
dataset-specific gate over the recovered three-channel auxiliary capture. It
reports `biometric_signal_present` or `insufficient_biometric_signal` from a
versioned external calibration policy. It is disabled by default, does not
claim to implement Futronic TLFD, and is not a certified live-finger or
anti-spoof control.

The included initial policy was fitted only to the project's genuine-finger and
four-layer-medical-glove research captures. Copy and revalidate it for every
environment; do not use its threshold as a general fraud-detection setting.

```sh
cargo run --features lfd-experimental-decision \
  --example evaluate_lfd_experimental -- \
  --enable-experimental-decision \
  config/lfd-experimental.example.conf \
  /private/tmp/lfd-evaluation
```

The evaluator writes only `experimental-report.txt`, containing the calibration
identifier, scanner product ID, threshold, measured correlation, and result.
It does not retain raw biometric streams. Use the explicitly named probe
examples only when raw capture has been authorized and secured.

## Development checks

```sh
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --doc
cargo doc --no-deps
```

Licensed under the [MIT license](LICENSE).
