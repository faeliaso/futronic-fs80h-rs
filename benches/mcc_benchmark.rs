use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use futronic_fs80h_rs::biometric_core::mcc::{
    GalleryIndex, GalleryRecord, MccCylinder, MccMatcher, MccTemplate, MCC_WORDS,
};
use futronic_fs80h_rs::biometric_core::{BiometricMinutia, MinutiaType};
use std::f32::consts::PI;

fn generate_synthetic_minutiae(count: usize, seed_offset: f32) -> Vec<BiometricMinutia> {
    let mut minutiae = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32;
        // Distribute within a 200x300 bounding box centered in a 320x480 frame
        let x = 60.0 + (t * 17.3 + seed_offset).sin().abs() * 200.0;
        let y = 80.0 + (t * 23.7 + seed_offset).cos().abs() * 300.0;
        let theta = (t * 0.45 + seed_offset).rem_euclid(2.0 * PI);
        let kind = if i % 2 == 0 {
            MinutiaType::Ending
        } else {
            MinutiaType::Bifurcation
        };
        minutiae.push(BiometricMinutia {
            x: x as u32,
            y: y as u32,
            theta,
            kind,
            quality: 0.9,
        });
    }
    minutiae
}

fn bench_local_similarity(c: &mut Criterion) {
    let mut c1 = MccCylinder::empty(100, 200, 45);
    let mut c2 = MccCylinder::empty(102, 198, 48);

    c1.mask = [!0u64; MCC_WORDS];
    c2.mask = [!0u64; MCC_WORDS];

    for i in 0..MCC_WORDS {
        c1.bits[i] = 0xAA55AA55_00FF00FF_u64 ^ (i as u64);
        c2.bits[i] = 0xAA55AA55_00FF00FE_u64 ^ (i as u64);
    }

    let mut group = c.benchmark_group("mcc_cylinder_similarity");
    group.throughput(Throughput::Elements(1));

    group.bench_function("local_similarity_single_pair", |b| {
        b.iter(|| black_box(c1.local_similarity(black_box(&c2))))
    });

    group.finish();
}

fn bench_template_match_1_to_1(c: &mut Criterion) {
    let minutiae_a = generate_synthetic_minutiae(40, 0.0);
    let minutiae_b = generate_synthetic_minutiae(40, 0.05);

    let template_a = MccTemplate::from_minutiae(&minutiae_a, 320, 480);
    let template_b = MccTemplate::from_minutiae(&minutiae_b, 320, 480);

    let mut group = c.benchmark_group("mcc_template_matching");
    group.throughput(Throughput::Elements(1));

    group.bench_function("match_templates_40_minutiae", |b| {
        b.iter(|| black_box(MccMatcher::match_templates(black_box(&template_a), black_box(&template_b))))
    });

    group.finish();
}

fn bench_gallery_search_1_to_n(c: &mut Criterion) {
    let gallery_size = 1000;
    let mut records = Vec::with_capacity(gallery_size);

    for id in 0..gallery_size {
        let minutiae = generate_synthetic_minutiae(35, id as f32 * 0.1);
        let template = MccTemplate::from_minutiae(&minutiae, 320, 480);
        records.push(GalleryRecord::new(id as u64, template));
    }

    let gallery = GalleryIndex::with_records(records);
    let probe_minutiae = generate_synthetic_minutiae(35, 42.0 * 0.1); // Matches id 42
    let probe_template = MccTemplate::from_minutiae(&probe_minutiae, 320, 480);

    let mut group = c.benchmark_group("mcc_gallery_search_1_to_n");
    group.throughput(Throughput::Elements(gallery_size as u64));

    group.bench_with_input(
        BenchmarkId::new("rayon_1_to_n_search", gallery_size),
        &gallery_size,
        |b, _| {
            b.iter(|| black_box(gallery.search(black_box(&probe_template), black_box(0.60))))
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_local_similarity,
    bench_template_match_1_to_1,
    bench_gallery_search_1_to_n
);
criterion_main!(benches);
