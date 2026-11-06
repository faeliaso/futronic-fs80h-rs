//! Captura ao vivo (ou carregamento de amostra), extração de minúcias e matching de alta performance com Bit-based MCC e Bozorth3.

use futronic_fs80h_rs::biometric_core::bozorth3;
use futronic_fs80h_rs::biometric_core::mcc::{GalleryIndex, MccMatcher, MccTemplate};
use futronic_fs80h_rs::{biometric_core, FutronicDevice, RawFrame};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Pipeline Biométrico de Alta Performance: Minutia Cylinder-Code (MCC) ===");

    // 1. Tenta capturar do scanner USB; se não houver dispositivo plugado, lê imagem de amostra
    let frame = match FutronicDevice::open_first() {
        Ok(mut dev) => {
            println!("Scanner detectado: {:?}", dev.info());
            println!("Pouse o dedo sobre o prisma óptico e pressione Enter para capturar...");
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            dev.capture_frame()?
        }
        Err(e) => {
            println!("Nenhum scanner USB conectado ({e}). Utilizando imagem de amostra...");
            let sample_path = "capture_original_black.png";
            let bytes = std::fs::read(sample_path)
                .map_err(|_| format!("Arquivo de amostra '{sample_path}' não encontrado."))?;
            let img = image::load_from_memory(&bytes)?.to_luma8();
            RawFrame::new(img.into_raw())?
        }
    };

    println!("Dimensões da imagem: {}x{}", frame.width(), frame.height());

    // 2. Extração das minúcias via pipeline de Visão Computacional
    let start_ext = Instant::now();
    let minutiae = biometric_core::extract_minutiae(&frame)?;
    let ext_time = start_ext.elapsed();
    println!(
        "Extração concluída: {} minúcias identificadas em {:?}",
        minutiae.len(),
        ext_time
    );

    // 3. Codificação do template MCC 3D (Bit-based)
    let start_enc = Instant::now();
    let probe_template = MccTemplate::from_minutiae(&minutiae, frame.width(), frame.height());
    let enc_time = start_enc.elapsed();
    println!(
        "Codificação MCC concluída: {} cilindros 3D gerados em {:?} ({} bytes)",
        probe_template.len(),
        enc_time,
        probe_template.len() * 192
    );

    // 4. Comparativo de Matching 1:1: NIST BOZORTH3 vs. Bit-based MCC
    println!("\n--- Comparativo 1:1 (Auto-Verificação do Mesmo Dedo) ---");
    let b3_score = bozorth3::match_score(&minutiae, &minutiae);
    println!("• NIST BOZORTH3 score : {b3_score} (Limiar de match típico >= 40)");

    let start_match = Instant::now();
    let mcc_res = MccMatcher::new().match_pair(&probe_template, &probe_template);
    let match_time = start_match.elapsed();
    println!(
        "• Bit-based MCC score : {:.4} ({} pares casados de K={}) em {:?}",
        mcc_res.score, mcc_res.matched_pairs, mcc_res.k, match_time
    );

    // 5. Simulação de Busca 1:N de Alta Escala com Rayon
    println!("\n--- Busca Concorrente 1:N com Rayon ---");
    let gallery_size = 500;
    println!("Construindo galeria em memória com {gallery_size} templates...");

    let mut gallery = GalleryIndex::new();
    for id in 1..=gallery_size {
        // Insere templates com perturbações simuladas
        let permuted_minutiae: Vec<_> = minutiae
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let mut p = *m;
                if id != 42 {
                    // Outros registros com offsets aleatórios
                    p.x = ((p.x as f32 + (i as f32 * 3.1 + id as f32).sin() * 25.0).max(10.0) as u32).min(310);
                    p.y = ((p.y as f32 + (i as f32 * 2.7 + id as f32).cos() * 25.0).max(10.0) as u32).min(470);
                }
                p
            })
            .collect();
        let tmpl = MccTemplate::from_minutiae(&permuted_minutiae, frame.width(), frame.height());
        gallery.insert(id, tmpl);
    }

    let start_search = Instant::now();
    let best = gallery.search_best(&probe_template);
    let search_time = start_search.elapsed();

    if let Some((best_id, best_score)) = best {
        println!(
            "Resultado da busca 1:{gallery_size}: Melhor correspondência ID #{best_id} com score {:.4} (Tempo total: {:?}, média por template: {:?})",
            best_score,
            search_time,
            search_time / gallery_size as u32
        );
        assert_eq!(best_id, 42, "O candidato id 42 deveria ser o match perfeito!");
    }

    println!("\nPipeline executado com sucesso!");
    Ok(())
}
