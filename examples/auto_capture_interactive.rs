use futronic_fs80h_rs::{AutoCaptureOptions, CaptureFeedback, FutronicDevice};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Futronic FS80H/FS81H Auto-Capture Demo ===");
    println!("Procurando leitor Futronic na porta USB...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Conectado com sucesso: {:?}", scanner.info());
    println!();

    println!("-> Instrução: O leitor agora aguarda o posicionamento do dedo.");
    println!("-> O LED verde acenderá indicando que o leitor está ativo.");
    println!("-> Não é necessário pressionar nenhuma tecla!");
    println!();

    let options = AutoCaptureOptions {
        timeout: Duration::from_secs(15),
        visual_feedback: true,
        ..Default::default()
    };

    let captured_frame = scanner.wait_for_finger_present(&options, |event| {
        match event {
            CaptureFeedback::WaitingForFinger { elapsed } => {
                print!(
                    "\r[Status] Aguardando dedo no prisma... ({:.1}s)   ",
                    elapsed.as_secs_f32()
                );
                let _ = io::stdout().flush();
            }
            CaptureFeedback::FingerDetected {
                contact_ratio,
                stable_count,
                required_stable,
            } => {
                let percent = (contact_ratio * 100.0).clamp(0.0, 100.0);
                print!(
                    "\r[Status] Dedo detectado! Contato: {:.1}% [Estabilizando: {}/{}]   ",
                    percent, stable_count, required_stable
                );
                let _ = io::stdout().flush();
            }
            CaptureFeedback::CapturingFinal => {
                println!("\n[Status] Dedo estável! Disparando captura em alta resolução...");
            }
            CaptureFeedback::WaitingForRemoval { elapsed } => {
                print!(
                    "\r[Status] Aguardando remoção do dedo... ({:.1}s)   ",
                    elapsed.as_secs_f32()
                );
                let _ = io::stdout().flush();
            }
            CaptureFeedback::Completed => {
                println!("[Status] Captura finalizada com sucesso!");
            }
        }
        true // Retornar true para continuar a operação (false cancelaria)
    })?;

    let metrics = captured_frame.presence_metrics();
    println!();
    println!("--- Relatório da Captura ---");
    println!(
        "Resolução: {}x{} @ {} DPI",
        captured_frame.width(),
        captured_frame.height(),
        captured_frame.dpi()
    );
    println!(
        "Tamanho do buffer: {} bytes",
        captured_frame.as_bytes().len()
    );
    println!(
        "Área de contato biométrica: {:.2}% ({} blocos de 16x16)",
        metrics.contact_ratio * 100.0,
        metrics.contact_blocks
    );
    println!(
        "Contraste médio das cristas: {:.1}",
        metrics.mean_contact_contrast
    );

    let output_file = "auto_captured_fingerprint.png";
    let inverted = captured_frame.inverted();
    image::save_buffer(
        output_file,
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        image::ColorType::L8,
    )?;
    println!("Imagem salva em: {output_file}");

    Ok(())
}
