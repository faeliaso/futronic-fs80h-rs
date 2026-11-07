# futronic-fs80h-rs

Driver USB em espaço de usuário, escrito em Rust, para os leitores ópticos de impressão digital Futronic FS80H e FS81H. Captura imagens em escala de cinza sem compressão e não depende da ScanAPI proprietária da Futronic.

[English documentation](README_EN.md)

## Compatibilidade

São suportados os dispositivos Futronic com vendor ID `0x1491` e product IDs `0x0020` (FS80/FS80H) e `0x0025` (FS81H). Cada captura retorna uma imagem linear de 8 bits, 320×480 pixels e 500 DPI.

O driver comunica-se com o scanner por `libusb-1.0`, usando o protocolo Bulk OUT/IN recuperado do SDK do fornecedor. Não há garantia de certificação governamental, ANSI, ISO ou de detecção de vivacidade.

## Instalação Rust

```toml
[dependencies]
futronic-fs80h-rs = { path = "../futronic-fs80h-rs" }
```

O `rusb` requer `libusb-1.0` no sistema:

- macOS: `brew install libusb pkg-config`
- Debian/Ubuntu: `sudo apt-get install libusb-1.0-0-dev pkg-config`

No Linux, conceda acesso USB à aplicação. Por exemplo, use uma regra udev com `SUBSYSTEM=="usb", ATTR{idVendor}=="1491", MODE="0660", GROUP="plugdev"`, recarregue as regras e reconecte o dispositivo.

## Captura básica

```rust,no_run
use futronic_fs80h_rs::FutronicDevice;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut scanner = FutronicDevice::open_first()?;
let frame = scanner.capture_frame()?;
println!("{}x{} a {} DPI", frame.width(), frame.height(), frame.dpi());
# Ok(())
# }
```

Também estão disponíveis detecção de presença, auto-capture, WSQ, templates ISO/IEC 19794-2 e ANSI INCITS 378, matcher local, extração de minutiae e MCC. Consulte o [README em inglês](README_EN.md) para a referência funcional completa e os exemplos disponíveis.

## Integração nativa C

Além do crate Rust, é possível criar integrações próprias em C/C++, Swift, .NET, Python ou outro runtime através da ABI C v1. Gere uma biblioteca dinâmica local com:

```sh
cargo build --release --features native
./scripts/package-native.sh
```

O pacote em `dist/` contém a biblioteca, `libusb`, header, documentação e licenças. Consulte:

- [Header C](include/futronic_fs80h.h)
- [Guia de integração em português](docs/integracao-nativa.md)
- [Native integration guide in English](docs/native-integration.md)
- [Exemplo C](examples/native_smoke.c)

A ABI é síncrona, usa handles opacos e exige que todo `fs80h_buffer` retornado seja liberado com `fs80h_buffer_free`.

## Recursos experimentais

As APIs de pesquisa LFD são exclusivas do FS80H e não habilitam a Traditional Live Finger Detection proprietária, não retornam um score de vivacidade e não devem ser usadas como proteção anti-spoof. NFIQ2 também é opcional e requer instalação local da biblioteca, OpenCV e do modelo NIST; esses componentes não são distribuídos neste repositório.

## Licença

Este projeto é licenciado sob a [licença MIT](LICENSE). O código WSQ derivado do NBIS possui a licença em [c_src/LICENSE-NIST](c_src/LICENSE-NIST).
