use futronic_fs80h_rs::{
    RawFrame, ScannerError, RAW_FRAME_SIZE, SENSOR_DPI, SENSOR_HEIGHT, SENSOR_WIDTH,
};

#[test]
fn valid_frame_has_fixed_sensor_metadata() {
    let frame = RawFrame::new(vec![42; RAW_FRAME_SIZE]).expect("test input has correct size");
    assert_eq!(frame.width(), SENSOR_WIDTH as u32);
    assert_eq!(frame.height(), SENSOR_HEIGHT as u32);
    assert_eq!(frame.dpi(), SENSOR_DPI);
    assert_eq!(frame.as_bytes().len(), RAW_FRAME_SIZE);
}

#[test]
fn short_frame_is_rejected() {
    let error = RawFrame::new(vec![0; RAW_FRAME_SIZE - 1]);
    assert_eq!(
        error,
        Err(ScannerError::IncompleteFrame {
            received: RAW_FRAME_SIZE - 1,
            expected: RAW_FRAME_SIZE,
        })
    );
}

#[test]
fn contrast_stretch_maps_requested_histogram_bounds() {
    let mut pixels = vec![100; RAW_FRAME_SIZE];
    pixels[..4].copy_from_slice(&[10, 20, 30, 40]);
    let frame = RawFrame::new(pixels).expect("test input has correct size");

    let stretched = frame.contrast_stretched(0, 99);
    assert_eq!(&stretched.as_bytes()[..5], &[0, 28, 56, 85, 255]);
}

#[test]
fn raw_sensor_stream_resamples_to_standard_geometry() {
    use futronic_fs80h_rs::{RAW_SENSOR_HEIGHT, RAW_SENSOR_STREAM_SIZE, RAW_SENSOR_WIDTH};

    let mut stream = vec![0_u8; RAW_SENSOR_STREAM_SIZE];
    for y in 0..RAW_SENSOR_HEIGHT {
        for x in 0..RAW_SENSOR_WIDTH {
            stream[y * RAW_SENSOR_WIDTH + x] = (x % 256) as u8;
        }
    }

    let frame = RawFrame::from_raw_sensor_stream(&stream).expect("valid stream");
    assert_eq!(frame.width(), SENSOR_WIDTH as u32);
    assert_eq!(frame.height(), SENSOR_HEIGHT as u32);
    assert_eq!(frame.as_bytes().len(), RAW_FRAME_SIZE);

    // Left column should be ~0 and right column should match the rescaled coordinate
    assert_eq!(frame.as_bytes()[0], 0);
}

#[test]
fn raw_sensor_stream_accepts_nominal_frame_directly() {
    let nominal = vec![128_u8; RAW_FRAME_SIZE];
    let frame = RawFrame::from_raw_sensor_stream(&nominal).expect("nominal frame accepts");
    assert_eq!(frame.as_bytes().len(), RAW_FRAME_SIZE);
    assert_eq!(frame.as_bytes()[0], 128);
}

#[test]
fn background_normalized_and_ridge_enhanced_preserve_geometry() {
    let frame = RawFrame::new(vec![128; RAW_FRAME_SIZE]).expect("valid frame");
    let norm = frame.background_normalized();
    assert_eq!(norm.width(), SENSOR_WIDTH as u32);
    assert_eq!(norm.height(), SENSOR_HEIGHT as u32);
    assert_eq!(norm.as_bytes().len(), RAW_FRAME_SIZE);

    let enh = frame.ridge_enhanced();
    assert_eq!(enh.width(), SENSOR_WIDTH as u32);
    assert_eq!(enh.height(), SENSOR_HEIGHT as u32);
    assert_eq!(enh.as_bytes().len(), RAW_FRAME_SIZE);
}
