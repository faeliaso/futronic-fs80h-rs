//! Classical computer vision minutiae extraction, visual annotation, and NIST BOZORTH3 matching.

pub mod binarize;
pub mod bozorth3;
pub mod crossing_number;
#[cfg(feature = "mcc")]
pub mod mcc;
pub mod render;
pub mod thinning;

use crate::{RawFrame, ScannerError, RAW_FRAME_SIZE};
use thiserror::Error;

pub use render::{render_minutiae_overlay, render_minutiae_overlay_inverted};

/// Classification of a fingerprint minutia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinutiaType {
    /// Ridge ending (terminação de crista).
    Ending,
    /// Ridge bifurcation (bifurcação de crista).
    Bifurcation,
}

/// Canonical representation of a detected fingerprint minutia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiometricMinutia {
    /// Cartesian horizontal coordinate in pixels.
    pub x: u32,
    /// Cartesian vertical coordinate in pixels.
    pub y: u32,
    /// Ridge orientation angle in radians [0, 2π).
    pub theta: f32,
    /// Type of minutia (Ending or Bifurcation).
    pub kind: MinutiaType,
    /// Mathematical reliability and quality estimate in [0.0, 1.0].
    pub quality: f32,
}

impl From<&BiometricMinutia> for fprint_bozorth3::Minutia {
    fn from(m: &BiometricMinutia) -> Self {
        Self {
            x: m.x as i32,
            y: m.y as i32,
            // Bozorth3 operates with discrete orientations normalized to [0, 359]
            theta: (m.theta.to_degrees().round() as i32).rem_euclid(360),
        }
    }
}

/// Errors occurring during biometric frame processing or minutiae extraction.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BiometricError {
    /// The supplied frame buffer does not match expected biometric sensor dimensions.
    #[error("invalid frame size: received {received} bytes, expected {expected}")]
    InvalidFrameSize { received: usize, expected: usize },
    /// No usable fingerprint ridge structure was identified in the frame.
    #[error("insufficient finger contact or ridge structure")]
    InsufficientRidges,
}

impl From<ScannerError> for BiometricError {
    fn from(err: ScannerError) -> Self {
        match err {
            ScannerError::IncompleteFrame { received, expected } => {
                Self::InvalidFrameSize { received, expected }
            }
            _ => Self::InsufficientRidges,
        }
    }
}

/// Executes the classical computer vision pipeline to extract minutiae from a [`RawFrame`].
///
/// Steps executed:
/// 1. Adaptive Local Thresholding (Bradley algorithm) with contact mask.
/// 2. Topological Skeletonization (Zhang-Suen thinning).
/// 3. Crossing Number (CN) feature detection.
/// 4. Inward ridge tracing for continuous angular orientation $\theta \in [0, 2\pi)$.
/// 5. Spatial boundary filtering (< 16 px) and pair noise suppression (< 8 px).
pub fn extract_minutiae(frame: &RawFrame) -> Result<Vec<BiometricMinutia>, BiometricError> {
    if frame.as_bytes().len() != RAW_FRAME_SIZE {
        return Err(BiometricError::InvalidFrameSize {
            received: frame.as_bytes().len(),
            expected: RAW_FRAME_SIZE,
        });
    }

    let width = frame.width() as usize;
    let height = frame.height() as usize;

    // 1. Adaptive binarization and safe interior mask segmentation
    let (binary, safe_interior) = binarize::binarize_adaptive_with_interior_mask(frame);

    // 2. Topological thinning
    let skeleton = thinning::zhang_suen_thinning(&binary, width, height);

    // 3. Crossing number detection, orientation estimation, spur pruning, and spatial filtering
    let minutiae = crossing_number::extract_minutiae_from_skeleton_with_mask(
        &skeleton,
        Some(&safe_interior),
        width,
        height,
    );

    Ok(minutiae)
}
