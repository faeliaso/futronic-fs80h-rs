# Roteiro de validação com hardware

Execute este roteiro para cada leitor disponível. Não inclua imagens, bytes de
frame, hashes de imagens ou qualquer dado biométrico no relatório ou nos logs.

## Preparação

```sh
make validate
cargo run --example list_devices
```

O segundo comando deve listar o dispositivo com o VID `1491` e PID `0020`
(FS80H) ou `0025` (FS81H). Se retornar uma lista vazia, confira o cabo, a
alimentação, a permissão de acesso USB e se o processo de teste está no mesmo
host que recebeu o dispositivo.

## Roteiro por modelo

1. Registre SO, versão do crate, modelo e identificador USB fornecidos pelo
   comando de descoberta.
2. Com apenas um leitor conectado, execute uma captura no modo padrão e outra
   no modo variável:

   ```sh
   cargo run --example capture_metadata
   cargo run --example capture_metadata -- --variable-dose
   ```

   Os exemplos não gravam imagens ou bytes de frame: eles mostram apenas
   metadados de descoberta e de geometria.
3. Confirme somente a geometria esperada (320×480, 500 DPI), o modo e o
   sucesso ou erro.
4. Registre mensagens de erro sem anexar dados biométricos.

## Registro mínimo

| Campo | Valor |
| --- | --- |
| Data/hora | |
| SO e versão | |
| Versão do crate | |
| Modelo e PID | |
| Modo (padrão/variável) | |
| Geometria observada | |
| Resultado | |
| Logs sem dados biométricos | |

## Resultados registrados

| Data | SO | Crate | Modelo/PID | Modo | Geometria | Resultado | Log sem biometria |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-11-08 | macOS 27.2 (26B123) | 0.3.0 | FS80H / `0020` | dose variável (`166`) | 320×480, 500 DPI | sucesso | `capture succeeded: model=FS80H pid=0020 geometry=320x480 dpi=500 mode=variable-dose` |
| 2026-11-08 | macOS 27.2 (26B123) | 0.3.0 | FS80H / `0020` | frame mode | 320×480, 500 DPI | sucesso | `capture succeeded: model=FS80H pid=0020 geometry=320x480 dpi=500 mode=frame-mode` |
