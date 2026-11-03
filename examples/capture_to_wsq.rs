use futronic_fs80h_rs::{FutronicDevice, RawFrame};
use std::{fs, io, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for a Futronic FS80H/FS81H scanner...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Connected to: {:?}", scanner.info());

    println!("Place a finger on the prism...");
    println!("Press Enter when ready to capture and compress to WSQ.");
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;

    let started = Instant::now();
    let frame = scanner.capture_frame()?;
    let capture_time = started.elapsed();

    let raw_bytes = frame.as_bytes();
    let raw_len = raw_bytes.len();

    // 1. Save original view to PNG for visual reference
    let png_output = "fingerprint_original.png";
    image::save_buffer(
        png_output,
        frame.inverted().as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::L8,
    )?;

    // 2. Compress to FBI-compliant WSQ format (bitrate = 0.75 bpp ~ 15:1 target ratio)
    let wsq_start = Instant::now();
    let wsq_bytes = frame.to_wsq(Some(RawFrame::DEFAULT_WSQ_BITRATE))?;
    let wsq_time = wsq_start.elapsed();

    let wsq_output = "fingerprint.wsq";
    fs::write(wsq_output, &wsq_bytes)?;

    let ratio = raw_len as f64 / wsq_bytes.len() as f64;
    let reduction = (1.0 - (wsq_bytes.len() as f64 / raw_len as f64)) * 100.0;

    println!("\n=== WSQ Compression Results ===");
    println!("Capture time:        {:?}", capture_time);
    println!("WSQ encode time:     {:?}", wsq_time);
    println!("Raw sensor frame:    {} bytes (320x480 @ 500 DPI)", raw_len);
    println!("FBI WSQ output:      {} bytes", wsq_bytes.len());
    println!(
        "Compression ratio:   {:.2}:1 ({:.1}% reduction)",
        ratio, reduction
    );
    println!("Wrote {wsq_output} (FBI compliant WSQ stream)");
    println!("Wrote {png_output} (AFIS inverted PNG)");

    // 3. Decompress the WSQ file back into a RawFrame to verify roundtrip fidelity
    let decode_start = Instant::now();
    let reconstructed = RawFrame::from_wsq(&wsq_bytes)?;
    let decode_time = decode_start.elapsed();

    let recon_output = "fingerprint_wsq_reconstructed.png";
    image::save_buffer(
        recon_output,
        reconstructed.inverted().as_bytes(),
        reconstructed.width(),
        reconstructed.height(),
        image::ColorType::L8,
    )?;

    println!("\n=== WSQ Decompression Verification ===");
    println!("WSQ decode time:     {:?}", decode_time);
    println!(
        "Reconstructed frame: {}x{} at {} DPI",
        reconstructed.width(),
        reconstructed.height(),
        reconstructed.dpi()
    );
    println!("Wrote {recon_output} (view to verify zero block artifacts)");

    Ok(())
}
