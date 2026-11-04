//! Capture the same finger twice and compare the extracted templates locally.

use futronic_fs80h_rs::{FutronicDevice, Matcher, MatcherOptions, TemplateOptions};
use std::io::{self, Write};

fn wait_for_enter(prompt: &str) -> io::Result<()> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Calibrated from the local live-test data set. Recalibrate before using this value
    // with a different device, population, enrollment policy, or security requirement.
    let matcher = Matcher::new(MatcherOptions { threshold: 0.285 })?;
    let options = TemplateOptions::default();
    let mut scanner = FutronicDevice::open_first()?;

    wait_for_enter("Place the enrollment finger on the scanner, then press Enter: ")?;
    let enrolled = scanner.capture_frame()?.extract_iso_template(&options)?;
    println!(
        "Enrollment captured: {} minutiae.",
        enrolled.minutiae().len()
    );

    wait_for_enter("Lift, reposition the same finger, then press Enter to verify: ")?;
    let live = scanner.capture_frame()?.extract_iso_template(&options)?;
    let result = matcher.verify(&enrolled, &live)?;

    println!(
        "score={:.3}; matched={}; matched_minutiae={}",
        result.score, result.is_match, result.matched_minutiae
    );
    Ok(())
}
