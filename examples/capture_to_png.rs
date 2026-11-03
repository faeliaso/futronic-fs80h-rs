use futronic_fs80h_rs::FutronicDevice;
use std::{io, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for a Futronic FS80H/FS81H scanner...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Connected to: {:?}", scanner.info());

    println!("Place a finger on the prism...");
    println!("Press Enter when ready to capture.");
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    let started = Instant::now();
    let frame = scanner.capture_frame()?;
    println!(
        "Captured {}x{} at {} DPI ({} bytes) in {:?}",
        frame.width(),
        frame.height(),
        frame.dpi(),
        frame.as_bytes().len(),
        started.elapsed()
    );

    let output = "fingerprint_calibrated.png";
    image::save_buffer(
        output,
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::L8,
    )?;
    println!("Wrote {output} (calibrated raw sensor frame, dose = 166)");

    let inverted = frame.inverted();
    let inverted_output = "fingerprint_inverted.png";
    image::save_buffer(
        inverted_output,
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        image::ColorType::L8,
    )?;
    println!("Wrote {inverted_output} (standard AFIS inverted view)");

    let normalized = inverted.background_normalized();
    let normalized_output = "fingerprint_normalized.png";
    image::save_buffer(
        normalized_output,
        normalized.as_bytes(),
        normalized.width(),
        normalized.height(),
        image::ColorType::L8,
    )?;
    println!("Wrote {normalized_output} (background-whitened & contrast-stretched view)");

    let enhanced = inverted.ridge_enhanced();
    let enhanced_output = "fingerprint_enhanced.png";
    image::save_buffer(
        enhanced_output,
        enhanced.as_bytes(),
        enhanced.width(),
        enhanced.height(),
        image::ColorType::L8,
    )?;
    println!(
        "Wrote {enhanced_output} (ridge-sharpened view for high-confidence minutiae extraction)"
    );

    Ok(())
}
