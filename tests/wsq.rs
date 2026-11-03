#![cfg(feature = "wsq")]

use futronic_fs80h_rs::{
    RawFrame, WsqError, RAW_FRAME_SIZE, SENSOR_DPI, SENSOR_HEIGHT, SENSOR_WIDTH,
};

fn generate_synthetic_fingerprint() -> RawFrame {
    let mut data = vec![230_u8; RAW_FRAME_SIZE];
    for y in 0..SENSOR_HEIGHT {
        for x in 0..SENSOR_WIDTH {
            // Simulate papillary ridges with a 2D sine wave
            let wave = (((x as f32 * 0.2).sin() + (y as f32 * 0.15).cos()) * 40.0) as i16;
            let val = (180 + wave).clamp(0, 255) as u8;
            data[y * SENSOR_WIDTH + x] = val;
        }
    }
    RawFrame::new(data).expect("valid frame dimensions")
}

#[test]
fn wsq_roundtrip_preserves_dimensions_and_metadata() {
    let original = generate_synthetic_fingerprint();
    let wsq_bytes = original
        .to_wsq(Some(RawFrame::DEFAULT_WSQ_BITRATE))
        .expect("wsq compression succeeds");

    assert!(!wsq_bytes.is_empty());
    // Verify standard FBI WSQ magic markers: SOI (0xFFA0) and EOI (0xFFA1)
    assert_eq!(&wsq_bytes[..2], &[0xFF, 0xA0]);
    assert_eq!(&wsq_bytes[wsq_bytes.len() - 2..], &[0xFF, 0xA1]);

    // Compression ratio check: 153,600 bytes -> under 20,000 bytes (> 7.5:1 ratio)
    assert!(
        wsq_bytes.len() < 20_000,
        "WSQ size is {} bytes, expected < 20 KB",
        wsq_bytes.len()
    );

    let reconstructed = RawFrame::from_wsq(&wsq_bytes).expect("wsq decompression succeeds");
    assert_eq!(reconstructed.width(), SENSOR_WIDTH as u32);
    assert_eq!(reconstructed.height(), SENSOR_HEIGHT as u32);
    assert_eq!(reconstructed.dpi(), SENSOR_DPI);
    assert_eq!(reconstructed.as_bytes().len(), RAW_FRAME_SIZE);

    // Verify reconstruction fidelity on lossy 15:1 compression
    let mse: f64 = original
        .as_bytes()
        .iter()
        .zip(reconstructed.as_bytes())
        .map(|(&orig, &rec)| {
            let diff = f64::from(orig) - f64::from(rec);
            diff * diff
        })
        .sum::<f64>()
        / RAW_FRAME_SIZE as f64;

    assert!(
        mse < 2000.0,
        "MSE after WSQ roundtrip is too high: {mse:.2}"
    );
}

#[test]
fn wsq_rejects_invalid_bitrates() {
    let frame = generate_synthetic_fingerprint();
    assert!(matches!(
        frame.to_wsq(Some(0.05)),
        Err(WsqError::InvalidBitrate(_))
    ));
    assert!(matches!(
        frame.to_wsq(Some(3.0)),
        Err(WsqError::InvalidBitrate(_))
    ));
}

#[test]
fn wsq_rejects_corrupted_data() {
    assert_eq!(RawFrame::from_wsq(&[]), Err(WsqError::EmptyBuffer));

    let corrupted = vec![0xFF, 0xA0, 0x00, 0x00, 0xDE, 0xAD];
    assert!(matches!(
        RawFrame::from_wsq(&corrupted),
        Err(WsqError::DecodeFailed(_))
    ));
}
