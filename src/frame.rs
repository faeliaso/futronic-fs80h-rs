use crate::{
    ScannerError, RAW_FRAME_SIZE, RAW_SENSOR_HEIGHT, RAW_SENSOR_STREAM_SIZE, RAW_SENSOR_WIDTH,
    SENSOR_DPI, SENSOR_HEIGHT, SENSOR_WIDTH,
};

/// A complete, uncompressed 8-bit grayscale image delivered by the sensor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    data: Vec<u8>,
}

impl RawFrame {
    /// Validates and wraps a frame using the scanner's fixed geometry.
    pub fn new(data: Vec<u8>) -> Result<Self, ScannerError> {
        if data.len() != RAW_FRAME_SIZE {
            return Err(ScannerError::IncompleteFrame {
                received: data.len(),
                expected: RAW_FRAME_SIZE,
            });
        }
        Ok(Self { data })
    }

    /// Creates a nominal 320x480 biometric frame from the raw 384x800 CMOS sensor stream
    /// using 2D bilinear resampling.
    ///
    /// If the input buffer already has the nominal size (`RAW_FRAME_SIZE` = 153,600 bytes),
    /// it is wrapped directly.
    pub fn from_raw_sensor_stream(stream: &[u8]) -> Result<Self, ScannerError> {
        if stream.len() == RAW_SENSOR_STREAM_SIZE {
            let resampled = resample_bilinear(
                stream,
                RAW_SENSOR_WIDTH,
                RAW_SENSOR_HEIGHT,
                SENSOR_WIDTH,
                SENSOR_HEIGHT,
            );
            Ok(Self { data: resampled })
        } else if stream.len() == RAW_FRAME_SIZE {
            Ok(Self {
                data: stream.to_vec(),
            })
        } else {
            Err(ScannerError::IncompleteFrame {
                received: stream.len(),
                expected: RAW_SENSOR_STREAM_SIZE,
            })
        }
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        SENSOR_WIDTH as u32
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        SENSOR_HEIGHT as u32
    }

    #[must_use]
    pub const fn dpi(&self) -> u16 {
        SENSOR_DPI
    }

    /// Returns the contiguous, row-major 8-bit grayscale pixel data.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Computes the NIST NFIQ2 image-quality score for this frame.
    ///
    /// This requires the optional `nfiq2` feature. The frame is assessed entirely in memory;
    /// no image data is written to disk or transmitted by this method.
    #[cfg(feature = "nfiq2")]
    pub fn assess_nfiq2(
        &self,
        engine: &crate::nfiq2::Nfiq2,
    ) -> Result<crate::nfiq2::Nfiq2Assessment, crate::nfiq2::Nfiq2Error> {
        engine.assess(self)
    }

    /// Extracts a quality-gated ISO/IEC 19794-2:2011 fingerprint template.
    ///
    /// The template contains minutiae and, when confidently estimated, experimental core/delta
    /// points. The source image remains in memory; this method never writes it to disk.
    #[cfg(feature = "templates")]
    pub fn extract_iso_template(
        &self,
        options: &crate::template::TemplateOptions,
    ) -> Result<crate::template::IsoTemplate, crate::template::TemplateError> {
        crate::template::extract(self, options)
    }

    /// Extracts a quality-gated template in a selected ISO/IEC 19794-2 revision.
    #[cfg(feature = "templates")]
    pub fn extract_iso_template_as(
        &self,
        format: crate::template::IsoFormat,
        options: &crate::template::TemplateOptions,
    ) -> Result<crate::template::IsoTemplate, crate::template::TemplateError> {
        crate::template::extract_as(self, format, options)
    }

    /// Extracts a quality-gated ANSI INCITS 378-2009/AM1 fingerprint template.
    ///
    /// The template represents one live, plain fingerprint view. It does not write the source
    /// image to disk.
    #[cfg(feature = "ansi-378")]
    pub fn extract_ansi_template(
        &self,
        options: &crate::template::TemplateOptions,
    ) -> Result<crate::template::ansi::AnsiTemplate, crate::template::TemplateError> {
        crate::template::ansi::extract(
            self,
            crate::template::ansi::AnsiFormat::Ansi3782009Am1,
            options,
        )
    }

    /// Extracts a quality-gated ANSI INCITS 378 template in a selected revision.
    #[cfg(feature = "ansi-378")]
    pub fn extract_ansi_template_as(
        &self,
        format: crate::template::ansi::AnsiFormat,
        options: &crate::template::TemplateOptions,
    ) -> Result<crate::template::ansi::AnsiTemplate, crate::template::TemplateError> {
        crate::template::ansi::extract(self, format, options)
    }

    /// Recommended target bitrate for standard FBI 15:1 lossy compression (0.75 bpp).
    pub const DEFAULT_WSQ_BITRATE: f32 = 0.75;

    /// Compresses the raw 8-bit grayscale biometric frame into FBI WSQ format.
    ///
    /// `bitrate` specifies the target bits-per-pixel ratio (typically `0.75` for ~15:1
    /// compression). Passing `None` defaults to [`Self::DEFAULT_WSQ_BITRATE`].
    #[cfg(feature = "wsq")]
    pub fn to_wsq(&self, bitrate: Option<f32>) -> Result<Vec<u8>, crate::error::WsqError> {
        let r = bitrate.unwrap_or(Self::DEFAULT_WSQ_BITRATE);
        crate::wsq::encode_to_wsq(
            self.as_bytes(),
            self.width() as usize,
            self.height() as usize,
            self.dpi() as usize,
            r,
        )
    }

    /// Reconstructs a [`RawFrame`] from an FBI-compliant WSQ compressed byte stream.
    #[cfg(feature = "wsq")]
    pub fn from_wsq(wsq_bytes: &[u8]) -> Result<Self, crate::error::WsqError> {
        let (pixels, width, height, _ppi) = crate::wsq::decode_from_wsq(wsq_bytes)?;
        if width != SENSOR_WIDTH || height != SENSOR_HEIGHT || pixels.len() != RAW_FRAME_SIZE {
            return Err(crate::error::WsqError::SizeMismatch {
                received: pixels.len(),
                expected: RAW_FRAME_SIZE,
            });
        }
        Self::new(pixels).map_err(|_| crate::error::WsqError::SizeMismatch {
            received: 0,
            expected: RAW_FRAME_SIZE,
        })
    }

    /// Returns a diagnostic view stretched between two histogram percentiles.
    ///
    /// `low_percentile` and `high_percentile` are expressed in the inclusive
    /// range `0..=100`. Invalid or equal bounds leave the frame unchanged.
    #[must_use]
    pub fn contrast_stretched(&self, low_percentile: u8, high_percentile: u8) -> Self {
        if low_percentile >= high_percentile || high_percentile > 100 {
            return self.clone();
        }

        let mut histogram = [0_usize; 256];
        for &pixel in &self.data {
            histogram[usize::from(pixel)] += 1;
        }

        let low = percentile_value(&histogram, low_percentile);
        let high = percentile_value(&histogram, high_percentile);
        if low >= high {
            return self.clone();
        }

        let range = u16::from(high - low);
        let data = self
            .data
            .iter()
            .map(|&pixel| {
                let scaled = u16::from(pixel.saturating_sub(low)) * u16::from(u8::MAX) / range;
                scaled.min(u16::from(u8::MAX)) as u8
            })
            .collect();
        Self { data }
    }

    /// Returns a copy with inverted pixel intensities (255 - pixel).
    ///
    /// The optical sensor delivers raw values where reflected light creates higher
    /// intensities. Standard biometric systems and visualization tools typically
    /// display or process dark papillary ridges on a light background.
    #[must_use]
    pub fn inverted(&self) -> Self {
        let data = self.data.iter().map(|&pixel| u8::MAX - pixel).collect();
        Self { data }
    }

    /// Returns a copy with background whitening and dynamic range expansion applied.
    ///
    /// This enhances the inverted AFIS view by whitening the non-contact prism background
    /// toward pure white (255) and stretching the papillary ridges across the full
    /// dynamic range, matching the vendor SDK's post-processing contrast improvement.
    #[must_use]
    pub fn background_normalized(&self) -> Self {
        let mut histogram = [0_usize; 256];
        for &pixel in &self.data {
            histogram[usize::from(pixel)] += 1;
        }

        // Determine the background threshold: in the inverted image, the glass
        // prism reflection sits in the upper range (above ~210). Any pixel above
        // this baseline is non-contact glass and is clamped to pure white (255).
        let p_low = percentile_value(&histogram, 1);
        let p_high = percentile_value(&histogram, 80).max(215);
        if p_high <= p_low {
            return self.clone();
        }

        let range = u16::from(p_high - p_low);
        let mut data: Vec<u8> = self
            .data
            .iter()
            .map(|&pixel| {
                if pixel <= p_low {
                    0
                } else if pixel >= p_high {
                    255
                } else {
                    let scaled = u16::from(pixel - p_low) * 255 / range;
                    scaled.min(255) as u8
                }
            })
            .collect();

        // Clear the mechanical bezel shadow / mounting margin (outer 4 pixels)
        // to pure white (255), eliminating non-contact optical aperture artifacts.
        const BEZEL_MARGIN: usize = 4;
        for y in 0..SENSOR_HEIGHT {
            let row_offset = y * SENSOR_WIDTH;
            if !(BEZEL_MARGIN..SENSOR_HEIGHT - BEZEL_MARGIN).contains(&y) {
                data[row_offset..row_offset + SENSOR_WIDTH].fill(255);
            } else {
                data[row_offset..row_offset + BEZEL_MARGIN].fill(255);
                data[row_offset + SENSOR_WIDTH - BEZEL_MARGIN..row_offset + SENSOR_WIDTH].fill(255);
            }
        }

        Self { data }
    }

    /// Returns a copy with biometric ridge sharpening / unsharp masking applied.
    ///
    /// Specifically tuned for 500 DPI fingerprint sensors: enhances the definition of
    /// papillary ridge bifurcations, terminations, and sweat pores while maintaining a
    /// pristine white background.
    #[must_use]
    pub fn ridge_enhanced(&self) -> Self {
        let normalized = self.background_normalized();
        let src = normalized.as_bytes();
        let width = SENSOR_WIDTH;
        let height = SENSOR_HEIGHT;
        let mut data = normalized.data.clone();

        for y in 1..height.saturating_sub(1) {
            let r0 = (y - 1) * width;
            let r1 = y * width;
            let r2 = (y + 1) * width;

            for x in 1..width.saturating_sub(1) {
                let blur = (u32::from(src[r0 + x - 1])
                    + u32::from(src[r0 + x]) * 2
                    + u32::from(src[r0 + x + 1])
                    + u32::from(src[r1 + x - 1]) * 2
                    + u32::from(src[r1 + x]) * 4
                    + u32::from(src[r1 + x + 1]) * 2
                    + u32::from(src[r2 + x - 1])
                    + u32::from(src[r2 + x]) * 2
                    + u32::from(src[r2 + x + 1]))
                    / 16;

                let original = i32::from(src[r1 + x]);
                let diff = original - blur as i32;
                // High-pass sharpening: original + 0.8 * high_frequency_detail
                let sharpened = original + (diff * 4) / 5;
                data[r1 + x] = sharpened.clamp(0, 255) as u8;
            }
        }

        Self { data }
    }

    /// Consumes the frame and returns its pixel buffer.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Resamples an uncompressed 8-bit grayscale image using 2D bilinear interpolation.
#[must_use]
pub fn resample_bilinear(
    raw: &[u8],
    src_width: usize,
    src_height: usize,
    dst_width: usize,
    dst_height: usize,
) -> Vec<u8> {
    let mut output = vec![0_u8; dst_width * dst_height];
    if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
        return output;
    }
    if src_width == dst_width && src_height == dst_height {
        let copy_len = (dst_width * dst_height).min(raw.len());
        output[..copy_len].copy_from_slice(&raw[..copy_len]);
        return output;
    }

    let x_scale = if dst_width > 1 {
        (src_width - 1) as f32 / (dst_width - 1) as f32
    } else {
        0.0
    };
    let y_scale = if dst_height > 1 {
        (src_height - 1) as f32 / (dst_height - 1) as f32
    } else {
        0.0
    };

    for y in 0..dst_height {
        let src_y = y as f32 * y_scale;
        let y0 = (src_y.floor() as usize).min(src_height - 1);
        let y1 = (y0 + 1).min(src_height - 1);
        let dy = src_y - y0 as f32;

        let row0_offset = y0 * src_width;
        let row1_offset = y1 * src_width;
        let out_offset = y * dst_width;

        for x in 0..dst_width {
            let src_x = x as f32 * x_scale;
            let x0 = (src_x.floor() as usize).min(src_width - 1);
            let x1 = (x0 + 1).min(src_width - 1);
            let dx = src_x - x0 as f32;

            let p00 = f32::from(raw[row0_offset + x0]);
            let p01 = f32::from(raw[row0_offset + x1]);
            let p10 = f32::from(raw[row1_offset + x0]);
            let p11 = f32::from(raw[row1_offset + x1]);

            let top = p00 + (p01 - p00) * dx;
            let bottom = p10 + (p11 - p10) * dx;
            let pixel = top + (bottom - top) * dy;

            output[out_offset + x] = pixel.clamp(0.0, 255.0).round() as u8;
        }
    }

    output
}

fn percentile_value(histogram: &[usize; 256], percentile: u8) -> u8 {
    let total = histogram.iter().sum::<usize>();
    let rank = total.saturating_sub(1) * usize::from(percentile) / 100;
    let mut cumulative = 0_usize;
    for (value, &count) in histogram.iter().enumerate() {
        cumulative += count;
        if cumulative > rank {
            return value as u8;
        }
    }
    u8::MAX
}
