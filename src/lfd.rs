//! Experimental data collection primitives for researching FS80H live-finger detection.
//!
//! This module does **not** implement, claim, or emulate Futronic's proprietary
//! TLFD decision. It retains a timed sequence of raw CMOS streams and calculates
//! reproducible, descriptive statistics that can be compared during research.
//! It is intentionally behind the `lfd-research` feature.

use crate::{protocol::FS80_PID, DeviceInfo, RAW_SENSOR_STREAM_SIZE};
use std::time::Duration;
use thiserror::Error;

/// Static, non-executing description of the FS80H TLFD capture flow inferred
/// from the supplied vendor SDKs.
///
/// This plan is deliberately separate from [`LfdResearchOptions`]. It neither
/// sends USB commands nor decides whether a finger is live. In particular, the
/// per-device geometry/profile table required to issue the auxiliary requests
/// has not been independently validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfdResearchAcquisitionPlan {
    steps: Vec<LfdResearchCaptureStep>,
}

impl LfdResearchAcquisitionPlan {
    /// Returns the ordered TLFD flow inferred consistently from the macOS and
    /// Linux vendor binaries.
    #[must_use]
    pub fn inferred_fs80h_tlfd() -> Self {
        Self {
            steps: vec![
                LfdResearchCaptureStep::EnableColoredCmos,
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x84,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x81,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x01,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::CalculateTlfdFeatures,
                LfdResearchCaptureStep::CalculateFinalDose,
                LfdResearchCaptureStep::FinalFrame { selector: 0x02 },
                LfdResearchCaptureStep::FinalFrame { selector: 0x82 },
                LfdResearchCaptureStep::RestoreColoredCmos,
            ],
        }
    }

    /// Returns the steps in their required order.
    #[must_use]
    pub fn steps(&self) -> &[LfdResearchCaptureStep] {
        &self.steps
    }
}

/// One action in an inferred TLFD acquisition plan.
///
/// The actions are evidence for research and test planning; they are not an
/// authorization to transmit their corresponding vendor commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfdResearchCaptureStep {
    EnableColoredCmos,
    AuxiliaryFrame { selector: u8, dose: u8 },
    CalculateTlfdFeatures,
    CalculateFinalDose,
    FinalFrame { selector: u8 },
    RestoreColoredCmos,
}

/// Low auxiliary-frame dose observed in both inspected vendor implementations.
pub const PREVIEW_LFD_DOSE: u8 = 0x2d;

/// Dataset-specific default chosen from the initial ten genuine and ten
/// four-layer-glove captures. It must be revalidated before each deployment.
#[cfg(feature = "lfd-experimental-decision")]
pub const DEFAULT_EXPERIMENTAL_MIN_CORRELATION: f64 = 0.434;

/// Default corrected `0x81` intensity used to select high-signal pixels for
/// auxiliary-frame research. It is a diagnostic constant, not a liveness
/// threshold.
pub const DEFAULT_AUXILIARY_SIGNAL_THRESHOLD: u8 = 20;

/// Descriptive comparison of the three inferred TLFD auxiliary frames.
///
/// `0x01` is treated as a dark/reference channel and is subtracted with
/// saturating arithmetic from `0x84` and `0x81`. This mirrors the *shape* of
/// the vendor's empty-frame subtraction, but not its proprietary algorithm,
/// calibration, or decision thresholds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LfdAuxiliaryFrameMetrics {
    pub selector_84_mean: f64,
    pub selector_81_mean: f64,
    pub selector_01_mean: f64,
    pub dark_subtracted_84_mean: f64,
    pub dark_subtracted_81_mean: f64,
    pub dark_subtracted_mean_absolute_difference: f64,
    /// Pearson correlation between dark-subtracted `0x84` and `0x81` pixels.
    pub dark_subtracted_correlation: f64,
    /// Number of pixels whose dark-subtracted `0x81` value meets the supplied
    /// high-signal threshold.
    pub high_signal_pixel_count: usize,
    pub high_signal_fraction: f64,
    pub high_signal_84_mean: f64,
    pub high_signal_81_mean: f64,
    pub high_signal_mean_absolute_difference: f64,
    /// Pearson correlation calculated only over high-signal pixels.
    pub high_signal_correlation: f64,
}

/// The three raw auxiliary streams used by the inferred FS80H TLFD flow.
///
/// These buffers are sensitive biometric data. The type intentionally exposes
/// no live/fake verdict: it is an acquisition artifact for research and for
/// building a properly labelled evaluation set.
#[derive(Debug, Clone, PartialEq)]
pub struct LfdAuxiliaryTriplet {
    selector_84: Vec<u8>,
    selector_81: Vec<u8>,
    selector_01: Vec<u8>,
    metrics: LfdAuxiliaryFrameMetrics,
}

impl LfdAuxiliaryTriplet {
    pub(crate) fn new(selector_84: Vec<u8>, selector_81: Vec<u8>, selector_01: Vec<u8>) -> Self {
        let metrics = analyze_lfd_auxiliary_frames(&selector_84, &selector_81, &selector_01)
            .expect("device acquisition always returns complete FS80H streams");
        Self {
            selector_84,
            selector_81,
            selector_01,
            metrics,
        }
    }

    #[must_use]
    pub fn selector_84(&self) -> &[u8] {
        &self.selector_84
    }

    #[must_use]
    pub fn selector_81(&self) -> &[u8] {
        &self.selector_81
    }

    #[must_use]
    pub fn selector_01(&self) -> &[u8] {
        &self.selector_01
    }

    #[must_use]
    pub const fn metrics(&self) -> LfdAuxiliaryFrameMetrics {
        self.metrics
    }
}

/// A deliberately non-decisive result for the current research pipeline.
///
/// A security decision is withheld until a model has been calibrated and
/// evaluated with labelled bona-fide and presentation-attack samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfdResearchAssessment {
    Indeterminate,
}

/// Returns the only responsible verdict available without a calibrated and
/// independently evaluated presentation-attack model.
#[must_use]
pub const fn assess_lfd_auxiliary_triplet(_: &LfdAuxiliaryTriplet) -> LfdResearchAssessment {
    LfdResearchAssessment::Indeterminate
}

/// Opt-in calibration for the experimental biometric-signal gate.
///
/// This threshold is intentionally supplied as data rather than embedded in
/// the acquisition protocol so future labelled captures can recalibrate it.
#[cfg(feature = "lfd-experimental-decision")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LfdExperimentalDecisionOptions {
    /// Minimum full-frame dark-subtracted correlation required by the gate.
    pub minimum_dark_subtracted_correlation: f64,
}

#[cfg(feature = "lfd-experimental-decision")]
impl Default for LfdExperimentalDecisionOptions {
    fn default() -> Self {
        Self {
            minimum_dark_subtracted_correlation: DEFAULT_EXPERIMENTAL_MIN_CORRELATION,
        }
    }
}

/// Experimental result for detecting sufficient fingerprint-like optical
/// signal. This is not a live/fake or anti-spoof verdict.
#[cfg(feature = "lfd-experimental-decision")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LfdExperimentalDecision {
    /// The observed triplet met the configured biometric-signal threshold.
    BiometricSignalPresent {
        observed_correlation: f64,
        minimum_correlation: f64,
    },
    /// The observed triplet did not meet the configured threshold.
    InsufficientBiometricSignal {
        observed_correlation: f64,
        minimum_correlation: f64,
    },
}

/// Versioned calibration policy loaded by an application at runtime.
#[cfg(feature = "lfd-experimental-decision")]
#[derive(Debug, Clone, PartialEq)]
pub struct LfdExperimentalPolicy {
    pub calibration_id: String,
    pub minimum_dark_subtracted_correlation: f64,
}

#[cfg(feature = "lfd-experimental-decision")]
impl Default for LfdExperimentalPolicy {
    fn default() -> Self {
        Self {
            calibration_id: "fs80h-initial-glove-v1".to_owned(),
            minimum_dark_subtracted_correlation: DEFAULT_EXPERIMENTAL_MIN_CORRELATION,
        }
    }
}

/// Error returned when parsing a line-based experimental calibration policy.
#[cfg(feature = "lfd-experimental-decision")]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LfdExperimentalPolicyError {
    #[error("unknown experimental policy key {0:?}")]
    UnknownKey(String),
    #[error("missing experimental policy key {0:?}")]
    MissingKey(&'static str),
    #[error("invalid experimental correlation threshold {0:?}")]
    InvalidThreshold(String),
}

#[cfg(feature = "lfd-experimental-decision")]
impl core::str::FromStr for LfdExperimentalPolicy {
    type Err = LfdExperimentalPolicyError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut calibration_id = None;
        let mut minimum_dark_subtracted_correlation = None;
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(LfdExperimentalPolicyError::UnknownKey(line.to_owned()));
            };
            match key.trim() {
                "calibration_id" => calibration_id = Some(value.trim().to_owned()),
                "minimum_dark_subtracted_correlation" => {
                    let value = value.trim();
                    let parsed = value.parse::<f64>().map_err(|_| {
                        LfdExperimentalPolicyError::InvalidThreshold(value.to_owned())
                    })?;
                    if !parsed.is_finite() {
                        return Err(LfdExperimentalPolicyError::InvalidThreshold(
                            value.to_owned(),
                        ));
                    }
                    minimum_dark_subtracted_correlation = Some(parsed);
                }
                key => return Err(LfdExperimentalPolicyError::UnknownKey(key.to_owned())),
            }
        }
        let calibration_id = calibration_id
            .filter(|value| !value.is_empty())
            .ok_or(LfdExperimentalPolicyError::MissingKey("calibration_id"))?;
        let minimum_dark_subtracted_correlation = minimum_dark_subtracted_correlation.ok_or(
            LfdExperimentalPolicyError::MissingKey("minimum_dark_subtracted_correlation"),
        )?;
        Ok(Self {
            calibration_id,
            minimum_dark_subtracted_correlation,
        })
    }
}

/// Complete result from one experimental biometric-signal evaluation.
#[cfg(feature = "lfd-experimental-decision")]
#[derive(Debug, Clone, PartialEq)]
pub struct LfdExperimentalEvaluation {
    pub device: DeviceInfo,
    pub calibration_id: String,
    pub decision: LfdExperimentalDecision,
    pub metrics: LfdAuxiliaryFrameMetrics,
}

/// Applies the explicitly configured, experimental biometric-signal gate.
///
/// A non-finite threshold is rejected by returning
/// [`LfdExperimentalDecision::InsufficientBiometricSignal`].
#[cfg(feature = "lfd-experimental-decision")]
#[must_use]
pub fn decide_lfd_experimental(
    triplet: &LfdAuxiliaryTriplet,
    options: LfdExperimentalDecisionOptions,
) -> LfdExperimentalDecision {
    let observed_correlation = triplet.metrics().dark_subtracted_correlation;
    let minimum_correlation = options.minimum_dark_subtracted_correlation;
    if minimum_correlation.is_finite() && observed_correlation >= minimum_correlation {
        LfdExperimentalDecision::BiometricSignalPresent {
            observed_correlation,
            minimum_correlation,
        }
    } else {
        LfdExperimentalDecision::InsufficientBiometricSignal {
            observed_correlation,
            minimum_correlation,
        }
    }
}

/// Error returned when auxiliary streams cannot be compared safely.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LfdAuxiliaryAnalysisError {
    #[error("auxiliary stream {name} has {actual} bytes; expected {expected}")]
    InvalidLength {
        name: &'static str,
        actual: usize,
        expected: usize,
    },
}

/// Analyzes three complete FS80H auxiliary streams in selector order `0x84`,
/// `0x81`, `0x01`.
///
/// This function is deterministic and offline: it does not communicate with a
/// scanner, retain biometric data, or determine whether a presented finger is
/// live.
pub fn analyze_lfd_auxiliary_frames(
    selector_84: &[u8],
    selector_81: &[u8],
    selector_01: &[u8],
) -> Result<LfdAuxiliaryFrameMetrics, LfdAuxiliaryAnalysisError> {
    analyze_lfd_auxiliary_frames_with_threshold(
        selector_84,
        selector_81,
        selector_01,
        DEFAULT_AUXILIARY_SIGNAL_THRESHOLD,
    )
}

/// Same as [`analyze_lfd_auxiliary_frames`], with an explicit high-signal
/// threshold for the dark-subtracted `0x81` channel.
pub fn analyze_lfd_auxiliary_frames_with_threshold(
    selector_84: &[u8],
    selector_81: &[u8],
    selector_01: &[u8],
    high_signal_threshold: u8,
) -> Result<LfdAuxiliaryFrameMetrics, LfdAuxiliaryAnalysisError> {
    for (name, stream) in [
        ("selector_84", selector_84),
        ("selector_81", selector_81),
        ("selector_01", selector_01),
    ] {
        if stream.len() != RAW_SENSOR_STREAM_SIZE {
            return Err(LfdAuxiliaryAnalysisError::InvalidLength {
                name,
                actual: stream.len(),
                expected: RAW_SENSOR_STREAM_SIZE,
            });
        }
    }

    let count = RAW_SENSOR_STREAM_SIZE as f64;
    let mut sum_84 = 0.0;
    let mut sum_81 = 0.0;
    let mut sum_01 = 0.0;
    let mut corrected_sum_84 = 0.0;
    let mut corrected_sum_81 = 0.0;
    let mut corrected_square_sum_84 = 0.0;
    let mut corrected_square_sum_81 = 0.0;
    let mut corrected_product_sum = 0.0;
    let mut corrected_mad_sum = 0.0;
    let mut high_signal_count = 0_usize;
    let mut high_signal_sum_84 = 0.0;
    let mut high_signal_sum_81 = 0.0;
    let mut high_signal_square_sum_84 = 0.0;
    let mut high_signal_square_sum_81 = 0.0;
    let mut high_signal_product_sum = 0.0;
    let mut high_signal_mad_sum = 0.0;

    for ((&first, &second), &dark) in selector_84.iter().zip(selector_81).zip(selector_01) {
        sum_84 += f64::from(first);
        sum_81 += f64::from(second);
        sum_01 += f64::from(dark);

        let first_corrected = f64::from(first.saturating_sub(dark));
        let second_corrected = f64::from(second.saturating_sub(dark));
        corrected_sum_84 += first_corrected;
        corrected_sum_81 += second_corrected;
        corrected_square_sum_84 += first_corrected * first_corrected;
        corrected_square_sum_81 += second_corrected * second_corrected;
        corrected_product_sum += first_corrected * second_corrected;
        corrected_mad_sum += (first_corrected - second_corrected).abs();
        if second_corrected >= f64::from(high_signal_threshold) {
            high_signal_count += 1;
            high_signal_sum_84 += first_corrected;
            high_signal_sum_81 += second_corrected;
            high_signal_square_sum_84 += first_corrected * first_corrected;
            high_signal_square_sum_81 += second_corrected * second_corrected;
            high_signal_product_sum += first_corrected * second_corrected;
            high_signal_mad_sum += (first_corrected - second_corrected).abs();
        }
    }

    let corrected_mean_84 = corrected_sum_84 / count;
    let corrected_mean_81 = corrected_sum_81 / count;
    let covariance = corrected_product_sum / count - corrected_mean_84 * corrected_mean_81;
    let variance_84 = corrected_square_sum_84 / count - corrected_mean_84.powi(2);
    let variance_81 = corrected_square_sum_81 / count - corrected_mean_81.powi(2);
    let denominator = (variance_84.max(0.0) * variance_81.max(0.0)).sqrt();
    let high_signal_count_as_f64 = high_signal_count as f64;
    let (high_signal_mean_84, high_signal_mean_81, high_signal_mad, high_signal_correlation) =
        if high_signal_count == 0 {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let high_signal_mean_84 = high_signal_sum_84 / high_signal_count_as_f64;
            let high_signal_mean_81 = high_signal_sum_81 / high_signal_count_as_f64;
            let high_signal_covariance = high_signal_product_sum / high_signal_count_as_f64
                - high_signal_mean_84 * high_signal_mean_81;
            let high_signal_variance_84 =
                high_signal_square_sum_84 / high_signal_count_as_f64 - high_signal_mean_84.powi(2);
            let high_signal_variance_81 =
                high_signal_square_sum_81 / high_signal_count_as_f64 - high_signal_mean_81.powi(2);
            let high_signal_denominator =
                (high_signal_variance_84.max(0.0) * high_signal_variance_81.max(0.0)).sqrt();
            (
                high_signal_mean_84,
                high_signal_mean_81,
                high_signal_mad_sum / high_signal_count_as_f64,
                if high_signal_denominator > f64::EPSILON {
                    high_signal_covariance / high_signal_denominator
                } else {
                    0.0
                },
            )
        };

    Ok(LfdAuxiliaryFrameMetrics {
        selector_84_mean: sum_84 / count,
        selector_81_mean: sum_81 / count,
        selector_01_mean: sum_01 / count,
        dark_subtracted_84_mean: corrected_mean_84,
        dark_subtracted_81_mean: corrected_mean_81,
        dark_subtracted_mean_absolute_difference: corrected_mad_sum / count,
        dark_subtracted_correlation: if denominator > f64::EPSILON {
            covariance / denominator
        } else {
            0.0
        },
        high_signal_pixel_count: high_signal_count,
        high_signal_fraction: high_signal_count_as_f64 / count,
        high_signal_84_mean: high_signal_mean_84,
        high_signal_81_mean: high_signal_mean_81,
        high_signal_mean_absolute_difference: high_signal_mad,
        high_signal_correlation,
    })
}

/// Options for one experimental multi-frame LFD acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LfdResearchOptions {
    /// Number of frame-mode captures to discard before retaining samples.
    ///
    /// These warm-up captures let the scanner's optical path and gain settle without storing
    /// additional biometric streams.
    pub warmup_count: usize,
    /// Number of raw sensor streams to retain. At least two are needed to calculate motion.
    pub sample_count: usize,
    /// Sensor exposure passed to the recovered variable-dose capture command.
    pub dose: u8,
    /// Delay between completed samples.
    pub inter_sample_delay: Duration,
}

impl Default for LfdResearchOptions {
    fn default() -> Self {
        Self {
            warmup_count: 2,
            sample_count: 4,
            dose: crate::DEFAULT_IMAGE_DOSE,
            inter_sample_delay: Duration::from_millis(40),
        }
    }
}

/// One raw 384x800 CMOS stream collected during a research acquisition.
///
/// The stream is sensitive biometric data. Persist it only with the subject's consent and
/// protect it as rigorously as any fingerprint image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfdResearchSample {
    elapsed: Duration,
    capture_timing: LfdResearchCaptureTiming,
    raw_sensor_stream: Vec<u8>,
}

impl LfdResearchSample {
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns timing for the USB and sensor phases that produced this stream.
    #[must_use]
    pub const fn capture_timing(&self) -> LfdResearchCaptureTiming {
        self.capture_timing
    }

    #[must_use]
    pub fn raw_sensor_stream(&self) -> &[u8] {
        &self.raw_sensor_stream
    }
}

/// Timing for one recovered frame-mode acquisition.
///
/// These durations are diagnostic measurements from the host and are not device timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LfdResearchCaptureTiming {
    pub configure_gain: Duration,
    pub sensor_settle: Duration,
    pub flush_bulk_in: Duration,
    pub trigger_frame: Duration,
    pub first_packet_after_trigger: Duration,
    pub read_stream: Duration,
    pub restore_gain: Duration,
    pub total: Duration,
}

/// Descriptive values computed from a raw-sensor sequence.
///
/// These are diagnostic features only. They are not a liveness score and have no
/// acceptance/rejection threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct LfdResearchMetrics {
    /// Arithmetic mean intensity of each stream, in capture order.
    pub mean_intensity: Vec<f64>,
    /// Population standard deviation of each stream, in capture order.
    pub intensity_standard_deviation: Vec<f64>,
    /// Mean absolute pixel difference between each adjacent stream.
    pub adjacent_mean_absolute_difference: Vec<f64>,
}

/// A complete experimental capture and its diagnostic metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct LfdResearchSequence {
    device: DeviceInfo,
    control_bytes: [u8; 7],
    descriptor_bytes: [u8; 64],
    warmup_count: usize,
    dose: u8,
    samples: Vec<LfdResearchSample>,
    metrics: LfdResearchMetrics,
}

impl LfdResearchSequence {
    pub(crate) fn new(
        device: DeviceInfo,
        control_bytes: [u8; 7],
        descriptor_bytes: [u8; 64],
        warmup_count: usize,
        dose: u8,
        samples: Vec<LfdResearchSample>,
    ) -> Self {
        let metrics = metrics_for(&samples);
        Self {
            device,
            control_bytes,
            descriptor_bytes,
            warmup_count,
            dose,
            samples,
            metrics,
        }
    }

    #[must_use]
    pub fn device(&self) -> &DeviceInfo {
        &self.device
    }

    /// Returns the seven control bytes reported before the research acquisition.
    #[must_use]
    pub const fn control_bytes(&self) -> &[u8; 7] {
        &self.control_bytes
    }

    /// Returns the 64-byte device descriptor reported before the research acquisition.
    #[must_use]
    pub const fn descriptor_bytes(&self) -> &[u8; 64] {
        &self.descriptor_bytes
    }

    /// Returns the number of discarded captures performed before recording samples.
    #[must_use]
    pub const fn warmup_count(&self) -> usize {
        self.warmup_count
    }

    #[must_use]
    pub const fn dose(&self) -> u8 {
        self.dose
    }

    #[must_use]
    pub fn samples(&self) -> &[LfdResearchSample] {
        &self.samples
    }

    #[must_use]
    pub fn metrics(&self) -> &LfdResearchMetrics {
        &self.metrics
    }
}

/// The result of attempting an experimental LFD data acquisition.
#[derive(Debug, Clone, PartialEq)]
pub enum LfdResearchOutcome {
    /// The device is not an FS80H and must not be treated as LFD-capable.
    Unsupported { product_id: u16 },
    /// Fewer than two samples were requested, so temporal diagnostics are unavailable.
    InsufficientSamples { requested: usize },
    /// Raw streams were acquired. This is not a live/fake decision.
    Acquired(LfdResearchSequence),
}

pub(crate) fn is_lfd_research_supported(device: &DeviceInfo) -> bool {
    device.product_id == FS80_PID
}

pub(crate) fn sample(
    elapsed: Duration,
    capture_timing: LfdResearchCaptureTiming,
    raw_sensor_stream: Vec<u8>,
) -> LfdResearchSample {
    debug_assert_eq!(raw_sensor_stream.len(), RAW_SENSOR_STREAM_SIZE);
    LfdResearchSample {
        elapsed,
        capture_timing,
        raw_sensor_stream,
    }
}

fn metrics_for(samples: &[LfdResearchSample]) -> LfdResearchMetrics {
    let mean_intensity = samples
        .iter()
        .map(|sample| mean_and_standard_deviation(sample.raw_sensor_stream()).0)
        .collect();
    let intensity_standard_deviation = samples
        .iter()
        .map(|sample| mean_and_standard_deviation(sample.raw_sensor_stream()).1)
        .collect();
    let adjacent_mean_absolute_difference = samples
        .windows(2)
        .map(|pair| {
            mean_absolute_difference(pair[0].raw_sensor_stream(), pair[1].raw_sensor_stream())
        })
        .collect();
    LfdResearchMetrics {
        mean_intensity,
        intensity_standard_deviation,
        adjacent_mean_absolute_difference,
    }
}

fn mean_and_standard_deviation(bytes: &[u8]) -> (f64, f64) {
    if bytes.is_empty() {
        return (0.0, 0.0);
    }
    let mean = bytes.iter().map(|&pixel| f64::from(pixel)).sum::<f64>() / bytes.len() as f64;
    let variance = bytes
        .iter()
        .map(|&pixel| {
            let distance = f64::from(pixel) - mean;
            distance * distance
        })
        .sum::<f64>()
        / bytes.len() as f64;
    (mean, variance.sqrt())
}

fn mean_absolute_difference(left: &[u8], right: &[u8]) -> f64 {
    debug_assert_eq!(left.len(), right.len());
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    left.iter()
        .zip(right)
        .map(|(&left, &right)| (i16::from(left) - i16::from(right)).unsigned_abs() as f64)
        .sum::<f64>()
        / left.len() as f64
}

#[cfg(test)]
mod tests {
    use super::{
        mean_absolute_difference, mean_and_standard_deviation, sample, LfdResearchCaptureTiming,
        LfdResearchSequence,
    };
    use crate::DeviceInfo;
    use std::time::Duration;

    #[test]
    fn reports_deterministic_sequence_statistics() {
        let first = vec![10; crate::RAW_SENSOR_STREAM_SIZE];
        let second = vec![14; crate::RAW_SENSOR_STREAM_SIZE];
        let sequence = LfdResearchSequence::new(
            DeviceInfo {
                vendor_id: 0x1491,
                product_id: 0x0020,
                bus_number: 1,
                address: 2,
                product_name: None,
            },
            [1; 7],
            [2; 64],
            2,
            166,
            vec![
                sample(Duration::ZERO, zero_timing(), first),
                sample(Duration::from_millis(40), zero_timing(), second),
            ],
        );

        assert_eq!(sequence.metrics().mean_intensity, vec![10.0, 14.0]);
        assert_eq!(sequence.control_bytes(), &[1; 7]);
        assert_eq!(sequence.descriptor_bytes(), &[2; 64]);
        assert_eq!(sequence.warmup_count(), 2);
        assert_eq!(
            sequence.metrics().intensity_standard_deviation,
            vec![0.0, 0.0]
        );
        assert_eq!(
            sequence.metrics().adjacent_mean_absolute_difference,
            vec![4.0]
        );
    }

    #[test]
    fn basic_statistics_handle_empty_buffers() {
        assert_eq!(mean_and_standard_deviation(&[]), (0.0, 0.0));
        assert_eq!(mean_absolute_difference(&[], &[]), 0.0);
    }

    #[test]
    fn only_the_fs80h_product_is_research_supported() {
        let fs80h = DeviceInfo {
            vendor_id: 0x1491,
            product_id: 0x0020,
            bus_number: 0,
            address: 0,
            product_name: None,
        };
        let fs81h = DeviceInfo {
            product_id: 0x0025,
            ..fs80h.clone()
        };

        assert!(super::is_lfd_research_supported(&fs80h));
        assert!(!super::is_lfd_research_supported(&fs81h));
    }

    #[test]
    fn inferred_plan_preserves_the_vendor_capture_order() {
        use super::{LfdResearchAcquisitionPlan, LfdResearchCaptureStep, PREVIEW_LFD_DOSE};

        let plan = LfdResearchAcquisitionPlan::inferred_fs80h_tlfd();
        assert_eq!(
            plan.steps(),
            [
                LfdResearchCaptureStep::EnableColoredCmos,
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x84,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x81,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::AuxiliaryFrame {
                    selector: 0x01,
                    dose: PREVIEW_LFD_DOSE,
                },
                LfdResearchCaptureStep::CalculateTlfdFeatures,
                LfdResearchCaptureStep::CalculateFinalDose,
                LfdResearchCaptureStep::FinalFrame { selector: 0x02 },
                LfdResearchCaptureStep::FinalFrame { selector: 0x82 },
                LfdResearchCaptureStep::RestoreColoredCmos,
            ]
        );
    }

    #[test]
    fn auxiliary_analysis_subtracts_the_dark_channel() {
        let selector_84 = vec![10; crate::RAW_SENSOR_STREAM_SIZE];
        let selector_81 = vec![25; crate::RAW_SENSOR_STREAM_SIZE];
        let selector_01 = vec![3; crate::RAW_SENSOR_STREAM_SIZE];

        let metrics = super::analyze_lfd_auxiliary_frames(&selector_84, &selector_81, &selector_01)
            .expect("matching stream lengths are valid");

        assert_eq!(metrics.dark_subtracted_84_mean, 7.0);
        assert_eq!(metrics.dark_subtracted_81_mean, 22.0);
        assert_eq!(metrics.dark_subtracted_mean_absolute_difference, 15.0);
        assert_eq!(metrics.dark_subtracted_correlation, 0.0);
        assert_eq!(
            metrics.high_signal_pixel_count,
            crate::RAW_SENSOR_STREAM_SIZE
        );
        assert_eq!(metrics.high_signal_84_mean, 7.0);
        assert_eq!(metrics.high_signal_81_mean, 22.0);
        assert_eq!(metrics.high_signal_mean_absolute_difference, 15.0);

        let no_signal = super::analyze_lfd_auxiliary_frames_with_threshold(
            &selector_84,
            &selector_81,
            &selector_01,
            23,
        )
        .expect("matching stream lengths are valid");
        assert_eq!(no_signal.high_signal_pixel_count, 0);
        assert_eq!(no_signal.high_signal_correlation, 0.0);
    }

    #[test]
    fn auxiliary_analysis_rejects_incomplete_streams() {
        let stream = vec![0; crate::RAW_SENSOR_STREAM_SIZE];
        let error = super::analyze_lfd_auxiliary_frames(&[], &stream, &stream)
            .expect_err("short stream must not be analyzed");
        assert!(matches!(
            error,
            super::LfdAuxiliaryAnalysisError::InvalidLength {
                name: "selector_84",
                actual: 0,
                ..
            }
        ));
    }

    #[test]
    fn a_triplet_is_explicitly_indeterminate_without_a_calibrated_model() {
        let first = vec![10; crate::RAW_SENSOR_STREAM_SIZE];
        let second = vec![25; crate::RAW_SENSOR_STREAM_SIZE];
        let dark = vec![3; crate::RAW_SENSOR_STREAM_SIZE];
        let triplet = super::LfdAuxiliaryTriplet::new(first, second, dark);

        assert_eq!(
            super::assess_lfd_auxiliary_triplet(&triplet),
            super::LfdResearchAssessment::Indeterminate
        );
        assert_eq!(
            triplet.metrics().high_signal_pixel_count,
            crate::RAW_SENSOR_STREAM_SIZE
        );
    }

    #[cfg(feature = "lfd-experimental-decision")]
    #[test]
    fn experimental_gate_uses_the_configured_threshold() {
        let first = vec![10; crate::RAW_SENSOR_STREAM_SIZE];
        let second = vec![25; crate::RAW_SENSOR_STREAM_SIZE];
        let dark = vec![3; crate::RAW_SENSOR_STREAM_SIZE];
        let triplet = super::LfdAuxiliaryTriplet::new(first, second, dark);

        let decision = super::decide_lfd_experimental(
            &triplet,
            super::LfdExperimentalDecisionOptions {
                minimum_dark_subtracted_correlation: 0.0,
            },
        );
        assert!(matches!(
            decision,
            super::LfdExperimentalDecision::BiometricSignalPresent { .. }
        ));

        let decision = super::decide_lfd_experimental(
            &triplet,
            super::LfdExperimentalDecisionOptions {
                minimum_dark_subtracted_correlation: 0.1,
            },
        );
        assert!(matches!(
            decision,
            super::LfdExperimentalDecision::InsufficientBiometricSignal { .. }
        ));
    }

    #[cfg(feature = "lfd-experimental-decision")]
    #[test]
    fn policy_parser_requires_a_versioned_finite_calibration() {
        let policy: super::LfdExperimentalPolicy =
            "calibration_id=test-v2\nminimum_dark_subtracted_correlation=0.44\n"
                .parse()
                .expect("valid policy");
        assert_eq!(policy.calibration_id, "test-v2");
        assert_eq!(policy.minimum_dark_subtracted_correlation, 0.44);
        assert!(
            "calibration_id=test\nminimum_dark_subtracted_correlation=NaN"
                .parse::<super::LfdExperimentalPolicy>()
                .is_err()
        );
    }

    fn zero_timing() -> LfdResearchCaptureTiming {
        LfdResearchCaptureTiming {
            configure_gain: Duration::ZERO,
            sensor_settle: Duration::ZERO,
            flush_bulk_in: Duration::ZERO,
            trigger_frame: Duration::ZERO,
            first_packet_after_trigger: Duration::ZERO,
            read_stream: Duration::ZERO,
            restore_gain: Duration::ZERO,
            total: Duration::ZERO,
        }
    }
}
