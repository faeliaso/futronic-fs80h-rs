//! Fast fingerprint presence detection and contact metrics for raw biometric frames.
//!
//! In optical FTIR sensors (Frustrated Total Internal Reflection), an empty glass platen
//! exhibits nearly uniform illumination across the entire field of view, resulting in very low
//! local variance (typically only sensor noise, dynamic range < 10).
//!
//! When a papillary ridge pattern makes physical contact with the glass prism, total internal
//! reflection is frustrated at the contact ridges, producing sharp local optical contrast
//! between ridges and valleys (dynamic range in 16x16 blocks typically exceeding 30–120).
//!
//! This module provides sub-millisecond evaluation of contact area and optical contrast
//! to reliably distinguish between an empty platen, partial touch, and a stable fingerprint.

use crate::{SENSOR_HEIGHT, SENSOR_WIDTH};

/// Minimum local dynamic range (max - min) in a 16x16 block to qualify as active ridge contact.
pub const MIN_BLOCK_DYNAMIC_RANGE: u8 = 20;

/// Minimum peak intensity in a 16x16 block to confirm active illumination/reflection.
pub const MIN_BLOCK_PEAK_INTENSITY: u8 = 60;

/// Nominal width of the analysis block in pixels.
pub const PRESENCE_BLOCK_SIZE: usize = 16;

/// Total number of 16x16 analysis blocks in a standard 320x480 frame (20 x 30 = 600).
pub const TOTAL_SENSOR_BLOCKS: usize =
    (SENSOR_WIDTH / PRESENCE_BLOCK_SIZE) * (SENSOR_HEIGHT / PRESENCE_BLOCK_SIZE);

/// Default minimum contact ratio (~8% of the active sensor area, approx. 48 blocks)
/// required to consider a finger present on the scanner prism.
pub const DEFAULT_MIN_CONTACT_RATIO: f32 = 0.08;

/// Quantitative metrics describing finger contact and optical ridge presence on a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FingerPresenceMetrics {
    /// Number of 16x16 blocks exhibiting active papillary ridge contrast.
    pub contact_blocks: usize,
    /// Total number of analysis blocks evaluated in the frame geometry.
    pub total_blocks: usize,
    /// Fraction of the active sensor surface covered by finger ridges in `[0.0, 1.0]`.
    pub contact_ratio: f32,
    /// Average dynamic range (`max - min`) across the detected contact blocks.
    pub mean_contact_contrast: f32,
    /// Boolean flag indicating whether `contact_ratio` meets or exceeds the required threshold.
    pub is_present: bool,
}

impl FingerPresenceMetrics {
    /// Creates an empty metric indicating no contact.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            contact_blocks: 0,
            total_blocks: TOTAL_SENSOR_BLOCKS,
            contact_ratio: 0.0,
            mean_contact_contrast: 0.0,
            is_present: false,
        }
    }
}

/// Evaluates finger presence metrics across a contiguous 8-bit grayscale image buffer.
///
/// Divides the image into 16x16 non-overlapping blocks, evaluating local dynamic range
/// and peak intensity. If the ratio of active contact blocks to total blocks equals or
/// exceeds `min_contact_ratio`, `is_present` is reported as `true`.
#[must_use]
pub fn evaluate_frame_presence(
    frame_bytes: &[u8],
    width: usize,
    height: usize,
    min_contact_ratio: f32,
) -> FingerPresenceMetrics {
    if width < PRESENCE_BLOCK_SIZE
        || height < PRESENCE_BLOCK_SIZE
        || frame_bytes.len() < width * height
    {
        return FingerPresenceMetrics::empty();
    }

    let blocks_x = width / PRESENCE_BLOCK_SIZE;
    let blocks_y = height / PRESENCE_BLOCK_SIZE;
    let total_blocks = blocks_x * blocks_y;

    if total_blocks == 0 {
        return FingerPresenceMetrics::empty();
    }

    let mut contact_blocks = 0_usize;
    let mut total_contrast_sum = 0_u64;

    for by in 0..blocks_y {
        let start_y = by * PRESENCE_BLOCK_SIZE;
        let end_y = start_y + PRESENCE_BLOCK_SIZE;

        for bx in 0..blocks_x {
            let start_x = bx * PRESENCE_BLOCK_SIZE;
            let end_x = start_x + PRESENCE_BLOCK_SIZE;

            let mut min_val = 255_u8;
            let mut max_val = 0_u8;

            for y in start_y..end_y {
                let row_offset = y * width;
                for x in start_x..end_x {
                    let pixel = frame_bytes[row_offset + x];
                    if pixel < min_val {
                        min_val = pixel;
                    }
                    if pixel > max_val {
                        max_val = pixel;
                    }
                }
            }

            let dynamic_range = max_val.saturating_sub(min_val);
            if dynamic_range >= MIN_BLOCK_DYNAMIC_RANGE && max_val >= MIN_BLOCK_PEAK_INTENSITY {
                contact_blocks += 1;
                total_contrast_sum += u64::from(dynamic_range);
            }
        }
    }

    let contact_ratio = (contact_blocks as f32) / (total_blocks as f32);
    let mean_contact_contrast = if contact_blocks > 0 {
        (total_contrast_sum as f32) / (contact_blocks as f32)
    } else {
        0.0
    };

    let is_present = contact_ratio >= min_contact_ratio;

    FingerPresenceMetrics {
        contact_blocks,
        total_blocks,
        contact_ratio,
        mean_contact_contrast,
        is_present,
    }
}
