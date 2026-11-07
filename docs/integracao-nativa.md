# Integração nativa (ABI C)

Os pacotes nativos expõem a ABI C versão 1 em `include/futronic_fs80h.h`. Eles permitem criar integrações próprias para C/C++, .NET P/Invoke, Python `ctypes`, Java JNI ou outro runtime sem ligar contra Rust.

## Contrato

- Todas as operações retornam `fs80h_status`; se o resultado não for `FS80H_OK`, consulte `fs80h_last_error_message()` imediatamente na mesma thread.
- Handles (`fs80h_device`, `fs80h_frame`, `fs80h_matcher` e `fs80h_mcc_template`) são opacos e devem ser liberados pela função `*_free` correspondente. Um handle não pode ser usado simultaneamente por mais de uma thread.
- `fs80h_buffer` pertence à biblioteca. Libere-o uma única vez com `fs80h_buffer_free`; não use `free`, `delete` ou desalocadores do runtime consumidor.
- O chamador conserva a propriedade de buffers de entrada durante a chamada. A biblioteca nunca persiste pixels, templates ou WSQ automaticamente.
- Captura, auto-capture, WSQ, extração e matching são síncronos. Execute-os fora da thread de interface da aplicação.
- As funções `fs80h_device_probe_lfd_auxiliary` e `fs80h_device_lfd_firmware_config` são estritamente recursos de pesquisa FS80H; não retornam nem estabelecem liveness.

## Primeiro uso em C

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
  printf("WSQ: %zu bytes\\n", wsq.len);

  fs80h_buffer_free(&wsq);
  fs80h_frame_free(frame);
  fs80h_device_close(device);
  return 0;
fail:
  fprintf(stderr, "%s\\n", fs80h_last_error_message());
  fs80h_buffer_free(&wsq);
  fs80h_frame_free(frame);
  fs80h_device_close(device);
  return 1;
}
```

Use `fs80h_list_devices(NULL, 0, &count)` antes de abrir quando houver mais de um leitor. Aloque `count` elementos de `fs80h_device_info` e execute a função novamente para obter os IDs `bus_number` e `address` usados por `fs80h_device_open`.

## Arquivos e carregamento

Cada release contém a biblioteca, a dependência runtime do `libusb`, o header, este guia e licenças. Mantenha a biblioteca do FS80H e a biblioteca `libusb` no mesmo diretório da aplicação ou em um caminho do carregador do sistema:

- Linux: `LD_LIBRARY_PATH`, rpath da aplicação ou diretório padrão do sistema.
- macOS: `@loader_path`/rpath no bundle da aplicação.
- Windows: diretório do executável ou diretório registrado no mecanismo de DLL search da aplicação.

Em Linux, o processo ainda precisa de permissão ao USB, por exemplo uma regra udev para vendor `1491`. A inclusão de `libusb` no pacote não concede essa permissão.

## Compilação local

Instale Rust, um compilador C e `libusb-1.0` de desenvolvimento. Em macOS, instale `libusb` e `pkg-config` via Homebrew; em Debian/Ubuntu, instale `libusb-1.0-0-dev`, `pkg-config` e `patchelf`.

```sh
cargo build --release --features native,lfd-experimental-decision
./scripts/package-native.sh
```

O pacote local fica em `dist/`. Para a variante NFIQ2, instale NFIQ2 2.3, OpenCV e o modelo separadamente, aponte `NFIQ2_DIR` à instalação e inclua `nfiq2` nas features. Esses componentes não são redistribuídos pelo projeto e requerem análise de licença própria.

## Compatibilidade

Confira `fs80h_abi_version()` antes de chamar funções. A ABI v1 preserva nomes, assinaturas, ownership e valores existentes de `fs80h_status`; mudanças incompatíveis exigirão uma nova versão ABI. As APIs LFD são experimentais e não devem ser usadas como controle anti-spoof.
