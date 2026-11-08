# Roteiro de validação com hardware

Execute este roteiro para cada leitor disponível. Não inclua imagens, bytes de
frame, hashes de imagens ou qualquer dado biométrico no relatório ou nos logs.

## Preparação

```sh
make validate
cd ../fs80hctl
cargo run -p fs80h-cli -- --json devices
```

O segundo comando deve listar o dispositivo com o VID `1491` e PID `0020`
(FS80H) ou `0025` (FS81H). Se retornar uma lista vazia, confira o cabo, a
alimentação, a permissão de acesso USB e se o processo de teste está no mesmo
host que recebeu o dispositivo.

## Roteiro por modelo

1. Registre SO, versão do crate, modelo, bus e address fornecidos pelo comando
   de descoberta.
2. Abra o dispositivo sem capturar: `cargo run -p fs80h-cli -- device inspect
   --bus <bus> --address <address>`.
3. Execute uma captura no modo padrão e outra no modo variável:

   ```sh
   cargo run -p fs80h-cli -- capture --bus <bus> --address <address> --output /tmp/fs80h-check.png
   cargo run -p fs80h-cli -- capture --bus <bus> --address <address> --variable-dose --dose 166 --output /tmp/fs80h-variable.png
   ```

4. Confirme somente a geometria esperada (320×480, 500 DPI), o modo e o
   sucesso ou erro. Remova imediatamente os PNGs e seus manifestos se não
   forem necessários para um procedimento autorizado de retenção.
5. Registre mensagens de erro sem anexar dados biométricos.

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
