//! Canonical constants, parameters, and configuration for Bit-based MCC.
//!
//! Calibrated according to the FVC-onGoing Minutia Cylinder-Code (MCC) baseline specification.

use std::f32::consts::PI;

/// Number of cells along each spatial dimension of the cylinder ($N_S \times N_S = 16 \times 16$).
pub const MCC_NS: usize = 16;

/// Number of directional angular slices ($N_D = 6$).
pub const MCC_ND: usize = 6;

/// Total number of voxels/cells in a single MCC cylinder: $16 \times 16 \times 6 = 1536$.
pub const MCC_TOTAL_CELLS: usize = MCC_NS * MCC_NS * MCC_ND;

/// Number of 64-bit words required to represent 1536 bits: $1536 / 64 = 24$.
pub const MCC_WORDS: usize = MCC_TOTAL_CELLS / 64;

/// Default cylinder spatial radius in pixels ($R = 75.0$ at 500 DPI).
pub const DEFAULT_RADIUS: f32 = 75.0;

/// Spatial cell size $d = \frac{2R}{N_S} = \frac{150.0}{16} = 9.375$ pixels.
pub const DEFAULT_CELL_SIZE: f32 = (2.0 * DEFAULT_RADIUS) / (MCC_NS as f32);

/// Angular slice width $\delta = \frac{2\pi}{N_D} = \frac{\pi}{3}$ rad ($60^\circ$).
pub const DEFAULT_SLICE_WIDTH: f32 = (2.0 * PI) / (MCC_ND as f32);

/// Spatial Gaussian standard deviation $\sigma_S = \frac{2R}{N_S \sqrt{2}} \approx 6.63$ pixels.
pub const DEFAULT_SIGMA_S: f32 =
    (2.0 * DEFAULT_RADIUS) / (MCC_NS as f32 * std::f32::consts::SQRT_2);

/// Directional Gaussian standard deviation $\sigma_D = \frac{\pi}{6}$ radians ($30^\circ$).
pub const DEFAULT_SIGMA_D: f32 = PI / 6.0;

/// Sigmoid activation center / threshold $\mu_{\Psi} = 0.006$ (calibrated for live sensor elastic skin tolerance).
pub const SIGMOID_MU: f32 = 0.006;

/// Sigmoid steepness / slope parameter $\tau_{\Psi} = 400.0$.
pub const SIGMOID_TAU: f32 = 400.0;

/// Minimum fraction of cylinder cells that must fall within the valid capture area (25%).
pub const MIN_VALID_CELLS_RATIO: f32 = 0.25;

/// Maximum central minutiae directional difference allowed during local pair matching ($\pi/3$ rad).
pub const DEFAULT_MAX_DIRECTIONAL_DIFF: f32 = PI / 3.0;

/// Default maximum number of top matching minutiae pairs used in Global Greedy Assignment ($K = 12$).
pub const DEFAULT_TOP_K: usize = 12;

/// Configuration parameters for Minutia Cylinder-Code generation and matching.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MccConfig {
    /// Cylinder spatial radius in pixels.
    pub radius: f32,
    /// Spatial Gaussian smoothing parameter ($\sigma_S$).
    pub sigma_s: f32,
    /// Directional Gaussian smoothing parameter ($\sigma_D$).
    pub sigma_d: f32,
    /// Sigmoid binarization threshold ($\mu_\Psi$).
    pub sigmoid_mu: f32,
    /// Sigmoid slope ($\tau_\Psi$).
    pub sigmoid_tau: f32,
    /// Minimum required valid cells ratio for a cylinder to be created or matched.
    pub min_valid_cells_ratio: f32,
    /// Maximum central minutiae angle difference allowed for cylinder pair evaluation.
    pub max_directional_diff: f32,
    /// Maximum number of pairs considered in Global Greedy Assignment.
    pub top_k: usize,
}

impl Default for MccConfig {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            sigma_s: DEFAULT_SIGMA_S,
            sigma_d: DEFAULT_SIGMA_D,
            sigmoid_mu: SIGMOID_MU,
            sigmoid_tau: SIGMOID_TAU,
            min_valid_cells_ratio: MIN_VALID_CELLS_RATIO,
            max_directional_diff: DEFAULT_MAX_DIRECTIONAL_DIFF,
            top_k: DEFAULT_TOP_K,
        }
    }
}
