//! Send exactly one unvalidated FS80H auxiliary-frame request for trace research.
//!
//! This does not enable TLFD and must only be used on a test reader.
//!
//! `cargo run --features lfd-research --example probe_lfd_auxiliary -- --send-unvalidated output.raw`

use futronic_fs80h_rs::FutronicDevice;
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

    let mut scanner = FutronicDevice::open_first()?;
    println!(
        "Sending one unvalidated auxiliary-frame probe: opcode 0x37, selector 0x84, profile 0, 384x800."
    );
    let stream = scanner.probe_lfd_auxiliary_frame(0x84, 0, 384, 800)?;
    fs::write(&output, stream)?;
    println!("Wrote the raw probe response to {}.", output.display());
    Ok(())
}

fn usage() -> &'static str {
    "usage: probe_lfd_auxiliary --send-unvalidated <output.raw>"
}
