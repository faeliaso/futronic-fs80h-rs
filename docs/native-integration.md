# Native integration (C ABI)

Native packages expose C ABI version 1 in `include/futronic_fs80h.h`. They let consumers create their own C/C++, .NET P/Invoke, Python `ctypes`, Java JNI, Swift, or other runtime integration without linking against Rust.

## Contract

- Every operation returns `fs80h_status`. If the result is not `FS80H_OK`, call `fs80h_last_error_message()` immediately on the same thread.
- Handles (`fs80h_device`, `fs80h_frame`, `fs80h_matcher`, and `fs80h_mcc_template`) are opaque and must be released with their matching `*_free` function. Do not use a handle concurrently from multiple threads.
- An `fs80h_buffer` is owned by the library. Release it exactly once with `fs80h_buffer_free`; never pass it to `free`, `delete`, or a consumer-runtime allocator.
- The caller owns input buffers throughout a call. The library never persists fingerprint pixels, templates, or WSQ data automatically.
- Capture, auto-capture, WSQ, extraction, and matching are synchronous; invoke them off the UI thread.
- The experimental `fs80h_device_probe_lfd_auxiliary` and `fs80h_device_lfd_firmware_config` operations are FS80H research facilities, not liveness or anti-spoof controls.

## Minimal C example

```c
#include "futronic_fs80h.h"
#include <stdio.h>

int main(void) {
  fs80h_device *device = NULL;
  fs80h_frame *frame = NULL;
  fs80h_buffer wsq = {0};

  if (fs80h_device_open_first(&device) != FS80H_OK) goto fail;
  if (fs80h_device_capture(device, 0, &frame) != FS80H_OK) goto fail;
  if (fs80h_frame_to_wsq(frame, 0.75f, &wsq) != FS80H_OK) goto fail;
  printf("WSQ: %zu bytes\n", wsq.len);

  fs80h_buffer_free(&wsq);
  fs80h_frame_free(frame);
  fs80h_device_close(device);
  return 0;
fail:
  fprintf(stderr, "%s\n", fs80h_last_error_message());
  fs80h_buffer_free(&wsq);
  fs80h_frame_free(frame);
  fs80h_device_close(device);
  return 1;
}
```

Call `fs80h_list_devices(NULL, 0, &count)` before opening when multiple scanners might be connected. Allocate `count` `fs80h_device_info` elements, call it again, and use `bus_number` and `address` with `fs80h_device_open`.

## Package contents and loading

Each release contains the dynamic library, the `libusb` runtime, the C header, this guide, and licenses. Keep the FS80H library and `libusb` in the application bundle or another loader search path:

- Linux: an application rpath, `LD_LIBRARY_PATH`, or a system library directory.
- macOS: the packaged library references its colocated `libusb` through `@loader_path`.
- Windows: the executable directory or another directory enabled through the application's DLL search configuration.

On Linux, USB permissions are still required, such as a udev rule for vendor `1491`. Bundling `libusb` does not grant device permission.

## Local build

Install Rust, a C compiler, and the `libusb-1.0` development package. On macOS, install `libusb` and `pkg-config` using Homebrew. On Debian/Ubuntu install `libusb-1.0-0-dev`, `pkg-config`, and `patchelf`.

```sh
cargo build --release --features native
./scripts/package-native.sh
```

The local package is written to `dist/`. For the optional NFIQ2 variant, separately install NFIQ2 2.3, OpenCV, and the model; point `NFIQ2_DIR` at that installation and add the `nfiq2` feature. Those components are not redistributed by this project and need their own license review.

To compile the C example directly against a local artifact, link with `-lfutronic_fs80h_rs`.

## Compatibility

Check `fs80h_abi_version()` before calling API functions. ABI v1 preserves existing names, signatures, ownership rules, and `fs80h_status` values; incompatible changes require a new ABI revision.
