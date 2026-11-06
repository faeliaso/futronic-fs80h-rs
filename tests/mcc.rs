use futronic_fs80h_rs::biometric_core::bozorth3;
use futronic_fs80h_rs::biometric_core::mcc::{GalleryIndex, MccMatcher, MccTemplate};
use futronic_fs80h_rs::biometric_core::{extract_minutiae, BiometricMinutia, MinutiaType};
use futronic_fs80h_rs::RawFrame;
use std::f32::consts::PI;

/// Synthesizes a realistic minutiae constellation within standard biometric dimensions ($320 \times 480$).
fn generate_canonical_minutiae_cluster(count: usize) -> Vec<BiometricMinutia> {
    generate_canonical_minutiae_cluster_with_seed(count, 0.0)
}

fn generate_canonical_minutiae_cluster_with_seed(count: usize, seed: f32) -> Vec<BiometricMinutia> {
    let mut minutiae = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32;
        // Distribute within central core area (x in [80, 240], y in [120, 360])
        let x = 160.0 + (t * 0.73 + seed).cos() * 70.0;
        let y = 240.0 + (t * 0.73 + seed).sin() * 110.0;
        let theta = (t * 0.52 + seed).rem_euclid(2.0 * PI);
        let kind = if i % 2 == 0 {
            MinutiaType::Ending
        } else {
            MinutiaType::Bifurcation
        };
        minutiae.push(BiometricMinutia {
            x: x.round() as u32,
            y: y.round() as u32,
            theta,
            kind,
            quality: 0.95,
        });
    }
    minutiae
}

#[test]
fn test_identical_template_self_similarity() {
    let minutiae = generate_canonical_minutiae_cluster(30);
    let template = MccTemplate::from_minutiae(&minutiae, 320, 480);
    assert!(!template.is_empty(), "Template should have valid cylinders");

    let score = MccMatcher::match_templates(&template, &template);
    assert!(
        (score - 1.0).abs() < 1e-4,
        "Self-similarity score must be 1.0, got {score}"
    );
}

#[test]
fn test_impostor_different_fingers_rejection() {
    // Two completely different fingers (different seeds)
    let finger_a = generate_canonical_minutiae_cluster_with_seed(35, 12.0);
    let finger_b = generate_canonical_minutiae_cluster_with_seed(35, 78.0);

    let template_a = MccTemplate::from_minutiae(&finger_a, 320, 480);
    let template_b = MccTemplate::from_minutiae(&finger_b, 320, 480);

    let score = MccMatcher::match_templates(&template_a, &template_b);
    assert!(
        score < 0.25,
        "Two different fingers must have low similarity (< 0.25), got: {score}"
    );
}

#[test]
fn test_rotation_invariance_up_to_30_degrees() {
    let minutiae = generate_canonical_minutiae_cluster(35);
    let original_template = MccTemplate::from_minutiae(&minutiae, 320, 480);

    let test_angles_deg = [10.0f32, 20.0f32, 30.0f32];
    let cx = 160.0f32;
    let cy = 240.0f32;

    for &angle_deg in &test_angles_deg {
        let rad = angle_deg.to_radians();
        let cos_r = rad.cos();
        let sin_r = rad.sin();

        // Rotate entire constellation rigidly around center (cx, cy)
        let rotated_minutiae: Vec<BiometricMinutia> = minutiae
            .iter()
            .map(|m| {
                let dx = m.x as f32 - cx;
                let dy = m.y as f32 - cy;
                let rx = cx + cos_r * dx - sin_r * dy;
                let ry = cy + sin_r * dx + cos_r * dy;
                let r_theta = (m.theta + rad).rem_euclid(2.0 * PI);

                BiometricMinutia {
                    x: rx.round() as u32,
                    y: ry.round() as u32,
                    theta: r_theta,
                    kind: m.kind,
                    quality: m.quality,
                }
            })
            .collect();

        let rotated_template = MccTemplate::from_minutiae(&rotated_minutiae, 320, 480);
        let score = MccMatcher::match_templates(&original_template, &rotated_template);

        assert!(
            score > 0.50,
            "Matching score under {angle_deg}° rotation must exceed 0.50, got {score:.4}"
        );
    }
}

#[test]
fn test_translation_invariance_up_to_30_pixels() {
    let minutiae = generate_canonical_minutiae_cluster(35);
    let original_template = MccTemplate::from_minutiae(&minutiae, 320, 480);

    let shifts = [(15.0f32, 10.0f32), (-20.0f32, 15.0f32), (30.0f32, -25.0f32)];

    for (dx, dy) in shifts {
        let translated_minutiae: Vec<BiometricMinutia> = minutiae
            .iter()
            .map(|m| BiometricMinutia {
                x: (m.x as f32 + dx).round() as u32,
                y: (m.y as f32 + dy).round() as u32,
                theta: m.theta,
                kind: m.kind,
                quality: m.quality,
            })
            .collect();

        let translated_template = MccTemplate::from_minutiae(&translated_minutiae, 320, 480);
        let score = MccMatcher::match_templates(&original_template, &translated_template);

        assert!(
            score > 0.50,
            "Matching score under translation ({dx}, {dy}) must exceed 0.50, got {score:.4}"
        );
    }
}

#[test]
fn test_input_minutiae_order_permutation_invariance() {
    let mut minutiae = generate_canonical_minutiae_cluster(30);
    let template_original = MccTemplate::from_minutiae(&minutiae, 320, 480);

    // Permute minutiae order (reverse and interleave)
    minutiae.reverse();
    let template_reversed = MccTemplate::from_minutiae(&minutiae, 320, 480);

    let score = MccMatcher::match_templates(&template_original, &template_reversed);
    assert!(
        (score - 1.0).abs() < 1e-4,
        "Order permutation must yield score ~ 1.0, got {score}"
    );
}

#[test]
fn test_end_to_end_extraction_and_mcc_on_real_frame() {
    let img_path = if std::path::Path::new("capture_original_black.png").exists() {
        "capture_original_black.png"
    } else {
        "capture_original_white.png"
    };

    let raw_bytes = match std::fs::read(img_path) {
        Ok(bytes) => {
            let img = image::load_from_memory(&bytes).expect("decode sample frame");
            img.to_luma8().into_raw()
        }
        Err(_) => {
            eprintln!("Sample image not found, skipping real frame test.");
            return;
        }
    };

    let frame = RawFrame::new(raw_bytes).expect("valid raw frame");
    let minutiae = extract_minutiae(&frame).expect("minutiae extraction");
    assert!(minutiae.len() >= 15, "Expected at least 15 minutiae from real sample frame");

    let template = MccTemplate::from_minutiae(&minutiae, frame.width(), frame.height());
    assert!(!template.is_empty(), "Expected non-empty MCC template");

    // Self match
    let self_score = MccMatcher::match_templates(&template, &template);
    assert!(self_score >= 0.95, "Real frame self-match score must be ~ 1.0, got {self_score}");

    // Cross verification with Bozorth3
    let bozorth_score = bozorth3::match_score(&minutiae, &minutiae);
    assert!(bozorth_score >= 40, "Bozorth3 should confirm self match on real frame (score {bozorth_score})");
}

#[test]
fn test_gallery_index_1_to_n_rayon_search() {
    let probe_cluster = generate_canonical_minutiae_cluster_with_seed(30, 42.0);
    let mut gallery = GalleryIndex::new();

    // Populate gallery with 50 synthetic records from different subjects
    for id in 1..=50 {
        let m = if id == 27 {
            probe_cluster.clone() // Subject 27 is the genuine match
        } else {
            generate_canonical_minutiae_cluster_with_seed(30, id as f32 * 1.37)
        };
        let tmpl = MccTemplate::from_minutiae(&m, 320, 480);
        gallery.insert(id, tmpl);
    }

    let probe_template = MccTemplate::from_minutiae(&probe_cluster, 320, 480);
    let best = gallery.search_best(&probe_template);

    assert!(best.is_some(), "Best candidate must be found");
    let (best_id, best_score) = best.unwrap();
    assert_eq!(best_id, 27, "Must correctly identify candidate ID #27");
    assert!(best_score >= 0.95, "Match score for exact candidate must be >= 0.95, got {best_score}");
}
