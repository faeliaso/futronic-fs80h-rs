//! 3D spatial-directional cylinder discretization and sigmoid binarization.

use super::cylinder::MccCylinder;
use super::params::{MccConfig, MCC_ND, MCC_NS};
use crate::biometric_core::BiometricMinutia;
use std::f32::consts::PI;

/// A complete fingerprint template represented as an ensemble of 3D Minutia Cylinder-Codes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MccTemplate {
    /// Discretized cylinders for each valid minutia in the fingerprint.
    pub cylinders: Vec<MccCylinder>,
}

impl MccTemplate {
    /// Creates an empty [`MccTemplate`].
    pub const fn empty() -> Self {
        Self {
            cylinders: Vec::new(),
        }
    }

    /// Encodes a slice of [`BiometricMinutia`] into an [`MccTemplate`] using default FVC parameters.
    pub fn from_minutiae(minutiae: &[BiometricMinutia], width: u32, height: u32) -> Self {
        MccEncoder::new(MccConfig::default()).encode(minutiae, width, height)
    }

    /// Encodes a slice of [`BiometricMinutia`] into an [`MccTemplate`] using custom configuration.
    pub fn from_minutiae_with_config(
        minutiae: &[BiometricMinutia],
        width: u32,
        height: u32,
        config: &MccConfig,
    ) -> Self {
        MccEncoder::new(config.clone()).encode(minutiae, width, height)
    }

    /// Returns the number of cylinders in this template.
    #[inline]
    pub fn len(&self) -> usize {
        self.cylinders.len()
    }

    /// Returns `true` if the template contains no cylinders.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cylinders.is_empty()
    }
}

/// Normalizes any angle to $[-\pi, \pi)$.
#[inline(always)]
pub fn normalize_angle(theta: f32) -> f32 {
    let t = theta.rem_euclid(2.0 * PI);
    if t >= PI {
        t - 2.0 * PI
    } else {
        t
    }
}

/// Smallest signed angular difference in $[-\pi, \pi)$.
#[inline(always)]
pub fn angle_diff(a: f32, b: f32) -> f32 {
    normalize_angle(a - b)
}

/// Computes the 1D Gaussian probability density $G(x, \sigma) = \frac{1}{\sigma \sqrt{2\pi}} \exp\left(-\frac{x^2}{2\sigma^2}\right)$.
#[inline(always)]
fn gaussian(x: f32, sigma: f32) -> f32 {
    let inv_norm = 1.0 / (sigma * (2.0 * PI).sqrt());
    inv_norm * (-0.5 * (x / sigma).powi(2)).exp()
}

/// Encoder that transforms raw minutiae constellations into Bit-based Minutia Cylinder-Codes.
#[derive(Debug, Clone)]
pub struct MccEncoder {
    config: MccConfig,
}

impl MccEncoder {
    /// Creates a new [`MccEncoder`] with the specified configuration.
    pub fn new(config: MccConfig) -> Self {
        Self { config }
    }

    /// Encodes a set of minutiae into a template.
    pub fn encode(&self, minutiae: &[BiometricMinutia], width: u32, height: u32) -> MccTemplate {
        if minutiae.is_empty() {
            return MccTemplate::empty();
        }

        let radius = self.config.radius;
        let sigma_s = self.config.sigma_s;
        let sigma_d = self.config.sigma_d;
        let cell_size = (2.0 * radius) / (MCC_NS as f32);
        let slice_width = (2.0 * PI) / (MCC_ND as f32);
        let neighbor_cutoff = radius + 3.0 * sigma_s;
        let neighbor_cutoff_sq = neighbor_cutoff * neighbor_cutoff;
        let spatial_cutoff = 3.0 * sigma_s;
        let spatial_cutoff_sq = spatial_cutoff * spatial_cutoff;

        let w_f32 = width as f32;
        let h_f32 = height as f32;

        let mut cylinders = Vec::with_capacity(minutiae.len());

        // Directional slice angles in local cylinder coordinates: φ_k = -π + (k + 0.5) * δ
        let mut phi_k = [0.0f32; MCC_ND];
        for (k, phi) in phi_k.iter_mut().enumerate() {
            *phi = -PI + (k as f32 + 0.5) * slice_width;
        }

        for (m_idx, m_center) in minutiae.iter().enumerate() {
            let cx = m_center.x as f32;
            let cy = m_center.y as f32;
            let c_theta = m_center.theta;

            let cos_theta = c_theta.cos();
            let sin_theta = c_theta.sin();

            let deg = (c_theta.to_degrees().round() as i32).rem_euclid(360) as u16;
            let mut cylinder = MccCylinder::empty(
                m_center.x.min(u16::MAX as u32) as u16,
                m_center.y.min(u16::MAX as u32) as u16,
                deg,
            );

            // Step 1: Pre-calculate spatial positions p_{i,j} and populate validity mask
            let mut p_x = [[0.0f32; MCC_NS]; MCC_NS];
            let mut p_y = [[0.0f32; MCC_NS]; MCC_NS];
            let mut is_valid = [[false; MCC_NS]; MCC_NS];
            let mut valid_cells = 0u32;

            for i in 0..MCC_NS {
                let u = (i as f32 - (MCC_NS as f32 - 1.0) / 2.0) * cell_size;
                for j in 0..MCC_NS {
                    let v = (j as f32 - (MCC_NS as f32 - 1.0) / 2.0) * cell_size;

                    // Affine transformation rotated by c_theta
                    let px = cx + cos_theta * u - sin_theta * v;
                    let py = cy + sin_theta * u + cos_theta * v;

                    p_x[i][j] = px;
                    p_y[i][j] = py;

                    // Sensor bounds check (with 0.5 margin)
                    let inside = px >= 0.0 && px < w_f32 && py >= 0.0 && py < h_f32;
                    is_valid[i][j] = inside;

                    if inside {
                        valid_cells += MCC_ND as u32;
                        for k in 0..MCC_ND {
                            cylinder.set_mask(i, j, k, true);
                        }
                    }
                }
            }

            // Discard cylinder if valid cells ratio < min_valid_cells_ratio
            let valid_ratio = valid_cells as f32 / (super::params::MCC_TOTAL_CELLS as f32);
            if valid_ratio < self.config.min_valid_cells_ratio {
                continue;
            }

            // Step 2: Filter neighbors within neighbor_cutoff
            let mut neighbors = Vec::new();
            for (t_idx, m_t) in minutiae.iter().enumerate() {
                if t_idx == m_idx {
                    continue;
                }
                let dx = m_t.x as f32 - cx;
                let dy = m_t.y as f32 - cy;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= neighbor_cutoff_sq {
                    neighbors.push((m_t.x as f32, m_t.y as f32, m_t.theta));
                }
            }

            // Step 3: Compute continuous cell contributions C(i, j, k)
            let mut c_values = [[[0.0f32; MCC_ND]; MCC_NS]; MCC_NS];

            for &(tx, ty, t_theta) in &neighbors {
                // Precalculate directional Gaussian for each slice k
                let mut gd_k = [0.0f32; MCC_ND];
                for k in 0..MCC_ND {
                    let theta_slice = c_theta + phi_k[k];
                    let d_theta = angle_diff(t_theta, theta_slice);
                    gd_k[k] = gaussian(d_theta, sigma_d);
                }

                for i in 0..MCC_NS {
                    for j in 0..MCC_NS {
                        if !is_valid[i][j] {
                            continue;
                        }

                        let dx = p_x[i][j] - tx;
                        let dy = p_y[i][j] - ty;
                        let d_sq = dx * dx + dy * dy;

                        if d_sq <= spatial_cutoff_sq {
                            let gs = gaussian(d_sq.sqrt(), sigma_s);
                            for k in 0..MCC_ND {
                                c_values[i][j][k] += gs * gd_k[k];
                            }
                        }
                    }
                }
            }

            // Step 4: Sigmoid Binarization
            for i in 0..MCC_NS {
                for j in 0..MCC_NS {
                    if !is_valid[i][j] {
                        continue;
                    }
                    for (k, &c) in c_values[i][j].iter().enumerate() {
                        // Canonical bit-based sigmoid binarization:
                        // sigmoid(C) >= 0.5 is mathematically equivalent to C >= sigmoid_mu
                        let bit = c >= self.config.sigmoid_mu;
                        cylinder.set_bit(i, j, k, bit);
                    }
                }
            }

            cylinders.push(cylinder);
        }

        MccTemplate { cylinders }
    }
}
