//! Local Similarity Matching and Global Greedy Assignment for MCC templates.

use super::encoder::MccTemplate;
use super::params::MccConfig;

/// Evaluation outcome of an MCC template comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MccMatchResult {
    /// Global normalized matching score in inclusive range `0.0..=1.0`.
    pub score: f32,
    /// Number of distinct one-to-one minutiae correspondences matched.
    pub matched_pairs: usize,
    /// Constellation normalization factor $K = \min(M, N, K_{cfg})$.
    pub k: usize,
}

/// Matcher providing 1:1 comparison between [`MccTemplate`] instances.
#[derive(Debug, Clone, Default)]
pub struct MccMatcher {
    config: MccConfig,
}

impl MccMatcher {
    /// Creates a new [`MccMatcher`] with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`MccMatcher`] with custom configuration.
    pub fn with_config(config: MccConfig) -> Self {
        Self { config }
    }

    /// Compares two templates and returns the global normalized matching score in $[0.0, 1.0]$.
    pub fn match_templates(probe: &MccTemplate, gallery: &MccTemplate) -> f32 {
        Self::default().match_pair(probe, gallery).score
    }

    /// Compares two templates and returns detailed match metrics.
    pub fn match_pair(&self, probe: &MccTemplate, gallery: &MccTemplate) -> MccMatchResult {
        let m = probe.cylinders.len();
        let n = gallery.cylinders.len();

        if m == 0 || n == 0 {
            return MccMatchResult {
                score: 0.0,
                matched_pairs: 0,
                k: 0,
            };
        }

        let k_target = m.min(n).min(self.config.top_k);
        if k_target == 0 {
            return MccMatchResult {
                score: 0.0,
                matched_pairs: 0,
                k: 0,
            };
        }

        // Directional tolerance in degrees
        let max_dir_diff_deg = (self.config.max_directional_diff.to_degrees().round() as i32).max(1);

        // Step 1: Compute candidate pairs (i, j, similarity)
        // Pre-filtered by central minutiae angular difference |θ_i - θ_j| <= π/3
        let mut candidates = Vec::with_capacity(m * n);

        for (i, p) in probe.cylinders.iter().enumerate() {
            for (j, g) in gallery.cylinders.iter().enumerate() {
                let diff_deg = (p.min_theta as i32 - g.min_theta as i32).rem_euclid(360);
                let shortest_deg = if diff_deg > 180 { 360 - diff_deg } else { diff_deg };

                if shortest_deg <= max_dir_diff_deg {
                    let sim = p.local_similarity(g);
                    if sim > 0.0 {
                        candidates.push((sim, i, j));
                    }
                }
            }
        }

        // Step 2: Sort descending by similarity score
        candidates.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        if candidates.is_empty() {
            return MccMatchResult {
                score: 0.0,
                matched_pairs: 0,
                k: k_target,
            };
        }

        // Step 3: Local Greedy Assignment with Spatial Consistency Verification
        // Evaluates top candidate seeds to find the geometrically consistent cluster
        let max_seeds = candidates.len().min(8);
        let max_dist_diff = 16.0f32; // Aligned with NIST POSITION_TOLERANCE (16px)
        let max_rot_diff_deg = 20i32; // Directional tolerance within constellation (20 deg)

        let mut best_score = 0.0f32;
        let mut best_matched_pairs = 0usize;

        for seed_idx in 0..max_seeds {
            let (seed_sim, seed_i, seed_j) = candidates[seed_idx];
            let p_seed = &probe.cylinders[seed_i];
            let g_seed = &gallery.cylinders[seed_j];

            let seed_rot_diff = (g_seed.min_theta as i32 - p_seed.min_theta as i32).rem_euclid(360);
            let seed_rot_shortest = if seed_rot_diff > 180 { seed_rot_diff - 360 } else { seed_rot_diff };

            let mut used_probe = vec![false; m];
            let mut used_gallery = vec![false; n];
            used_probe[seed_i] = true;
            used_gallery[seed_j] = true;

            let mut score_sum = seed_sim;
            let mut matched_pairs = 1usize;

            for &(sim, i, j) in &candidates {
                if used_probe[i] || used_gallery[j] {
                    continue;
                }

                let p = &probe.cylinders[i];
                let g = &gallery.cylinders[j];

                // 1. Angular consistency relative to seed
                let rot_diff = (g.min_theta as i32 - p.min_theta as i32).rem_euclid(360);
                let rot_shortest = if rot_diff > 180 { rot_diff - 360 } else { rot_diff };
                if (rot_shortest - seed_rot_shortest).abs() > max_rot_diff_deg {
                    continue;
                }

                // 2. Relative Euclidean distance consistency
                let dx_p = p.min_x as f32 - p_seed.min_x as f32;
                let dy_p = p.min_y as f32 - p_seed.min_y as f32;
                let dist_p = (dx_p * dx_p + dy_p * dy_p).sqrt();

                let dx_g = g.min_x as f32 - g_seed.min_x as f32;
                let dy_g = g.min_y as f32 - g_seed.min_y as f32;
                let dist_g = (dx_g * dx_g + dy_g * dy_g).sqrt();

                if (dist_p - dist_g).abs() <= max_dist_diff {
                    used_probe[i] = true;
                    used_gallery[j] = true;
                    score_sum += sim;
                    matched_pairs += 1;
                    if matched_pairs == k_target {
                        break;
                    }
                }
            }

            let candidate_score = (score_sum / k_target as f32).clamp(0.0, 1.0);
            if candidate_score > best_score {
                best_score = candidate_score;
                best_matched_pairs = matched_pairs;
            }
        }

        MccMatchResult {
            score: best_score,
            matched_pairs: best_matched_pairs,
            k: k_target,
        }
    }
}
