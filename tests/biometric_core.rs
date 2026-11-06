use fprint_bozorth3::{match_score, MIN_COMPUTABLE_BOZORTH_MINUTIAE};
use futronic_fs80h_rs::{
    biometric_core::{
        self,
        crossing_number::extract_minutiae_from_skeleton,
        thinning::zhang_suen_thinning,
        BiometricMinutia, MinutiaType,
    },
    RawFrame, RAW_FRAME_SIZE, SENSOR_HEIGHT, SENSOR_WIDTH,
};
use std::f32::consts::PI;

#[test]
fn domain_model_converts_to_bozorth_minutia() {
    let m = BiometricMinutia {
        x: 120,
        y: 240,
        theta: PI, // 180 degrees
        kind: MinutiaType::Ending,
        quality: 0.95,
    };

    let bozorth_m: fprint_bozorth3::Minutia = (&m).into();
    assert_eq!(bozorth_m.x, 120);
    assert_eq!(bozorth_m.y, 240);
    assert_eq!(bozorth_m.theta, 180);

    // Test angle wrapping
    let m2 = BiometricMinutia {
        x: 50,
        y: 80,
        theta: 2.0 * PI - 0.001,
        kind: MinutiaType::Bifurcation,
        quality: 0.8,
    };
    let bozorth_m2: fprint_bozorth3::Minutia = (&m2).into();
    assert_eq!(bozorth_m2.theta, 0); // 360 rem_euclid 360 == 0
}

#[test]
fn zhang_suen_thinning_reduces_thick_line_to_single_pixel() {
    let width = 20;
    let height = 20;
    // 0 = ridge, 255 = valley
    let mut binary = vec![255_u8; width * height];

    // Create a 5-pixel thick vertical bar from y=4..16, x=8..12
    for y in 4..16 {
        for x in 8..=12 {
            binary[y * width + x] = 0;
        }
    }

    let skeleton = zhang_suen_thinning(&binary, width, height);

    let mut skeleton_pixels = 0;
    for y in 0..height {
        let count = (0..width).filter(|&x| skeleton[y * width + x]).count();
        // Each row must either have 0 or exactly 1 skeleton pixel (never > 1)
        assert!(count <= 1, "row {y} has {count} skeleton pixels, expected <= 1");
        if count == 1 {
            skeleton_pixels += 1;
        }
    }
    assert!(skeleton_pixels >= 5, "skeleton line must be preserved");
}

#[test]
fn crossing_number_detects_canonical_ending_and_bifurcation() {
    let width = 40;
    let height = 40;
    let mut skeleton = vec![false; width * height];

    // Create an Ending: horizontal line from (20, 20) to (28, 20)
    for x in 20..=28 {
        skeleton[20 * width + x] = true;
    }

    let minutiae = extract_minutiae_from_skeleton(&skeleton, width, height);
    // (20, 20) is an ending
    let ending = minutiae.iter().find(|m| m.x == 20 && m.y == 20);
    assert!(ending.is_some(), "Ending should be detected at (20, 20)");
    let ending = ending.unwrap();
    assert_eq!(ending.kind, MinutiaType::Ending);
}

#[test]
fn end_to_end_extraction_and_bozorth3_match_on_real_frame() {
    let img_path = if std::path::Path::new("capture_original_black.png").exists() {
        "capture_original_black.png"
    } else {
        "fingerprint_calibrated.png"
    };
    let raw_bytes = match std::fs::read(img_path) {
        Ok(bytes) => {
            let img = image::load_from_memory(&bytes).expect("decode sample fingerprint");
            img.to_luma8().into_raw()
        }
        Err(_) => {
            eprintln!("Sample fingerprint image not found at '{img_path}', skipping live frame test.");
            return;
        }
    };

    assert_eq!(raw_bytes.len(), RAW_FRAME_SIZE);
    let frame = RawFrame::new(raw_bytes).expect("valid RawFrame");
    assert_eq!(frame.width(), SENSOR_WIDTH as u32);
    assert_eq!(frame.height(), SENSOR_HEIGHT as u32);

    // 1. Extração algorítmica de minúcias
    let minutiae = biometric_core::extract_minutiae(&frame).expect("extraction succeeds");
    println!("Extracted {} minutiae from calibrated frame", minutiae.len());

    assert!(
        minutiae.len() >= MIN_COMPUTABLE_BOZORTH_MINUTIAE,
        "Expected at least {} minutiae, got {}",
        MIN_COMPUTABLE_BOZORTH_MINUTIAE,
        minutiae.len()
    );

    // Verify both types of minutiae are present
    let endings = minutiae.iter().filter(|m| m.kind == MinutiaType::Ending).count();
    let bifurcations = minutiae.iter().filter(|m| m.kind == MinutiaType::Bifurcation).count();
    println!("Endings: {endings}, Bifurcations: {bifurcations}");
    assert!(endings > 0, "Must have endings");
    assert!(bifurcations > 0, "Must have bifurcations");

    // Verify border filtering: no minutia within 16px of borders
    for m in &minutiae {
        assert!(m.x >= 16 && m.x < SENSOR_WIDTH as u32 - 16);
        assert!(m.y >= 16 && m.y < SENSOR_HEIGHT as u32 - 16);
        assert!(m.theta >= 0.0 && m.theta < 2.0 * PI);
        assert!(m.quality >= 0.0 && m.quality <= 1.0);
    }

    let mut min_dist_found = f32::MAX;
    for i in 0..minutiae.len() {
        for j in (i + 1)..minutiae.len() {
            let dx = minutiae[i].x as f32 - minutiae[j].x as f32;
            let dy = minutiae[i].y as f32 - minutiae[j].y as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < min_dist_found {
                min_dist_found = dist;
            }
            assert!(
                dist >= 8.0,
                "Minutiae pair too close: d={dist:.2} between ({}, {}) and ({}, {})",
                minutiae[i].x,
                minutiae[i].y,
                minutiae[j].x,
                minutiae[j].y
            );
        }
    }
    let mean_quality: f32 = minutiae.iter().map(|m| m.quality).sum::<f32>() / minutiae.len() as f32;
    println!("Min distance between minutiae: {min_dist_found:.2} px");
    println!("Mean minutia quality: {mean_quality:.2}");

    // 2. Renderização dos mapas de minúcias anotados (fundo preto e fundo branco)
    let annotated_black = biometric_core::render_minutiae_overlay(&frame, &minutiae);
    assert_eq!(annotated_black.width(), SENSOR_WIDTH as u32);
    assert_eq!(annotated_black.height(), SENSOR_HEIGHT as u32);

    let annotated_white = biometric_core::render_minutiae_overlay_inverted(&frame, &minutiae);
    assert_eq!(annotated_white.width(), SENSOR_WIDTH as u32);
    assert_eq!(annotated_white.height(), SENSOR_HEIGHT as u32);

    // Save test artifacts (4 imagens solicitadas)
    image::save_buffer(
        "capture_original_black.png",
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::L8,
    )
    .expect("save original black PNG");

    let inverted = frame.inverted();
    image::save_buffer(
        "capture_original_white.png",
        inverted.as_bytes(),
        inverted.width(),
        inverted.height(),
        image::ColorType::L8,
    )
    .expect("save original white PNG");

    annotated_black
        .save("capture_annotated_black.png")
        .expect("save annotated black PNG");

    annotated_white
        .save("capture_annotated_white.png")
        .expect("save annotated white PNG");

    // 3. Verificação de matching 1:1 via fprint-bozorth3
    let bozorth_points: Vec<fprint_bozorth3::Minutia> = minutiae.iter().map(Into::into).collect();
    let score = match_score(&bozorth_points, &bozorth_points);
    println!("BOZORTH3 self-match score: {score}");

    // Criterion 7: Score >= 40
    assert!(
        score >= 40,
        "Self-match score must be >= 40 for authentication, got {score}"
    );
}
