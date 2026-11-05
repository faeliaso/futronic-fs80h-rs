//! Capture the FS80H auxiliary triplet with recovered colored-CMOS registers.
//!
//! This writes device-specific CMOS registers and is for isolated test readers only.
//!
//! `cargo run --features lfd-research --example capture_lfd_colored_cmos -- --apply-verified-cmos output-dir`

use futronic_fs80h_rs::FutronicDevice;
use std::{env, fs, path::PathBuf};

const ACKNOWLEDGEMENT: &str = "--apply-verified-cmos";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    if arguments.next().as_deref() != Some(ACKNOWLEDGEMENT.as_ref()) {
        return Err(usage().into());
    }
    let output = arguments.next().map(PathBuf::from).ok_or(usage())?;
    if arguments.next().is_some() {
        return Err(usage().into());
    }
    fs::create_dir_all(&output)?;

    let mut scanner = FutronicDevice::open_first()?;
    println!(
        "Applying recovered CMOS profile (8-byte framing), capturing the auxiliary triplet, then restoring 40 4A 03."
    );
    let triplet = scanner.capture_lfd_colored_cmos_triplet()?;
    for (selector, stream) in [
        (0x84_u8, triplet.selector_84()),
        (0x81, triplet.selector_81()),
        (0x01, triplet.selector_01()),
    ] {
        fs::write(output.join(format!("selector-{selector:02x}.raw")), stream)?;
    }
    let metrics = triplet.metrics();
    fs::write(
        output.join("metadata.txt"),
        format!(
            "profile_source=firmware_query\nrestore_command=40 4A 03 00 00 00 00 00\ndark_subtracted_correlation={:.6}\nhigh_signal_fraction={:.6}\n",
            metrics.dark_subtracted_correlation, metrics.high_signal_fraction
        ),
    )?;
    println!("Wrote sensitive auxiliary streams to {}.", output.display());
    Ok(())
}

fn usage() -> &'static str {
    "usage: capture_lfd_colored_cmos --apply-verified-cmos <output-directory>"
}
