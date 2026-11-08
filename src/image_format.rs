//! Lossless image-container encoders for fingerprint frames.
//!
//! These encoders preserve the captured 8-bit grayscale samples exactly. They do not invert,
//! normalize, or enhance the image; callers can apply one of [`RawFrame`] transformation methods
//! first when they deliberately need a rendered view.

use crate::{error::ImageEncodingError, RawFrame, SENSOR_DPI};
use std::io::Cursor;

/// Lossless image containers supported for exporting a [`RawFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintImageFormat {
    /// Portable Network Graphics with 8-bit grayscale samples and a 500-DPI `pHYs` chunk.
    Png,
    /// TIFF 6.0 with lossless Deflate compression and 500-DPI resolution tags.
    Tiff,
    /// Binary portable graymap (P5) for technical tools. PGM has no standardized DPI metadata.
    Pgm,
}

impl RawFrame {
    /// Encodes this frame in a lossless image container.
    ///
    /// The output always stores the frame's current pixels as 8-bit grayscale (`L8`). PNG and
    /// TIFF include the scanner's physical 500-DPI resolution; PGM only records dimensions and
    /// pixel values by design.
    pub fn encode_image(
        &self,
        format: FingerprintImageFormat,
    ) -> Result<Vec<u8>, ImageEncodingError> {
        match format {
            FingerprintImageFormat::Png => encode_png(self),
            FingerprintImageFormat::Tiff => encode_tiff(self),
            FingerprintImageFormat::Pgm => Ok(encode_pgm(self)),
        }
    }
}

fn encode_png(frame: &RawFrame) -> Result<Vec<u8>, ImageEncodingError> {
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, frame.width(), frame.height());
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: pixels_per_meter(frame.dpi()),
            yppu: pixels_per_meter(frame.dpi()),
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder
            .write_header()
            .map_err(|error| ImageEncodingError::Png(error.to_string()))?;
        writer
            .write_image_data(frame.as_bytes())
            .map_err(|error| ImageEncodingError::Png(error.to_string()))?;
    }
    Ok(output)
}

fn encode_tiff(frame: &RawFrame) -> Result<Vec<u8>, ImageEncodingError> {
    let mut output = Vec::new();
    {
        let cursor = Cursor::new(&mut output);
        let mut encoder = tiff::encoder::TiffEncoder::new(cursor)
            .map_err(|error| ImageEncodingError::Tiff(error.to_string()))?
            .with_compression(tiff::encoder::Compression::Deflate(
                tiff::encoder::DeflateLevel::Balanced,
            ))
            .with_predictor(tiff::tags::Predictor::Horizontal);
        let mut image = encoder
            .new_image::<tiff::encoder::colortype::Gray8>(frame.width(), frame.height())
            .map_err(|error| ImageEncodingError::Tiff(error.to_string()))?;
        image.resolution(
            tiff::tags::ResolutionUnit::Inch,
            tiff::encoder::Rational {
                n: u32::from(frame.dpi()),
                d: 1,
            },
        );
        image
            .write_data(frame.as_bytes())
            .map_err(|error| ImageEncodingError::Tiff(error.to_string()))?;
    }
    Ok(output)
}

fn encode_pgm(frame: &RawFrame) -> Vec<u8> {
    let header = format!("P5\n{} {}\n255\n", frame.width(), frame.height());
    let mut output = Vec::with_capacity(header.len() + frame.as_bytes().len());
    output.extend_from_slice(header.as_bytes());
    output.extend_from_slice(frame.as_bytes());
    output
}

fn pixels_per_meter(dpi: u16) -> u32 {
    // PNG pHYs stores an integer number of pixels per metre. 500 DPI is 19,685.039..., so this
    // is the nearest representable value and correctly preserves the sensor's square pixels.
    (f64::from(dpi) / 0.0254).round() as u32
}

const _: () = assert!(SENSOR_DPI == 500);
