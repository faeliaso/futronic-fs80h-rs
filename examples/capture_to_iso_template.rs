//! Capture one fingerprint and persist only its ISO/IEC 19794-2:2011 minutiae template.

use futronic_fs80h_rs::{FutronicDevice, TemplateOptions};
use std::{fs, io};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for a Futronic FS80H/FS81H scanner...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Connected to: {:?}", scanner.info());
    println!("Place a finger on the prism and press Enter to capture.");
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;

    let frame = scanner.capture_frame()?;
    let template = frame.extract_iso_template(&TemplateOptions::default())?;
    fs::write("fingerprint.iso19794", template.as_bytes())?;
    println!(
        "Wrote fingerprint.iso19794 ({} bytes, {} minutiae, mean quality {})",
        template.as_bytes().len(),
        template.minutiae().len(),
        template.mean_quality()
    );
    if !template.singular_points().is_empty() {
        println!(
            "Included {} experimental core/delta estimate(s).",
            template.singular_points().len()
        );
    }
    Ok(())
}
