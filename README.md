# futronic-fs80h-rs

Driver USB Rust para os leitores ópticos Futronic FS80H e FS81H. O crate
descobre leitores, reclama a interface USB, captura um frame L8 de 320×480 a
500 DPI e controla os diodos de estado.

```rust,no_run
use futronic_fs80h_rs::FutronicDevice;

let mut scanner = FutronicDevice::open_first()?;
let frame = scanner.capture_frame()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

O escopo deste repositório é exclusivamente transporte e aquisição do
hardware. WSQ, PNG/TIFF, templates, matching, auto-captura, qualidade e
aplicações pertencem ao workspace `fs80hctl`. A antiga ABI C, experimentos LFD
e NFIQ2 foram preservados na branch `archive/ffi-lfd-research`.

## Dependências

O build exige `libusb-1.0` e `pkg-config`. Em Linux, conceda à aplicação
permissão de acesso USB ao VID `1491`.

## Verificação

```sh
cargo check
cargo test --lib --tests
```
