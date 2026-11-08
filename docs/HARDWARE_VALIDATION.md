# Hardware validation guide

Run this guide for each available scanner. Do not include images, frame bytes,
image hashes, or any biometric data in reports or logs.

## Preparation

```sh
make validate
cargo run --example list_devices
```

The second command must list a device with VID `1491` and PID `0020` (FS80H)
or `0025` (FS81H). If it returns an empty list, check the cable, power, USB
access permissions, and whether the test process is running on the host that
received the device.

## Per-model procedure

1. Record the OS, crate version, model, and USB identifier returned by the
   discovery command.
2. With exactly one scanner connected, capture once in the default mode and
   once with variable dose:

   ```sh
   cargo run --example capture_metadata
   cargo run --example capture_metadata -- --variable-dose
   ```

   The examples do not write images or frame bytes; they output discovery and
   geometry metadata only.
3. Confirm only the expected geometry (320×480, 500 DPI), mode, and success or
   failure.
4. Record error messages without attaching biometric data.

## Minimum record

| Field | Value |
| --- | --- |
| Timestamp | |
| OS and version | |
| Crate version | |
| Model and PID | |
| Mode (default/variable) | |
| Observed geometry | |
| Result | |
| Logs without biometric data | |

## Recorded results

| Date | OS | Crate | Model/PID | Mode | Geometry | Result | Log without biometrics |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-11-08 | macOS 27.2 (26B123) | 0.3.0 | FS80H / `0020` | variable dose (`166`) | 320×480, 500 DPI | success | `capture succeeded: model=FS80H pid=0020 geometry=320x480 dpi=500 mode=variable-dose` |
| 2026-11-08 | macOS 27.2 (26B123) | 0.3.0 | FS80H / `0020` | frame mode | 320×480, 500 DPI | success | `capture succeeded: model=FS80H pid=0020 geometry=320x480 dpi=500 mode=frame-mode` |
