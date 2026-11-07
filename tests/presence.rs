use futronic_fs80h_rs::{
    presence::{DEFAULT_MIN_CONTACT_RATIO, PRESENCE_BLOCK_SIZE, TOTAL_SENSOR_BLOCKS},
    AutoCaptureOptions, CaptureFeedback, RawFrame, ScannerError, RAW_FRAME_SIZE, SENSOR_HEIGHT,
    SENSOR_WIDTH,
};

#[test]
fn blank_and_uniform_frames_report_no_contact() {
    // Test pure black, gray, and pure white empty prism frames
    for val in [0_u8, 30_u8, 128_u8, 240_u8, 255_u8] {
        let frame = RawFrame::new(vec![val; RAW_FRAME_SIZE]).expect("valid frame size");
        let metrics = frame.presence_metrics();

        assert_eq!(metrics.contact_blocks, 0);
        assert_eq!(metrics.total_blocks, TOTAL_SENSOR_BLOCKS);
        assert_eq!(metrics.contact_ratio, 0.0);
        assert_eq!(metrics.mean_contact_contrast, 0.0);
        assert!(!metrics.is_present);
        assert!(!frame.is_finger_present());
        assert_eq!(frame.contact_ratio(), 0.0);
    }
}

#[test]
fn sensor_noise_below_dynamic_range_threshold_is_rejected() {
    // Create subtle noise with dynamic range < 15 across each block
    let mut data = vec![120_u8; RAW_FRAME_SIZE];
    for (i, p) in data.iter_mut().enumerate() {
        *p = 120 + ((i % 7) as u8); // variations between 120 and 126 (range = 6)
    }

    let frame = RawFrame::new(data).expect("valid frame");
    let metrics = frame.presence_metrics();

    assert_eq!(metrics.contact_blocks, 0);
    assert_eq!(metrics.contact_ratio, 0.0);
    assert!(!frame.is_finger_present());
}

#[test]
fn synthetic_finger_ridge_pattern_triggers_presence() {
    let mut data = vec![20_u8; RAW_FRAME_SIZE];

    // Simulate high-contrast ridges in a 160x240 central region (x: 80..240, y: 120..360)
    // 160x240 is 10 x 15 = 150 blocks out of 600 total blocks (25% contact ratio)
    for y in 120..360 {
        for x in 80..240 {
            // Alternating ridges and valleys with high contrast
            let is_ridge = (x / 4) % 2 == 0;
            data[y * SENSOR_WIDTH + x] = if is_ridge { 200 } else { 40 };
        }
    }

    let frame = RawFrame::new(data).expect("valid frame");
    let metrics = frame.presence_metrics();

    assert!(
        metrics.contact_blocks >= 140,
        "expected at least 140 contact blocks, got {}",
        metrics.contact_blocks
    );
    assert!(
        metrics.contact_ratio >= 0.20,
        "expected contact ratio >= 0.20, got {}",
        metrics.contact_ratio
    );
    assert!(metrics.is_present);
    assert!(frame.is_finger_present());
    assert!(metrics.mean_contact_contrast >= 150.0);
}

#[test]
fn evaluate_frame_presence_respects_custom_threshold() {
    let mut data = vec![10_u8; RAW_FRAME_SIZE];

    // Create 30 blocks with ridges (30 / 600 = 0.05 or 5% contact)
    for by in 0..5 {
        for bx in 0..6 {
            for y in (by * PRESENCE_BLOCK_SIZE)..((by + 1) * PRESENCE_BLOCK_SIZE) {
                for x in (bx * PRESENCE_BLOCK_SIZE)..((bx + 1) * PRESENCE_BLOCK_SIZE) {
                    let is_ridge = (x / 2) % 2 == 0;
                    data[y * SENSOR_WIDTH + x] = if is_ridge { 180 } else { 30 };
                }
            }
        }
    }

    let frame = RawFrame::new(data).expect("valid frame");

    // Default threshold is 0.08 (8%), so 5% should NOT trigger presence
    assert!(!frame.is_finger_present());

    // Custom threshold of 0.04 (4%) SHOULD trigger presence
    assert!(frame.is_finger_present_with_ratio(0.04));
    let metrics = frame.presence_metrics_with_threshold(0.04);
    assert!(metrics.is_present);
    assert!((metrics.contact_ratio - 0.05).abs() < 0.01);
}

#[test]
fn real_fingerprint_file_detects_presence_if_available() {
    let path = "capture_original_black.png";
    if !std::path::Path::new(path).exists() {
        return;
    }

    let img = image::open(path).expect("open sample capture");
    let luma = img.to_luma8();
    assert_eq!(luma.width(), SENSOR_WIDTH as u32);
    assert_eq!(luma.height(), SENSOR_HEIGHT as u32);

    let frame = RawFrame::new(luma.into_raw()).expect("valid frame");
    let metrics = frame.presence_metrics();

    println!(
        "Real capture contact_blocks: {}/{}, contact_ratio: {:.2}%, mean_contrast: {:.1}",
        metrics.contact_blocks,
        metrics.total_blocks,
        metrics.contact_ratio * 100.0,
        metrics.mean_contact_contrast
    );

    assert!(
        metrics.is_present,
        "real fingerprint capture must trigger finger presence"
    );
    assert!(
        metrics.contact_ratio >= 0.15,
        "real fingerprint should occupy at least 15% of sensor area, got {:.2}",
        metrics.contact_ratio
    );
}

#[test]
fn auto_capture_options_default_values() {
    let opts = AutoCaptureOptions::default();
    assert_eq!(opts.timeout, std::time::Duration::from_secs(10));
    assert_eq!(opts.min_contact_ratio, DEFAULT_MIN_CONTACT_RATIO);
    assert_eq!(opts.poll_interval, std::time::Duration::from_millis(40));
    assert_eq!(opts.consecutive_stable_frames, 2);
    assert_eq!(opts.stability_tolerance, 0.06);
    assert!(opts.visual_feedback);
    assert!(
        !opts.wait_for_removal,
        "removal wait must be disabled by default"
    );
    assert_eq!(opts.removal_timeout, std::time::Duration::from_secs(5));
}

#[test]
fn capture_feedback_enum_variants_format() {
    let fb1 = CaptureFeedback::WaitingForFinger {
        elapsed: std::time::Duration::from_millis(150),
    };
    let fb2 = CaptureFeedback::FingerDetected {
        contact_ratio: 0.35,
        stable_count: 1,
        required_stable: 2,
    };
    let fb3 = CaptureFeedback::CapturingFinal;
    let fb4 = CaptureFeedback::WaitingForRemoval {
        elapsed: std::time::Duration::from_millis(300),
    };
    let fb5 = CaptureFeedback::Completed;

    assert_eq!(format!("{fb1:?}"), "WaitingForFinger { elapsed: 150ms }");
    assert!(format!("{fb2:?}").contains("0.35"));
    assert_eq!(format!("{fb3:?}"), "CapturingFinal");
    assert_eq!(format!("{fb4:?}"), "WaitingForRemoval { elapsed: 300ms }");
    assert_eq!(format!("{fb5:?}"), "Completed");
}

#[test]
fn scanner_error_displays_new_variants() {
    let err1 = ScannerError::FingerWaitTimeout;
    assert_eq!(
        err1.to_string(),
        "timed out waiting for finger to be placed on the scanner"
    );

    let err2 = ScannerError::FingerRemovalTimeout;
    assert_eq!(
        err2.to_string(),
        "timed out waiting for finger to be removed from the scanner"
    );

    let err3 = ScannerError::Cancelled;
    assert_eq!(err3.to_string(), "scanner capture operation was cancelled");
}
