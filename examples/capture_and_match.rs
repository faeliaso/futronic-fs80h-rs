use fprint_bozorth3::{match_score, MIN_COMPUTABLE_BOZORTH_MINUTIAE};
use futronic_fs80h_rs::{biometric_core, FutronicDevice};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Pipeline Biométrico: Captura, Minúcias e Bozorth3 ===");

    // 1. Conexão e captura via driver USB
    let mut dev = FutronicDevice::open_first()?;
    println!("Scanner detectado: {:?}", dev.info());
    println!("Pouse o dedo sobre o prisma óptico e pressione Enter para capturar...");

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    let frame = dev.capture_frame()?;

    // 2. Persistência dos quadros originais (fundo preto / sensor e fundo branco / AFIS)
    let orig_black_path = "capture_original_black.png";
    image::save_buffer(
        orig_black_path,
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::L8,
    )?;
    println!(
        "1. Imagem original (fundo preto) gravada em '{}'",
        orig_black_path
    );

    let inverted = frame.inverted();
    let orig_white_path = "capture_original_white.png";
    image::save_buffer(
        orig_white_path,
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        image::ColorType::L8,
    )?;
    println!(
        "2. Imagem original (fundo branco) gravada em '{}'",
        orig_white_path
    );

    // 3. Extração algorítmica de minúcias
    let start_ext = Instant::now();
    let minutiae = biometric_core::extract_minutiae(&frame)?;
    println!(
        "\nExtração concluída: {} minúcias válidas identificadas em {:?}",
        minutiae.len(),
        start_ext.elapsed()
    );

    // 4. Renderização dos mapas de minúcias anotados (fundo preto e fundo branco)
    let annot_black_path = "capture_annotated_black.png";
    let annot_black_img = biometric_core::render_minutiae_overlay(&frame, &minutiae);
    annot_black_img.save(annot_black_path)?;
    println!(
        "3. Imagem anotada (fundo preto) gravada em '{}'",
        annot_black_path
    );

    let annot_white_path = "capture_annotated_white.png";
    let annot_white_img = biometric_core::render_minutiae_overlay_inverted(&frame, &minutiae);
    annot_white_img.save(annot_white_path)?;
    println!(
        "4. Imagem anotada (fundo branco) gravada em '{}'",
        annot_white_path
    );

    // 5. Verificação de matching 1:1 via fprint-bozorth3
    let bozorth_points: Vec<fprint_bozorth3::Minutia> = minutiae.iter().map(Into::into).collect();

    if bozorth_points.len() >= MIN_COMPUTABLE_BOZORTH_MINUTIAE {
        // Validação de sanidade: teste de auto-comparação (Probe vs Gallery idênticos)
        let score = match_score(&bozorth_points, &bozorth_points);
        println!("\n--- Resultado do Matching BOZORTH3 ---");
        println!("Pontuação de auto-similaridade: {}", score);
        println!(
            "Status: {}",
            if score >= 40 {
                "AUTENTICADO (Match)"
            } else {
                "REJEITADO"
            }
        );
    } else {
        eprintln!(
            "Aviso: Quantidade de minúcias ({}) abaixo do mínimo necessário ({}) para o Bozorth3.",
            bozorth_points.len(),
            MIN_COMPUTABLE_BOZORTH_MINUTIAE
        );
    }

    Ok(())
}
