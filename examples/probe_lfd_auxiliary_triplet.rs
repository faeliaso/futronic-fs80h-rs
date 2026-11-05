//! Capture the three inferred FS80H auxiliary frames for comparative research.
//!
//! This does not activate TLFD or the colored-CMOS register sequence.
//!
//! `cargo run --features lfd-research --example probe_lfd_auxiliary_triplet -- --send-unvalidated output-dir`

use futronic_fs80h_rs::{assess_lfd_auxiliary_triplet, FutronicDevice, LfdResearchAssessment};
use std::{env, fs, path::PathBuf};

const ACKNOWLEDGEMENT: &str = "--send-unvalidated";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let acknowledgement = arguments.next().ok_or(usage())?;
    if acknowledgement != ACKNOWLEDGEMENT {
        return Err(usage().into());
    }
    let output = arguments.next().map(PathBuf::from).ok_or(usage())?;
    if arguments.next().is_some() {
        return Err(usage().into());
    }
    fs::create_dir_all(&output)?;

    let mut scanner = FutronicDevice::open_first()?;
    println!("Sending inferred unvalidated auxiliary-frame triplet.");
    let triplet = scanner.capture_lfd_auxiliary_triplet(0)?;
    let mut report = String::from("profile=0\ngeometry=384x800\n");
    for (selector, stream) in [
        (0x84_u8, triplet.selector_84()),
        (0x81, triplet.selector_81()),
        (0x01, triplet.selector_01()),
    ] {
        let mean = stream.iter().map(|&value| f64::from(value)).sum::<f64>() / stream.len() as f64;
        fs::write(output.join(format!("selector-{selector:02x}.raw")), stream)?;
        report.push_str(&format!(
            "selector={selector:#04X} bytes={} mean={mean:.4}\n",
            stream.len()
        ));
    }
    let metrics = triplet.metrics();
    let assessment = match assess_lfd_auxiliary_triplet(&triplet) {
        LfdResearchAssessment::Indeterminate => "indeterminate",
    };
    report.push_str(&format!(
        "assessment={assessment}\ndark_subtracted_84_mean={:.4}\ndark_subtracted_81_mean={:.4}\ndark_subtracted_mad={:.4}\ndark_subtracted_correlation={:.6}\nhigh_signal_pixel_count={}\nhigh_signal_fraction={:.6}\nhigh_signal_84_mean={:.4}\nhigh_signal_81_mean={:.4}\nhigh_signal_mad={:.4}\nhigh_signal_correlation={:.6}\n",
        metrics.dark_subtracted_84_mean,
        metrics.dark_subtracted_81_mean,
        metrics.dark_subtracted_mean_absolute_difference,
        metrics.dark_subtracted_correlation,
        metrics.high_signal_pixel_count,
        metrics.high_signal_fraction,
        metrics.high_signal_84_mean,
        metrics.high_signal_81_mean,
        metrics.high_signal_mean_absolute_difference,
        metrics.high_signal_correlation,
    ));
    fs::write(output.join("metadata.txt"), report)?;
    println!("Wrote sensitive auxiliary streams to {}.", output.display());
    Ok(())
}

fn usage() -> &'static str {
    "usage: probe_lfd_auxiliary_triplet --send-unvalidated <output-directory>"
}
