# futronic-fs80h-rs

Rust USB driver for Futronic FS80H and FS81H scanners. It owns discovery,
exclusive USB access, frame capture and scanner LEDs. Image formats, biometrics,
auto-capture and applications live in the `fs80hctl` workspace. The retired C
ABI, LFD research and NFIQ2 are preserved in `archive/ffi-lfd-research`.
