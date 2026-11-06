//! Canonical Bit-based Minutia Cylinder-Code (MCC) biometric representation and matching.
//!
//! Based on the FVC-onGoing benchmark standard for 3D invariant cylinder discretization.
//!
//! # Core Features
//! - **3D Invariant Representation:** Transforms spatial and angular minutiae configurations into a fixed-size 3D cylinder.
//! - **Extreme Bitwise Vectorization:** Binarizes cylinder cells into 24 continuous `u64` words (192 bytes), computing similarity strictly via native CPU `POPCNT` (`count_ones()`).
//! - **Zero-Allocation Hot Path:** Cylinder comparison performs no heap allocations.
//! - **1:N Parallel Gallery:** Accelerated multi-core gallery identification via Rayon work-stealing.

pub mod cylinder;
pub mod encoder;
pub mod index;
pub mod matcher;
pub mod params;

pub use cylinder::MccCylinder;
pub use encoder::{MccEncoder, MccTemplate};
pub use index::{GalleryIndex, GalleryRecord};
pub use matcher::{MccMatchResult, MccMatcher};
pub use params::{
    MccConfig, DEFAULT_CELL_SIZE, DEFAULT_MAX_DIRECTIONAL_DIFF, DEFAULT_RADIUS, DEFAULT_SIGMA_D,
    DEFAULT_SIGMA_S, DEFAULT_SLICE_WIDTH, DEFAULT_TOP_K, MCC_ND, MCC_NS, MCC_TOTAL_CELLS,
    MCC_WORDS, MIN_VALID_CELLS_RATIO, SIGMOID_MU, SIGMOID_TAU,
};

use crate::biometric_core::BiometricMinutia;

/// Convenience function to encode a set of minutiae into an [`MccTemplate`] with default configuration.
#[inline]
pub fn encode(minutiae: &[BiometricMinutia], width: u32, height: u32) -> MccTemplate {
    MccTemplate::from_minutiae(minutiae, width, height)
}

/// Convenience function to match two templates and return a normalized score in $[0.0, 1.0]$.
#[inline]
pub fn match_templates(probe: &MccTemplate, gallery: &MccTemplate) -> f32 {
    MccMatcher::match_templates(probe, gallery)
}
