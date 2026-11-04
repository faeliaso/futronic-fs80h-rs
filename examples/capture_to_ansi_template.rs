//! Capture one fingerprint and persist ANSI INCITS 378 minutiae templates only.

use futronic_fs80h_rs::{AnsiFormat, FutronicDevice, TemplateOptions};
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
    for (format, path) in [
        (AnsiFormat::Ansi3782004, "fingerprint-ansi378-2004.fmr"),
        (AnsiFormat::Ansi3782009, "fingerprint-ansi378-2009.fmr"),
        (
            AnsiFormat::Ansi3782009Am1,
            "fingerprint-ansi378-2009-am1.fmr",
        ),
    ] {
        let template = frame.extract_ansi_template_as(format, &options)?;
        fs::write(path, template.as_bytes())?;
        println!(
            "Wrote {path} ({} bytes, {} minutiae, mean quality {})",
            template.as_bytes().len(),
            template.minutiae().len(),
            template.mean_quality()
        );
    }
    Ok(())
}
