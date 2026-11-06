//! Adaptive local thresholding and contact mask segmentation for raw biometric frames.

use crate::RawFrame;

/// Window radius for local mean calculation in pixels (~21x21 window).
const WINDOW_RADIUS: i32 = 10;
/// Bradley threshold sensitivity factor (percentage below local average, e.g. 7%).
const BRADLEY_T_PERCENT: u64 = 7;
/// Minimum local intensity in the raw frame to be considered valid finger contact on the glass prism.
const MIN_CONTACT_RAW_MEAN: u64 = 35;
/// Minimum local dynamic range in a 16x16 block to qualify as active papillary ridge structure.
const MIN_BLOCK_VARIANCE: u8 = 20;

/// Binarizes the raw biometric frame using adaptive local thresholding (Bradley algorithm).
///
/// Returns a row-major byte vector where:
/// - `0`: Papillary ridge (crista)
/// - `255`: Valley / non-contact background (vale)
pub fn binarize_adaptive(frame: &RawFrame) -> Vec<u8> {
    binarize_adaptive_with_interior_mask(frame).0
}

/// Binarizes the raw biometric frame and computes a safe interior contact mask.
///
/// Returns `(binary_image, safe_interior_mask)`.
/// The `safe_interior_mask` is `true` for pixels located well within the active contact area,
/// filtering out boundary contour false endings at the finger silhouette perimeter.
pub fn binarize_adaptive_with_interior_mask(frame: &RawFrame) -> (Vec<u8>, Vec<bool>) {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let raw = frame.as_bytes();

    // 1. Invert the raw frame:
    // In optical FTIR sensors, finger contact reflects light into the sensor (high raw intensity),
    // while glass background reflects minimal light (low raw intensity). Inverting maps ridges
    // to dark pixels (0..100) and valleys/glass to bright pixels (180..255).
    let inverted: Vec<u8> = raw.iter().map(|&p| u8::MAX - p).collect();

    // 2. Compute 2D Integral Images (Summed-Area Tables) for inverted image and raw image
    let stride = width + 1;
    let mut integral_inv = vec![0_u64; stride * (height + 1)];
    let mut integral_raw = vec![0_u64; stride * (height + 1)];

    for y in 0..height {
        let mut row_sum_inv = 0_u64;
        let mut row_sum_raw = 0_u64;
        let row_idx = y * width;
        let int_row = (y + 1) * stride;
        let int_prev_row = y * stride;

        for x in 0..width {
            row_sum_inv += u64::from(inverted[row_idx + x]);
            row_sum_raw += u64::from(raw[row_idx + x]);

            integral_inv[int_row + x + 1] = integral_inv[int_prev_row + x + 1] + row_sum_inv;
            integral_raw[int_row + x + 1] = integral_raw[int_prev_row + x + 1] + row_sum_raw;
        }
    }

    let window_sum = |integral: &[u64], x1: usize, y1: usize, x2: usize, y2: usize| -> u64 {
        let a = integral[y1 * stride + x1];
        let b = integral[y1 * stride + (x2 + 1)];
        let c = integral[(y2 + 1) * stride + x1];
        let d = integral[(y2 + 1) * stride + (x2 + 1)];
        d + a - b - c
    };

    // 3. Precompute block contrast / dynamic range mask to reject non-contact glass
    let block_size = 16_usize;
    let blocks_x = width.div_ceil(block_size);
    let blocks_y = height.div_ceil(block_size);
    let mut block_mask = vec![false; blocks_x * blocks_y];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let start_x = bx * block_size;
            let end_x = (start_x + block_size).min(width);
            let start_y = by * block_size;
            let end_y = (start_y + block_size).min(height);

            let mut min_val = 255_u8;
            let mut max_val = 0_u8;

            for y in start_y..end_y {
                let offset = y * width;
                for x in start_x..end_x {
                    let p = raw[offset + x];
                    if p < min_val {
                        min_val = p;
                    }
                    if p > max_val {
                        max_val = p;
                    }
                }
            }

            if max_val.saturating_sub(min_val) >= MIN_BLOCK_VARIANCE && max_val >= 60 {
                block_mask[by * blocks_x + bx] = true;
            }
        }
    }

    // 4. Compute safe interior mask: blocks where all 8 neighboring blocks are also contact blocks
    let mut interior_blocks = vec![false; blocks_x * blocks_y];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            if !block_mask[by * blocks_x + bx] {
                continue;
            }

            let mut all_neighbors_contact = true;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nbx = bx as i32 + dx;
                    let nby = by as i32 + dy;
                    if nbx < 0 || nbx >= blocks_x as i32 || nby < 0 || nby >= blocks_y as i32 {
                        all_neighbors_contact = false;
                        break;
                    }
                    if !block_mask[nby as usize * blocks_x + nbx as usize] {
                        all_neighbors_contact = false;
                        break;
                    }
                }
                if !all_neighbors_contact {
                    break;
                }
            }

            if all_neighbors_contact {
                interior_blocks[by * blocks_x + bx] = true;
            }
        }
    }

    // 5. Bradley Local Adaptive Thresholding and Pixel-Level Safe Interior Mask
    let mut binary = vec![255_u8; width * height];
    let mut safe_interior = vec![false; width * height];
    let w_i32 = width as i32;
    let h_i32 = height as i32;

    for y in 0..height {
        let by = y / block_size;
        let y_i32 = y as i32;
        let y1 = (y_i32 - WINDOW_RADIUS).clamp(0, h_i32 - 1) as usize;
        let y2 = (y_i32 + WINDOW_RADIUS).clamp(0, h_i32 - 1) as usize;

        for x in 0..width {
            let bx = x / block_size;
            let idx = y * width + x;

            if interior_blocks[by * blocks_x + bx] {
                safe_interior[idx] = true;
            }

            // If this block is outside the finger contact area, leave as valley (255)
            if !block_mask[by * blocks_x + bx] {
                continue;
            }

            let x_i32 = x as i32;
            let x1 = (x_i32 - WINDOW_RADIUS).clamp(0, w_i32 - 1) as usize;
            let x2 = (x_i32 + WINDOW_RADIUS).clamp(0, w_i32 - 1) as usize;

            let count = ((x2 - x1 + 1) * (y2 - y1 + 1)) as u64;

            // Verify contact using raw frame window average
            let raw_sum = window_sum(&integral_raw, x1, y1, x2, y2);
            let raw_mean = raw_sum / count;
            if raw_mean < MIN_CONTACT_RAW_MEAN {
                continue;
            }

            // Inverted frame local average
            let inv_sum = window_sum(&integral_inv, x1, y1, x2, y2);
            let pixel_val = u64::from(inverted[idx]);

            // Ridge condition: pixel in inverted image is darker than local mean by BRADLEY_T_PERCENT
            if pixel_val * 100 * count < inv_sum * (100 - BRADLEY_T_PERCENT) {
                binary[idx] = 0; // Ridge (crista)
            }
        }
    }

    (binary, safe_interior)
}
