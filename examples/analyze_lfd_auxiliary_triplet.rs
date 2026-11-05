//! Analyze an already captured FS80H auxiliary-frame triplet offline.
//!
//! `cargo run --features lfd-research --example analyze_lfd_auxiliary_triplet -- path/to/triplet`

use futronic_fs80h_rs::analyze_lfd_auxiliary_frames;
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: analyze_lfd_auxiliary_triplet <directory>")?;
    let selector_84 = fs::read(directory.join("selector-84.raw"))?;
    let selector_81 = fs::read(directory.join("selector-81.raw"))?;
    let selector_01 = fs::read(directory.join("selector-01.raw"))?;
    let metrics = analyze_lfd_auxiliary_frames(&selector_84, &selector_81, &selector_01)?;

    println!("selector_84_mean={:.4}", metrics.selector_84_mean);
    println!("selector_81_mean={:.4}", metrics.selector_81_mean);
    println!("selector_01_mean={:.4}", metrics.selector_01_mean);
    println!(
        "dark_subtracted_84_mean={:.4}",
        metrics.dark_subtracted_84_mean
    );
    println!(
        "dark_subtracted_81_mean={:.4}",
        metrics.dark_subtracted_81_mean
    );
    println!(
        "dark_subtracted_mad={:.4}",
        metrics.dark_subtracted_mean_absolute_difference
    );
    println!(
        "dark_subtracted_correlation={:.6}",
        metrics.dark_subtracted_correlation
    );
    println!(
        "high_signal_pixel_count={}",
        metrics.high_signal_pixel_count
    );
    println!("high_signal_fraction={:.6}", metrics.high_signal_fraction);
    println!("high_signal_84_mean={:.4}", metrics.high_signal_84_mean);
    println!("high_signal_81_mean={:.4}", metrics.high_signal_81_mean);
    println!(
        "high_signal_mad={:.4}",
        metrics.high_signal_mean_absolute_difference
    );
    println!(
        "high_signal_correlation={:.6}",
        metrics.high_signal_correlation
    );
    Ok(())
}
