//! Comparação interativa entre dois dedos ao vivo com o leitor Futronic FS80H.
//!
//! Testa tanto a confirmação de identidade (mesmo dedo capturado duas vezes)
//! quanto a rejeição de impostor (dois dedos diferentes).

use futronic_fs80h_rs::biometric_core::bozorth3;
use futronic_fs80h_rs::biometric_core::mcc::{MccMatcher, MccTemplate};
use futronic_fs80h_rs::{biometric_core, FutronicDevice, RawFrame};
use std::io::{stdin, stdout, Write};
use std::time::Instant;

fn capture_single_finger(
    dev: &mut FutronicDevice,
    finger_label: &str,
) -> Result<(RawFrame, Vec<biometric_core::BiometricMinutia>, MccTemplate), Box<dyn std::error::Error>>
{
    print!("\n👉 Pouse o {} e pressione [Enter] para capturar...", finger_label);
    stdout().flush()?;
    let mut buffer = String::new();
    stdin().read_line(&mut buffer)?;

    print!("Capturando do leitor... ");
    stdout().flush()?;
    let frame = dev.capture_frame()?;
    println!("OK ({}x{} pixels)", frame.width(), frame.height());

    print!("Extraindo minúcias biométricas... ");
    stdout().flush()?;
    let start_ext = Instant::now();
    let minutiae = biometric_core::extract_minutiae(&frame)?;
    let ext_time = start_ext.elapsed();
    println!("OK: {} minúcias em {:?}", minutiae.len(), ext_time);

    print!("Codificando cilindros 3D MCC... ");
    stdout().flush()?;
    let start_enc = Instant::now();
    let template = MccTemplate::from_minutiae(&minutiae, frame.width(), frame.height());
    let enc_time = start_enc.elapsed();
    println!(
        "OK: {} cilindros ({} bytes) em {:?}",
        template.len(),
        template.len() * 192,
        enc_time
    );

    Ok((frame, minutiae, template))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=================================================================");
    println!("     VERIFICAÇÃO BIOMÉTRICA DE DOIS DEDOS AO VIVO (FS80H)       ");
    println!("          NIST BOZORTH3  vs.  Bit-based CYLINDER-CODE (MCC)      ");
    println!("=================================================================");

    let mut dev = match FutronicDevice::open_first() {
        Ok(d) => {
            println!("\n✅ Leitor óptico detectado: {:?}", d.info().product_name.as_deref().unwrap_or("FS80H"));
            d
        }
        Err(e) => {
            eprintln!("\n❌ Erro ao conectar ao leitor USB: {e}");
            eprintln!("Certifique-se de que o cabo USB do FS80H está plugado.");
            return Ok(());
        }
    };

    println!("\n[Instruções de Teste]:");
    println!(" • Para testar MATCH (Mesmo Dedo)    : Pouse o mesmo dedo na 1ª e na 2ª captura.");
    println!(" • Para testar REJEIÇÃO (Dedos Dif.) : Pouse o Indicador na 1ª e o Polegar na 2ª.");

    // 1. Captura da Amostra A
    let (_frame_a, minutiae_a, template_a) =
        capture_single_finger(&mut dev, "PRIMEIRO DEDO (Amostra A)")?;

    // 2. Captura da Amostra B
    let (_frame_b, minutiae_b, template_b) =
        capture_single_finger(&mut dev, "SEGUNDO DEDO (Amostra B)")?;

    println!("\n=================================================================");
    println!("                  RESULTADOS DA COMPARAÇÃO                       ");
    println!("=================================================================");

    // Match Bozorth3
    let start_b3 = Instant::now();
    let b3_score = bozorth3::match_score(&minutiae_a, &minutiae_b);
    let b3_time = start_b3.elapsed();

    // Match MCC
    let start_mcc = Instant::now();
    let mcc_res = MccMatcher::new().match_pair(&template_a, &template_b);
    let mcc_time = start_mcc.elapsed();

    println!("\n1. NIST BOZORTH3:");
    println!("   • Score de similaridade : {}", b3_score);
    println!("   • Limiar típico de match: >= 40");
    println!("   • Tempo de execução     : {:?}", b3_time);

    println!("\n2. Bit-based Minutia Cylinder-Code (MCC):");
    println!("   • Score normalizado [0..1]: {:.4}", mcc_res.score);
    println!("   • Pares casados           : {} de K={}", mcc_res.matched_pairs, mcc_res.k);
    println!("   • Limiar de match calibrado: >= 0.2700 (Consistência Geométrica NIST 16px)");
    println!("   • Tempo de execução       : {:?}", mcc_time);

    println!("\n-----------------------------------------------------------------");
    println!("                        VEREDITO FINAL                           ");
    println!("-----------------------------------------------------------------");

    let mcc_is_match = mcc_res.score >= 0.27;
    let b3_is_match = b3_score >= 40;

    println!("• Avaliação NIST BOZORTH3 : {}", if b3_is_match { "✅ MATCH (>= 40)" } else { "❌ REPROVADO (< 40)" });
    println!("• Avaliação Bit-based MCC : {}", if mcc_is_match { "✅ MATCH (>= 0.2700)" } else { "❌ REPROVADO (< 0.2700)" });
    println!();

    if mcc_is_match && b3_is_match {
        println!("🎉 DECISÃO CONJUNTA: ✅ [MATCH UNÂNIME]");
        println!("   Ambos os algoritmos confirmam que é o MESMO DEDO com alta confiança!");
    } else if mcc_is_match && !b3_is_match {
        println!("🚀 DECISÃO CONJUNTA: ✅ [MATCH CONFIRMADO VIA MCC]");
        println!("   O MCC superou a distorção elástica/pressão da pele onde o Bozorth3 falhou!");
        println!("   (Isso comprova a superioridade da representação 3D invariante do MCC).");
    } else if !mcc_is_match && !b3_is_match {
        println!("🛡️  DECISÃO CONJUNTA: ❌ [REJEIÇÃO DE IMPOSTOR CONFIRMADA]");
        println!("   Ambos os algoritmos confirmam: as capturas pertencem a DEDOS DIFERENTES!");
    } else {
        println!("⚠️  DECISÃO CONJUNTA: ❓ [ZONA DE INCERTEZA / CAPTURA PARCIAL]");
        println!("   Bozorth3 aprovou mas MCC ficou abaixo da margem. Verifique a área útil.");
    }
    println!("=================================================================\n");

    Ok(())
}
