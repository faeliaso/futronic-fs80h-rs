# FS80H LFD research notes

The `lfd-research` feature exists to collect evidence for an independent,
native implementation. It is not an implementation of Futronic TLFD and does
not make a live/fake decision.

## Static-analysis evidence

The supplied Linux x64 `libScanAPI.so` exposes FS80H-specific symbols including:

- `CFsUsb20t80HDevice::SetOptions(unsigned int, unsigned int)`
- `CFsUsb20t80HDevice::GetFrame(void*, __FTRSCAN_FRAME_PARAMETERS*)`
- `CFsUsb20t80HDevice::CalculationDLFD(...)`
- `CHwLfdMethod::IsFakeFinger80H(...)`

It also contains diagnostic strings for a fake-image result. The Android demo
in the supplied materials enables `FTR_OPTIONS_DETECT_FAKE_FINGER` (`0x01`)
through `SetOptions` before calling `GetFrame`.

This establishes that the proprietary implementation consumes more than the
crate's exported, resampled 320x480 frame. It does not establish the
calibration constants or acceptance thresholds.

### Recovered acquisition shape

Static inspection of both supplied macOS and Linux SDK binaries now agrees on
the following FS80H-only high-level sequence when option bit `0x01` is set:

| Order | SDK action | Evidence |
| --- | --- | --- |
| 1 | Set the colored/backlight CMOS state on. | `WriteBL2CMOS(1)` / `WriteBL2CMOSE(1)` |
| 2 | Capture a low-dose auxiliary frame. | dose `0x2d` (45), selector `0x84` |
| 3 | Capture a second low-dose auxiliary frame. | dose `0x2d`, selector `0x81` |
| 4 | Capture a third low-dose auxiliary frame. | dose `0x2d`, selector `0x01` |
| 5 | Apply empty-frame subtraction and run `CLfdColored::TLFD_calculation`. | calls present in both binaries |
| 6 | Calculate an exposure from the auxiliary frame. | `Dosage_calculation` |
| 7 | Capture final frames using the calculated dose. | selectors `0x02` and `0x82` |
| 8 | Restore the colored/backlight CMOS state during cleanup. | `WriteBL2CMOS(0)` |

`SetOptions` itself does not contain direct USB transport calls in the Linux
binary; it stores the option state. `GetFrame` tests that state and selects the
sequence above. Therefore LFD is not merely a classifier applied to ordinary
`0x6a` captures.

The helper used for the auxiliary frames calls the common transport routine
with opcode `0x37`. Its serialized request begins with `0x37`, the dose, and
an 11-byte parameter block containing the selector, acquisition profile and
image geometry. That opcode and parameter layout are **inferred only**. This
crate intentionally does not send them: the exact profile and geometry tables,
register-write transport, device revision constraints, and recovery behavior
have not yet been validated on an isolated device.

The `LfdResearchAcquisitionPlan::inferred_fs80h_tlfd()` API exposes this
sequence as typed, non-executing Rust data. It is useful for reviewing the
recovered behavior and for later trace-based tests, without creating a path
that can accidentally send inferred vendor requests to a scanner.

### Colored-CMOS dry run

The vendor's `WriteBL2CMOS(1)` writes registers `0x4A` through `0x4F` before
the auxiliary frames. Static analysis of `CFsUsb20t80HDevice::WriteBL2CMOS` and
`CFs80CompatibleDevice::InternalSave7ControlBytes` in `libScanAPI.so` recovered the
transport framing, order, normal-state restore command, and activation byte mapping:
- Every register write is sent as an **8-byte USB bulk OUT frame**:
  `[0x40, register, value, 0x00, 0x00, 0x00, 0x00, 0x00]`. `0x40` is the opcode
  for saving control bytes/registers, and bytes 3..7 are zero padding.
- The normal-state restore command is likewise 8 bytes:
  `[0x40, 0x4A, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]`.
- The SDK reads the 64-byte firmware configuration block with `FC 40 00 00`, then
  maps offsets `0x19`, `0x11`, `0x14`, `0x13`, `0x12`, and `0x22` respectively
  to registers `0x4A`–`0x4F`. `0x4A` is transformed as `((config[0x19] * 2) & 0xF0)`.

The following command is intentionally offline: it does not enumerate a
scanner or use USB.

```sh
cargo run --features lfd-research --example probe_lfd_colored_cmos -- --dry-run
```

It prints those unresolved slots and always reports `safe_to_apply=false`.
No `--apply` mode exists until static analysis independently recovers all six
activation values and a restore-on-error design is validated.

The read-only firmware query is now available for an isolated FS80H test
reader:

```sh
cargo run --features lfd-research --example probe_lfd_colored_cmos -- \
  --read-firmware-config
```

It sends only `FC 40 00 00`, reads 64 bytes, and prints the derived register
pairs. It does not send any colored-CMOS activation register writes.

After verifying the printed profile for the connected test reader, an explicit
capture mode can apply it, collect the auxiliary triplet, and restore `4A 03`
even when capture fails:

```sh
cargo run --features lfd-research --example capture_lfd_colored_cmos -- \
  --apply-verified-cmos /private/tmp/lfd-colored-cmos
```

This is still experimental and stores sensitive raw streams. It must not be
used on a production reader or represented as a certified liveness decision.

## One-command hardware probe

For an isolated FS80H test reader, the `probe_lfd_auxiliary` example sends one
inferred auxiliary request and writes its raw response:

```sh
cargo run --features lfd-research --example probe_lfd_auxiliary -- \
  --send-unvalidated result.lfd-probe.raw
```

It transmits `37 2d 84 01 00 01 80 03 20 00 00 00 00` (opcode `0x37`, selector
`0x84`, profile `0`, geometry 384x800). The profile and geometry are a
test hypothesis, not a recovered per-device calibration. It does not enable
the unvalidated colored-CMOS register sequence, so a successful response does
not mean TLFD is active. If it fails, close the process and reconnect the
reader before normal use.

The captured triplet can be compared offline without a reader:

```sh
cargo run --features lfd-research --example analyze_lfd_auxiliary_triplet -- \
  path/to/triplet
```

The analysis subtracts `0x01` from both other channels using saturating pixel
subtraction, then reports corrected intensity, pixel-wise mean absolute
difference, and correlation. It additionally reports the same measures for
the corrected `0x81 >= 20` high-signal mask, reducing the influence of dark
background pixels. `20` is a transparent diagnostic setting, not a live/fake
threshold; callers can supply another value to
`analyze_lfd_auxiliary_frames_with_threshold` for experiments.

`FutronicDevice::capture_lfd_auxiliary_triplet(profile)` provides the same
three-request flow as one explicit experimental operation. Its assessment is
always `Indeterminate`: a live/fake result must not be fabricated from genuine
finger captures alone. Completing a classifier requires consented, labelled
captures for both bona-fide fingers and authorized presentation-attack media,
plus a held-out evaluation of false accept/reject rates.

## Opt-in experimental biometric-signal gate

The `lfd-experimental-decision` feature is disabled by default. It applies the
initial calibration threshold (`0.434`) to the full-frame dark-subtracted
correlation and reports either `biometric_signal_present` or
`insufficient_biometric_signal`:

```sh
cargo run --features lfd-experimental-decision --example evaluate_lfd_experimental -- \
  --enable-experimental-decision config/lfd-experimental.example.conf /private/tmp/lfd-evaluation
```

Copy the example policy per environment and change its versioned
`calibration_id` plus threshold after a new calibration. The standard evaluator
writes only `experimental-report.txt`; use the separate explicit probe example
when retention of sensitive RAW streams is authorized. This gate rejects the
observed four-layer-glove samples in the initial data set; it is not a live
finger or anti-spoof certification and must be recalibrated with future
labelled data.

## Collection protocol

1. Capture several sequences from the same consenting subject with the FS80H.
2. Keep the finger placement and pressure as stable as practical, then collect
   additional sequences with normal placement variation.
3. Record the scanner PID, control bytes, descriptor bytes, dose, sample timing,
   raw 384x800 streams, and reported metrics for every session.
   The default collector discards two warm-up captures before recording samples.
4. Never add the raw streams to Git. Treat them as sensitive biometric data.
5. Compare the resulting measurements only as diagnostics. Do not derive a
   production threshold from genuine-finger data alone.

## Next reverse-engineering milestones

1. Recover and validate, without a production reader attached, the geometry
   table and two-byte register-write transport used by `WriteBL2CMOS`.
2. Add a declarative, non-executing representation of the recovered sequence
   and unit-test its serialization against independently obtained USB traces.
3. Only after the preceding validation, add an opt-in *unsafe experimental*
   auxiliary-frame capture path with a guaranteed CMOS-state restore guard.
4. Reconstruct the required intermediate-frame geometry and deterministic
   features in Rust.
5. Add an explicit `Indeterminate` research result for any incomplete or
   unsupported acquisition sequence.
6. Only after authorized spoof samples and independently reviewed error rates,
   consider a separately named experimental classifier. Never label it as
   equivalent to Futronic TLFD without evidence.
