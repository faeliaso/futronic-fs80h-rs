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

#[cfg(feature = "image-formats")]
fn patterned_frame() -> RawFrame {
    let pixels = (0..RAW_FRAME_SIZE)
        .map(|index| (index % 251) as u8)
        .collect();
    RawFrame::new(pixels).expect("generated pattern has the required geometry")
}

#[cfg(feature = "image-formats")]
#[test]
fn png_export_round_trips_pixels_and_records_sensor_dpi() {
    use futronic_fs80h_rs::FingerprintImageFormat;
    use std::io::Cursor;

    let frame = patterned_frame();
    let encoded = frame
        .encode_image(FingerprintImageFormat::Png)
        .expect("PNG encoding succeeds");
    let decoder = png::Decoder::new(Cursor::new(encoded));
    let mut reader = decoder.read_info().expect("encoded PNG is readable");
    let dimensions = reader.info().pixel_dims.expect("PNG has pHYs metadata");
    assert_eq!(dimensions.xppu, 19_685);
    assert_eq!(dimensions.yppu, 19_685);
    assert_eq!(dimensions.unit, png::Unit::Meter);

    let mut decoded = vec![0; reader.output_buffer_size().expect("known output size")];
    let output = reader.next_frame(&mut decoded).expect("pixels decode");
    assert_eq!(output.width, SENSOR_WIDTH as u32);
    assert_eq!(output.height, SENSOR_HEIGHT as u32);
    assert_eq!(output.color_type, png::ColorType::Grayscale);
    assert_eq!(&decoded[..output.buffer_size()], frame.as_bytes());
}

#[cfg(feature = "image-formats")]
#[test]
fn tiff_export_round_trips_pixels_and_records_sensor_dpi() {
    use futronic_fs80h_rs::FingerprintImageFormat;
    use std::io::Cursor;
    use tiff::{decoder::DecodingResult, tags::Tag, ColorType};

    let frame = patterned_frame();
    let encoded = frame
        .encode_image(FingerprintImageFormat::Tiff)
        .expect("TIFF encoding succeeds");
    let mut decoder =
        tiff::decoder::Decoder::new(Cursor::new(encoded)).expect("encoded TIFF is readable");
    assert_eq!(
        decoder.dimensions().expect("dimensions available"),
        (SENSOR_WIDTH as u32, SENSOR_HEIGHT as u32)
    );
    assert_eq!(
        decoder.colortype().expect("color type available"),
        ColorType::Gray(8)
    );
    assert_eq!(
        decoder
            .get_tag_u32_vec(Tag::XResolution)
            .expect("x DPI tag"),
        vec![500, 1]
    );
    assert_eq!(
        decoder
            .get_tag_u32_vec(Tag::YResolution)
            .expect("y DPI tag"),
        vec![500, 1]
    );
    assert_eq!(
        decoder
            .get_tag_unsigned::<u16>(Tag::ResolutionUnit)
            .expect("resolution unit tag"),
        2
    );
    match decoder.read_image().expect("pixels decode") {
        DecodingResult::U8(pixels) => assert_eq!(pixels, frame.as_bytes()),
        other => panic!("expected 8-bit grayscale TIFF, got {other:?}"),
    }
}

#[cfg(feature = "image-formats")]
#[test]
fn pgm_export_has_binary_header_and_exact_pixels() {
    use futronic_fs80h_rs::FingerprintImageFormat;

    let frame = patterned_frame();
    let encoded = frame
        .encode_image(FingerprintImageFormat::Pgm)
        .expect("PGM encoding cannot fail in memory");
    let header = b"P5\n320 480\n255\n";
    assert!(encoded.starts_with(header));
    assert_eq!(&encoded[header.len()..], frame.as_bytes());
}
