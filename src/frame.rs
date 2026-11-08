use crate::{
    ScannerError, FS80_RAW_SENSOR_STREAM_SIZE, FS80_RAW_SENSOR_WIDTH, RAW_FRAME_SIZE,
    RAW_SENSOR_HEIGHT, RAW_SENSOR_STREAM_SIZE, RAW_SENSOR_WIDTH, SENSOR_DPI, SENSOR_HEIGHT,
    SENSOR_WIDTH,
};

/// A complete uncompressed 8-bit grayscale image delivered by the scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    data: Vec<u8>,
}

impl RawFrame {
    pub fn new(data: Vec<u8>) -> Result<Self, ScannerError> {
        if data.len() == RAW_FRAME_SIZE {
            Ok(Self { data })
        } else {
            Err(ScannerError::IncompleteFrame {
                received: data.len(),
                expected: RAW_FRAME_SIZE,
            })
        }
    }

    pub fn from_raw_sensor_stream(stream: &[u8]) -> Result<Self, ScannerError> {
        match stream.len() {
            RAW_FRAME_SIZE => Ok(Self {
                data: stream.to_vec(),
            }),
            RAW_SENSOR_STREAM_SIZE => Ok(Self {
                data: resample_bilinear(
                    stream,
                    RAW_SENSOR_WIDTH,
                    RAW_SENSOR_HEIGHT,
                    SENSOR_WIDTH,
                    SENSOR_HEIGHT,
                ),
            }),
            FS80_RAW_SENSOR_STREAM_SIZE => Ok(Self {
                data: resample_bilinear(
                    stream,
                    FS80_RAW_SENSOR_WIDTH,
                    RAW_SENSOR_HEIGHT,
                    SENSOR_WIDTH,
                    SENSOR_HEIGHT,
                ),
            }),
            received => Err(ScannerError::IncompleteFrame {
                received,
                expected: RAW_SENSOR_STREAM_SIZE,
            }),
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
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Resamples an uncompressed 8-bit grayscale image using bilinear interpolation.
#[must_use]
pub fn resample_bilinear(
    raw: &[u8],
    src_width: usize,
    src_height: usize,
    dst_width: usize,
    dst_height: usize,
) -> Vec<u8> {
    let mut output = vec![0; dst_width * dst_height];
    if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
        return output;
    }
    for y in 0..dst_height {
        let sy = y as f32 * (src_height.saturating_sub(1)) as f32
            / dst_height.saturating_sub(1).max(1) as f32;
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(src_height - 1);
        let dy = sy - y0 as f32;
        for x in 0..dst_width {
            let sx = x as f32 * (src_width.saturating_sub(1)) as f32
                / dst_width.saturating_sub(1).max(1) as f32;
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(src_width - 1);
            let dx = sx - x0 as f32;
            let a = f32::from(raw[y0 * src_width + x0]) * (1.0 - dx)
                + f32::from(raw[y0 * src_width + x1]) * dx;
            let b = f32::from(raw[y1 * src_width + x0]) * (1.0 - dx)
                + f32::from(raw[y1 * src_width + x1]) * dx;
            output[y * dst_width + x] = (a * (1.0 - dy) + b * dy).round() as u8;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{resample_bilinear, RawFrame};
    use crate::{
        ScannerError, FS80_RAW_SENSOR_STREAM_SIZE, RAW_FRAME_SIZE, RAW_SENSOR_STREAM_SIZE,
        SENSOR_HEIGHT, SENSOR_WIDTH,
    };

    #[test]
    fn raw_frame_accepts_fs80h_sensor_stream() {
        let mut stream = vec![0; FS80_RAW_SENSOR_STREAM_SIZE];
        let last_row = stream.len() - 320;
        let last_pixel = stream.len() - 1;
        stream[0] = 11;
        stream[319] = 22;
        stream[last_row] = 33;
        stream[last_pixel] = 44;

        let frame = RawFrame::from_raw_sensor_stream(&stream).unwrap();

        assert_eq!(frame.as_bytes().len(), RAW_FRAME_SIZE);
        assert_eq!(frame.width(), SENSOR_WIDTH as u32);
        assert_eq!(frame.height(), SENSOR_HEIGHT as u32);
        assert_eq!(frame.as_bytes()[0], 11);
        assert_eq!(frame.as_bytes()[SENSOR_WIDTH - 1], 22);
        assert_eq!(frame.as_bytes()[RAW_FRAME_SIZE - SENSOR_WIDTH], 33);
        assert_eq!(frame.as_bytes()[RAW_FRAME_SIZE - 1], 44);
    }

    #[test]
    fn raw_frame_accepts_fs81h_sensor_stream() {
        let mut stream = vec![0; RAW_SENSOR_STREAM_SIZE];
        let last_row = stream.len() - 384;
        let last_pixel = stream.len() - 1;
        stream[0] = 11;
        stream[383] = 22;
        stream[last_row] = 33;
        stream[last_pixel] = 44;

        let frame = RawFrame::from_raw_sensor_stream(&stream).unwrap();

        assert_eq!(frame.as_bytes().len(), RAW_FRAME_SIZE);
        assert_eq!(frame.as_bytes()[0], 11);
        assert_eq!(frame.as_bytes()[SENSOR_WIDTH - 1], 22);
        assert_eq!(frame.as_bytes()[RAW_FRAME_SIZE - SENSOR_WIDTH], 33);
        assert_eq!(frame.as_bytes()[RAW_FRAME_SIZE - 1], 44);
    }

    #[test]
    fn raw_frame_rejects_unknown_sensor_stream_size() {
        let error = RawFrame::from_raw_sensor_stream(&[0; 123]).unwrap_err();
        assert_eq!(
            error,
            ScannerError::IncompleteFrame {
                received: 123,
                expected: RAW_SENSOR_STREAM_SIZE,
            }
        );
    }

    #[test]
    fn bilinear_resampling_interpolates_a_predictable_small_image() {
        let output = resample_bilinear(&[0, 100, 200, 255], 2, 2, 3, 3);
        assert_eq!(output, vec![0, 50, 100, 100, 139, 178, 200, 228, 255]);
    }

    #[test]
    fn bilinear_resampling_preserves_corners_when_downscaling() {
        let output = resample_bilinear(&[0, 10, 20, 30, 40, 50, 60, 70, 80], 3, 3, 2, 2);
        assert_eq!(output, vec![0, 20, 60, 80]);
    }
}
