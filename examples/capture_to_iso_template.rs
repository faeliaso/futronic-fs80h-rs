//! Capture one fingerprint and persist only its ISO/IEC 19794-2:2011 minutiae template.

use futronic_fs80h_rs::{FutronicDevice, IsoFormat, TemplateOptions};
use std::{fs, io};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for a Futronic FS80H/FS81H scanner...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Connected to: {:?}", scanner.info());
    println!("Place a finger on the prism and press Enter to capture.");
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;

    let frame = scanner.capture_frame()?;
    let options = TemplateOptions::default();
    let iso_2011 = frame.extract_iso_template(&options)?;
    let iso_2005 = frame.extract_iso_template_as(IsoFormat::Iso2005, &options)?;

    fs::write("fingerprint-2011.iso19794", iso_2011.as_bytes())?;
    println!(
        "Wrote fingerprint-2011.iso19794 ({} bytes, {} minutiae, mean quality {})",
        iso_2011.as_bytes().len(),
        iso_2011.minutiae().len(),
        iso_2011.mean_quality()
    );
    fs::write("fingerprint-2005.iso19794", iso_2005.as_bytes())?;
    println!(
        "Wrote fingerprint-2005.iso19794 ({} bytes, {} minutiae, mean quality {})",
        iso_2005.as_bytes().len(),
        iso_2005.minutiae().len(),
        iso_2005.mean_quality()
    );
    if !iso_2011.singular_points().is_empty() {
        println!(
            "Included {} experimental core/delta estimate(s) in both templates.",
            iso_2011.singular_points().len()
        );
    }
    Ok(())
}
