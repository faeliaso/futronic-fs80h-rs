//! Interactively collect genuine/impostor scores and suggest a local decision threshold.
//!
//! Run with an optional CSV destination:
//! `cargo run --release --example calibrate_live_match -- calibration.csv`

use futronic_fs80h_rs::{FutronicDevice, Matcher, MatcherOptions, TemplateOptions};
use std::{
    env,
    fs::File,
    io::{self, Write},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleKind {
    Genuine,
    Impostor,
}

impl SampleKind {
    fn as_csv(self) -> &'static str {
        match self {
            Self::Genuine => "genuine",
            Self::Impostor => "impostor",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    kind: SampleKind,
    score: f32,
}

#[derive(Debug, Clone, Copy)]
struct ThresholdStats {
    threshold: f32,
    far: f32,
    frr: f32,
}

fn prompt(message: &str) -> io::Result<String> {
    print!("{message}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_ascii_lowercase())
}

fn recommend_threshold(samples: &[Sample]) -> Option<ThresholdStats> {
    let genuine: Vec<_> = samples
        .iter()
        .filter(|sample| sample.kind == SampleKind::Genuine)
        .map(|sample| sample.score)
        .collect();
    let impostor: Vec<_> = samples
        .iter()
        .filter(|sample| sample.kind == SampleKind::Impostor)
        .map(|sample| sample.score)
        .collect();
    if genuine.is_empty() || impostor.is_empty() {
        return None;
    }

    let mut thresholds: Vec<f32> = samples.iter().map(|sample| sample.score).collect();
    thresholds.sort_by(f32::total_cmp);
    thresholds.dedup_by(|left, right| left.total_cmp(right).is_eq());

    thresholds
        .into_iter()
        .map(|threshold| ThresholdStats {
            threshold,
            far: impostor.iter().filter(|&&score| score >= threshold).count() as f32
                / impostor.len() as f32,
            frr: genuine.iter().filter(|&&score| score < threshold).count() as f32
                / genuine.len() as f32,
        })
        .min_by(|left, right| {
            let left_error = left.far + left.frr;
            let right_error = right.far + right.frr;
            left_error
                .total_cmp(&right_error)
                .then_with(|| left.far.total_cmp(&right.far))
                .then_with(|| right.threshold.total_cmp(&left.threshold))
        })
}

fn print_summary(samples: &[Sample]) {
    let genuine: Vec<_> = samples
        .iter()
        .filter(|sample| sample.kind == SampleKind::Genuine)
        .map(|sample| sample.score)
        .collect();
    let impostor: Vec<_> = samples
        .iter()
        .filter(|sample| sample.kind == SampleKind::Impostor)
        .map(|sample| sample.score)
        .collect();

    println!(
        "Collected {} genuine and {} impostor samples.",
        genuine.len(),
        impostor.len()
    );
    if let (Some(genuine_min), Some(genuine_max)) = (
        genuine.iter().copied().reduce(f32::min),
        genuine.iter().copied().reduce(f32::max),
    ) {
        println!("Genuine score range: {genuine_min:.3}..={genuine_max:.3}");
    }
    if let (Some(impostor_min), Some(impostor_max)) = (
        impostor.iter().copied().reduce(f32::min),
        impostor.iter().copied().reduce(f32::max),
    ) {
        println!("Impostor score range: {impostor_min:.3}..={impostor_max:.3}");
    }
    match recommend_threshold(samples) {
        Some(stats) => println!(
            "Suggested threshold: {:.3} (FAR {:.1}%, FRR {:.1}%).",
            stats.threshold,
            stats.far * 100.0,
            stats.frr * 100.0
        ),
        None => println!("Capture at least one genuine and one impostor sample for a suggestion."),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut csv = match env::args().nth(1) {
        Some(path) => {
            let mut file = File::create(path)?;
            writeln!(file, "kind,score")?;
            Some(file)
        }
        None => None,
    };
    let matcher = Matcher::new(MatcherOptions { threshold: 0.0 })?;
    let options = TemplateOptions::default();
    let mut scanner = FutronicDevice::open_first()?;

    println!("Calibration records scores only; it does not choose a production security policy.");
    let _ = prompt("Place the enrollment finger on the scanner, then press Enter: ")?;
    let enrolled = scanner.capture_frame()?.extract_iso_template(&options)?;
    println!(
        "Enrollment captured: {} minutiae.",
        enrolled.minutiae().len()
    );

    let mut samples = Vec::new();
    loop {
        let action =
            prompt("[g] same enrolled finger, [i] different finger, [s] summary, [q] quit: ")?;
        let kind = match action.as_str() {
            "g" | "genuine" => SampleKind::Genuine,
            "i" | "impostor" => SampleKind::Impostor,
            "s" | "summary" => {
                print_summary(&samples);
                continue;
            }
            "q" | "quit" => break,
            _ => {
                println!("Choose g, i, s, or q.");
                continue;
            }
        };

        let instruction = match kind {
            SampleKind::Genuine => "Reposition the enrolled finger, then press Enter to capture: ",
            SampleKind::Impostor => "Place a different finger, then press Enter to capture: ",
        };
        let _ = prompt(instruction)?;
        match scanner.capture_frame()?.extract_iso_template(&options) {
            Ok(probe) => {
                let result = matcher.verify(&enrolled, &probe)?;
                println!(
                    "{} score={:.3}; matched_minutiae={}",
                    kind.as_csv(),
                    result.score,
                    result.matched_minutiae
                );
                let sample = Sample {
                    kind,
                    score: result.score,
                };
                if let Some(file) = &mut csv {
                    writeln!(file, "{},{:.6}", sample.kind.as_csv(), sample.score)?;
                    file.flush()?;
                }
                samples.push(sample);
                print_summary(&samples);
            }
            Err(error) => println!("Capture rejected; sample not recorded: {error}"),
        }
    }
    print_summary(&samples);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_separating_score_groups_has_no_observed_errors() {
        let samples = [
            Sample {
                kind: SampleKind::Genuine,
                score: 0.8,
            },
            Sample {
                kind: SampleKind::Genuine,
                score: 0.7,
            },
            Sample {
                kind: SampleKind::Impostor,
                score: 0.3,
            },
            Sample {
                kind: SampleKind::Impostor,
                score: 0.2,
            },
        ];
        let stats = recommend_threshold(&samples).unwrap();
        assert_eq!(stats.far, 0.0);
        assert_eq!(stats.frr, 0.0);
        assert!((0.3..=0.7).contains(&stats.threshold));
    }
}
