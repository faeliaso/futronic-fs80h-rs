//! Opt-in experimental biometric-signal gate for FS80H LFD research.
//!
//! This is not a live/fake decision and must only be used on a test scanner.
//!
//! `cargo run --features lfd-experimental-decision --example evaluate_lfd_experimental -- --enable-experimental-decision policy.conf output-dir`

use futronic_fs80h_rs::{FutronicDevice, LfdExperimentalDecision, LfdExperimentalPolicy};
use std::{env, fs, path::PathBuf};

const ACKNOWLEDGEMENT: &str = "--enable-experimental-decision";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let acknowledgement = arguments.next().ok_or(usage())?;
    if acknowledgement != ACKNOWLEDGEMENT {
        return Err(usage().into());
    }
    let policy_path = arguments.next().map(PathBuf::from).ok_or(usage())?;
    let output = arguments.next().map(PathBuf::from).ok_or(usage())?;
    if arguments.next().is_some() {
        return Err(usage().into());
    }
    fs::create_dir_all(&output)?;
    let policy: LfdExperimentalPolicy = fs::read_to_string(&policy_path)?.parse()?;

    let mut scanner = FutronicDevice::open_first()?;
    let evaluation = scanner.evaluate_lfd_experimental(&policy)?;
    let result = match evaluation.decision {
        LfdExperimentalDecision::BiometricSignalPresent { .. } => "biometric_signal_present",
        LfdExperimentalDecision::InsufficientBiometricSignal { .. } => {
            "insufficient_biometric_signal"
        }
    };
    let metrics = evaluation.metrics;
    let report = format!(
        "experimental_result={result}\ncalibration_id={}\nminimum_correlation={:.6}\nobserved_correlation={:.6}\nhigh_signal_fraction={:.6}\nproduct_id={:#06X}\n",
        evaluation.calibration_id,
        policy.minimum_dark_subtracted_correlation,
        metrics.dark_subtracted_correlation,
        metrics.high_signal_fraction,
        evaluation.device.product_id,
    );
    fs::write(output.join("experimental-report.txt"), report)?;
    println!(
        "Experimental result: {result} (calibration {}, correlation {:.6}, threshold {:.6}).",
        evaluation.calibration_id,
        metrics.dark_subtracted_correlation,
        policy.minimum_dark_subtracted_correlation,
    );
    Ok(())
}

fn usage() -> &'static str {
    "usage: evaluate_lfd_experimental --enable-experimental-decision <policy.conf> <output-directory>"
}
