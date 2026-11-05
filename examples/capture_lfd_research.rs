//! Collect raw FS80H streams for LFD research without making a liveness decision.
//!
//! `cargo run --features lfd-research --example capture_lfd_research -- lfd-research`

use futronic_fs80h_rs::{FutronicDevice, LfdResearchOptions, LfdResearchOutcome};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: capture_lfd_research <output-directory>")?;
    fs::create_dir_all(&output)?;

    let mut scanner = FutronicDevice::open_first()?;
    println!("Place and hold one finger on the FS80H scanner.");
    match scanner.capture_lfd_research_sequence(LfdResearchOptions::default())? {
        LfdResearchOutcome::Unsupported { product_id } => {
            eprintln!("LFD research is unsupported for product ID {product_id:#06X}.");
        }
        LfdResearchOutcome::InsufficientSamples { requested } => {
            eprintln!("Need at least two samples; requested {requested}.");
        }
        LfdResearchOutcome::Acquired(sequence) => {
            for (index, sample) in sequence.samples().iter().enumerate() {
                let path = output.join(format!("sample-{index:02}.raw"));
                fs::write(path, sample.raw_sensor_stream())?;
            }
            let metrics = sequence.metrics();
            let sample_elapsed_millis: Vec<u128> = sequence
                .samples()
                .iter()
                .map(|sample| sample.elapsed().as_millis())
                .collect();
            let sample_timings_millis: Vec<String> = sequence
                .samples()
                .iter()
                .map(|sample| {
                    let timing = sample.capture_timing();
                    format!(
                        "configure_gain={} sensor_settle={} flush_bulk_in={} trigger_frame={} first_packet_after_trigger={} read_stream={} restore_gain={} total={}",
                        timing.configure_gain.as_millis(),
                        timing.sensor_settle.as_millis(),
                        timing.flush_bulk_in.as_millis(),
                        timing.trigger_frame.as_millis(),
                        timing.first_packet_after_trigger.as_millis(),
                        timing.read_stream.as_millis(),
                        timing.restore_gain.as_millis(),
                        timing.total.as_millis(),
                    )
                })
                .collect();
            let report = format!(
                "product_id={:#06X}\ndose={}\nwarmup_count={}\nsample_count={}\nsample_elapsed_millis={sample_elapsed_millis:?}\nsample_timings_millis={sample_timings_millis:#?}\ncontrol_bytes={:?}\ndescriptor_bytes={:?}\nmean_intensity={:?}\nstddev={:?}\nadjacent_mad={:?}\n",
                sequence.device().product_id,
                sequence.dose(),
                sequence.warmup_count(),
                sequence.samples().len(),
                sequence.control_bytes(),
                sequence.descriptor_bytes(),
                metrics.mean_intensity,
                metrics.intensity_standard_deviation,
                metrics.adjacent_mean_absolute_difference,
            );
            fs::write(output.join("metadata.txt"), report)?;
            println!(
                "Wrote sensitive raw streams and diagnostics to {}.",
                output.display()
            );
        }
    }
    Ok(())
}
