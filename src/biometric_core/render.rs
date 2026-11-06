//! Visual annotation and overlay rendering for biometric minutiae using imageproc.

use super::{BiometricMinutia, MinutiaType};
use crate::RawFrame;
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_hollow_circle_mut, draw_line_segment_mut};

const COLOR_BIFURCATION: Rgb<u8> = Rgb([180, 50, 240]); // Violet / Magenta
const COLOR_ENDING: Rgb<u8> = Rgb([40, 200, 60]);        // Emerald Green
const CIRCLE_RADIUS: i32 = 5;
const VECTOR_START_RADIUS: f32 = 5.0;
const VECTOR_LENGTH: f32 = 14.0;

/// Renders visual minutiae annotations directly over an RGB projection of the captured frame
/// (original sensor view, dark/black background).
///
/// - Bifurcations are marked with a clean radius-5 hollow circle in violet/magenta (`[180, 50, 240]`).
/// - Endings are marked with a clean radius-5 hollow circle in emerald green (`[40, 200, 60]`).
/// - Ridge orientation is rendered as a clean vector line segment starting at the circle perimeter
///   and pointing outward along the ridge orientation $\theta$.
pub fn render_minutiae_overlay(frame: &RawFrame, minutiae: &[BiometricMinutia]) -> RgbImage {
    render_minutiae_overlay_on_bytes(frame.as_bytes(), frame.width(), frame.height(), minutiae)
}

/// Renders visual minutiae annotations over the inverted AFIS view (light/white background).
pub fn render_minutiae_overlay_inverted(
    frame: &RawFrame,
    minutiae: &[BiometricMinutia],
) -> RgbImage {
    let inverted = frame.inverted();
    render_minutiae_overlay_on_bytes(
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        minutiae,
    )
}

/// Helper to render minutiae over an arbitrary 8-bit grayscale row-major buffer.
pub fn render_minutiae_overlay_on_bytes(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    minutiae: &[BiometricMinutia],
) -> RgbImage {
    let mut rgb_img = RgbImage::new(width, height);

    // Copy grayscale L8 pixels into RGB image
    for y in 0..height {
        let row_offset = (y * width) as usize;
        for x in 0..width {
            let gray = raw_bytes[row_offset + x as usize];
            rgb_img.put_pixel(x, y, Rgb([gray, gray, gray]));
        }
    }

    // Overlay minutiae markers and direction vectors
    for m in minutiae {
        let color = match m.kind {
            MinutiaType::Bifurcation => COLOR_BIFURCATION,
            MinutiaType::Ending => COLOR_ENDING,
        };

        let center = (m.x as i32, m.y as i32);

        // 1. Draw a clean, symmetric hollow circle of exact radius 5
        draw_hollow_circle_mut(&mut rgb_img, center, CIRCLE_RADIUS, color);

        // 2. Draw the orientation vector starting cleanly at the circle perimeter
        // and pointing outward along theta, keeping the circle interior intact
        let cos_t = m.theta.cos();
        let sin_t = m.theta.sin();
        let start = (
            m.x as f32 + VECTOR_START_RADIUS * cos_t,
            m.y as f32 + VECTOR_START_RADIUS * sin_t,
        );
        let end = (
            m.x as f32 + (VECTOR_START_RADIUS + VECTOR_LENGTH) * cos_t,
            m.y as f32 + (VECTOR_START_RADIUS + VECTOR_LENGTH) * sin_t,
        );
        draw_line_segment_mut(&mut rgb_img, start, end, color);
    }

    rgb_img
}
