use base64::{engine::general_purpose::STANDARD, Engine as _};
use futronic_fs80h_rs::FutronicDevice;
use std::fs;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Futronic FS80H/FS81H - Captura para Base64 e PNG ===");
    println!("Conectando ao leitor...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Dispositivo detectado: {:?}", scanner.info());
    println!();

    println!("-> Posicione o dedo no leitor para captura automática (timeout: 10s)...");
    let frame = scanner.wait_for_finger_and_capture(Duration::from_secs(10))?;
    println!("Dedo capturado com sucesso!");
    println!(
        "Dimensões: {}x{} @ {} DPI",
        frame.width(),
        frame.height(),
        frame.dpi()
    );
    println!();

    // 1. Salva a imagem em PNG com cristas escuras e fundo claro
    let png_path = "fingerprint.png";
    let inverted = frame.inverted();
    image::save_buffer(
        png_path,
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        image::ColorType::L8,
    )?;

    // 2. Lê os bytes do arquivo PNG gerado e converte para Base64
    let png_bytes = fs::read(png_path)?;
    let png_base64 = STANDARD.encode(&png_bytes);

    let txt_path = "fingerprint_base64.txt";
    fs::write(txt_path, &png_base64)?;

    println!("Arquivos gerados com sucesso:");
    println!(
        "  1. PNG da digital:  {png_path} ({} bytes)",
        png_bytes.len()
    );
    println!(
        "  2. TXT com Base64:  {txt_path} ({} caracteres)",
        png_base64.len()
    );
    println!();
    println!(
        "Preview do Base64 (primeiros 70 caracteres): {}...",
        &png_base64[..70.min(png_base64.len())]
    );

    // 3. Validação de integridade do Base64 (roundtrip test)
    let decoded_bytes = STANDARD.decode(&png_base64)?;
    assert_eq!(
        decoded_bytes, png_bytes,
        "Os bytes decodificados do Base64 devem ser idênticos aos do arquivo PNG original"
    );
    println!("Validação de integridade: Base64 decodifica com 100% de fidelidade!");

    // 4. Se a compressão WSQ estiver habilitada, gera também o Base64 do WSQ (padrão de APIs governamentais)
    #[cfg(feature = "wsq")]
    {
        let wsq_bytes = frame.to_wsq(None)?;
        let wsq_base64 = STANDARD.encode(&wsq_bytes);
        let wsq_txt_path = "fingerprint_wsq_base64.txt";
        fs::write(wsq_txt_path, &wsq_base64)?;
        println!();
        println!("Bônus FBI WSQ (ideal para APIs com limitação de payload):");
        println!(
            "  - Arquivo WSQ Base64: {wsq_txt_path} (apenas {} bytes, ~15:1 compressão)",
            wsq_base64.len()
        );
    }

    println!();
    println!("Dica: você pode abrir o '{png_path}' para inspecionar visualmente a digital,");
    println!("ou utilizar o conteúdo de '{txt_path}' em tags HTML: <img src=\"data:image/png;base64,...\" />");

    Ok(())
}
