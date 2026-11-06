//! Minutiae extraction via Crossing Number (CN), ridge tracing orientation, and spatial noise filtering.

use super::{BiometricMinutia, MinutiaType};
use std::f32::consts::PI;

const BORDER_MARGIN: u32 = 16;
const MIN_MINUTIA_DISTANCE: f32 = 8.0;
const TRACE_STEPS: usize = 10;
const MIN_ENDING_TRACE_STEPS: usize = 4;
const MIN_BIFURCATION_TRACE_STEPS: usize = 3;

/// Clockwise 8-neighborhood offsets: (dx, dy)
const CW_OFFSETS: [(i32, i32); 8] = [
    (0, -1),  // 0: N
    (1, -1),  // 1: NE
    (1, 0),   // 2: E
    (1, 1),   // 3: SE
    (0, 1),   // 4: S
    (-1, 1),  // 5: SW
    (-1, 0),  // 6: W
    (-1, -1), // 7: NW
];

/// Extracts and filters biometric minutiae from a thinned skeleton grid.
pub fn extract_minutiae_from_skeleton(
    skeleton: &[bool],
    width: usize,
    height: usize,
) -> Vec<BiometricMinutia> {
    extract_minutiae_from_skeleton_with_mask(skeleton, None, width, height)
}

/// Extracts and filters biometric minutiae with an optional safe interior contact mask.
///
/// When `interior_mask` is provided, candidate minutiae lying in the boundary contour
/// of the finger impression are pruned as boundary false endings.
pub fn extract_minutiae_from_skeleton_with_mask(
    skeleton: &[bool],
    interior_mask: Option<&[bool]>,
    width: usize,
    height: usize,
) -> Vec<BiometricMinutia> {
    let mut candidates = Vec::new();
    let w_i32 = width as i32;
    let h_i32 = height as i32;

    for y in 1..height.saturating_sub(1) {
        let row = y * width;
        for x in 1..width.saturating_sub(1) {
            let idx = row + x;
            if !skeleton[idx] {
                continue;
            }

            // Exclude image borders (< 16 px)
            if (x as u32) < BORDER_MARGIN
                || (x as u32) >= (width as u32).saturating_sub(BORDER_MARGIN)
                || (y as u32) < BORDER_MARGIN
                || (y as u32) >= (height as u32).saturating_sub(BORDER_MARGIN)
            {
                continue;
            }

            // Exclude boundary contour false endings outside the safe contact interior
            if let Some(mask) = interior_mask {
                if !mask[idx] {
                    continue;
                }
            }

            // Collect 8-neighborhood
            let mut neighbors = [false; 8];
            for (i, &(dx, dy)) in CW_OFFSETS.iter().enumerate() {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && nx < w_i32 && ny >= 0 && ny < h_i32 {
                    neighbors[i] = skeleton[ny as usize * width + nx as usize];
                }
            }

            // Compute Crossing Number (CN):
            // CN = 0.5 * sum_{i=0..7} |P_i - P_{(i+1)%8}|
            let mut cn_sum = 0;
            for i in 0..8 {
                let curr = i32::from(neighbors[i]);
                let next = i32::from(neighbors[(i + 1) % 8]);
                cn_sum += (curr - next).abs();
            }
            let cn = cn_sum / 2;

            if cn == 1 {
                // Ridge Ending
                let (theta, trace_len) = compute_ending_orientation(skeleton, width, height, x, y);
                // Prune short spurs and pore artifacts (must have at least MIN_ENDING_TRACE_STEPS)
                if trace_len < MIN_ENDING_TRACE_STEPS {
                    continue;
                }
                let quality = calculate_quality(x, y, width, height, trace_len);
                candidates.push(BiometricMinutia {
                    x: x as u32,
                    y: y as u32,
                    theta,
                    kind: MinutiaType::Ending,
                    quality,
                });
            } else if cn == 3 {
                // Ridge Bifurcation
                let (theta, trace_len) =
                    compute_bifurcation_orientation(skeleton, width, height, x, y);
                if trace_len < MIN_BIFURCATION_TRACE_STEPS {
                    continue;
                }
                let quality = calculate_quality(x, y, width, height, trace_len);
                candidates.push(BiometricMinutia {
                    x: x as u32,
                    y: y as u32,
                    theta,
                    kind: MinutiaType::Bifurcation,
                    quality,
                });
            }
        }
    }

    // Filter out pairs closer than MIN_MINUTIA_DISTANCE (8.0 pixels)
    filter_close_pairs(candidates, MIN_MINUTIA_DISTANCE)
}

/// Traces along the skeleton from a ridge ending to determine its inward orientation angle.
fn compute_ending_orientation(
    skeleton: &[bool],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
) -> (f32, usize) {
    let mut prev = (x, y);
    let mut curr = (x, y);

    // Find the single connected neighbor
    for &(dx, dy) in &CW_OFFSETS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            let u_nx = nx as usize;
            let u_ny = ny as usize;
            if skeleton[u_ny * width + u_nx] {
                curr = (u_nx, u_ny);
                break;
            }
        }
    }

    if curr == prev {
        return (0.0, 0);
    }

    let mut steps = 1;
    while steps < TRACE_STEPS {
        let mut next = None;
        for &(dx, dy) in &CW_OFFSETS {
            let nx = curr.0 as i32 + dx;
            let ny = curr.1 as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                let u_nx = nx as usize;
                let u_ny = ny as usize;
                if (u_nx, u_ny) != prev && (u_nx, u_ny) != curr && skeleton[u_ny * width + u_nx] {
                    next = Some((u_nx, u_ny));
                    break;
                }
            }
        }

        match next {
            Some(nxt) => {
                prev = curr;
                curr = nxt;
                steps += 1;
            }
            None => break,
        }
    }

    let dx = curr.0 as f32 - x as f32;
    let dy = curr.1 as f32 - y as f32;
    let mut theta = dy.atan2(dx);
    if theta < 0.0 {
        theta += 2.0 * PI;
    }

    (theta, steps)
}

/// Traces the 3 branches of a bifurcation to determine the orientation of its trunk.
fn compute_bifurcation_orientation(
    skeleton: &[bool],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
) -> (f32, usize) {
    // Find all connected neighbors (up to 3)
    let mut branches = Vec::new();
    for &(dx, dy) in &CW_OFFSETS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            let u_nx = nx as usize;
            let u_ny = ny as usize;
            if skeleton[u_ny * width + u_nx] {
                branches.push((u_nx, u_ny));
            }
        }
    }

    if branches.is_empty() {
        return (0.0, 0);
    }

    // Trace each branch
    let mut branch_vectors = Vec::new();
    let mut total_steps = 0;

    for &first_step in &branches {
        let mut prev = (x, y);
        let mut curr = first_step;
        let mut steps = 1;

        while steps < TRACE_STEPS {
            let mut next = None;
            for &(dx, dy) in &CW_OFFSETS {
                let nx = curr.0 as i32 + dx;
                let ny = curr.1 as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let u_nx = nx as usize;
                    let u_ny = ny as usize;
                    if (u_nx, u_ny) != prev && (u_nx, u_ny) != curr && skeleton[u_ny * width + u_nx]
                    {
                        next = Some((u_nx, u_ny));
                        break;
                    }
                }
            }

            match next {
                Some(nxt) => {
                    prev = curr;
                    curr = nxt;
                    steps += 1;
                }
                None => break,
            }
        }

        total_steps += steps;
        let dx = curr.0 as f32 - x as f32;
        let dy = curr.1 as f32 - y as f32;
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 {
            branch_vectors.push((dx / len, dy / len, dx, dy));
        }
    }

    // If 3 branches are identified, identify the fork (pair with smallest angular separation)
    // and select the remaining branch as the trunk.
    let (dx, dy) = if branch_vectors.len() == 3 {
        let dot01 = branch_vectors[0].0 * branch_vectors[1].0
            + branch_vectors[0].1 * branch_vectors[1].1;
        let dot12 = branch_vectors[1].0 * branch_vectors[2].0
            + branch_vectors[1].1 * branch_vectors[2].1;
        let dot02 = branch_vectors[0].0 * branch_vectors[2].0
            + branch_vectors[0].1 * branch_vectors[2].1;

        // Max dot product corresponds to the smallest angle (the fork branches)
        if dot01 >= dot12 && dot01 >= dot02 {
            // 0 and 1 are the fork, branch 2 is the trunk
            (branch_vectors[2].2, branch_vectors[2].3)
        } else if dot12 >= dot01 && dot12 >= dot02 {
            // 1 and 2 are the fork, branch 0 is the trunk
            (branch_vectors[0].2, branch_vectors[0].3)
        } else {
            // 0 and 2 are the fork, branch 1 is the trunk
            (branch_vectors[1].2, branch_vectors[1].3)
        }
    } else if let Some(first) = branch_vectors.first() {
        (first.2, first.3)
    } else {
        (1.0, 0.0)
    };

    let mut theta = dy.atan2(dx);
    if theta < 0.0 {
        theta += 2.0 * PI;
    }

    (theta, total_steps / branches.len().max(1))
}

/// Discards any pair of minutiae whose Euclidean distance is less than `min_dist`.
fn filter_close_pairs(minutiae: Vec<BiometricMinutia>, min_dist: f32) -> Vec<BiometricMinutia> {
    let n = minutiae.len();
    let mut suppressed = vec![false; n];
    let min_dist_sq = min_dist * min_dist;

    for i in 0..n {
        if suppressed[i] {
            continue;
        }
        for j in (i + 1)..n {
            let dx = minutiae[i].x as f32 - minutiae[j].x as f32;
            let dy = minutiae[i].y as f32 - minutiae[j].y as f32;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq < min_dist_sq {
                suppressed[i] = true;
                suppressed[j] = true;
            }
        }
    }

    minutiae
        .into_iter()
        .enumerate()
        .filter_map(|(i, m)| if !suppressed[i] { Some(m) } else { None })
        .collect()
}

/// Calculates minutia quality based on boundary margin and skeleton stability.
fn calculate_quality(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    trace_len: usize,
) -> f32 {
    let dist_x = (x as f32).min((width - 1 - x) as f32);
    let dist_y = (y as f32).min((height - 1 - y) as f32);
    let border_dist = dist_x.min(dist_y);

    let border_factor = (border_dist / 64.0).clamp(0.2, 1.0);
    let trace_factor = (trace_len as f32 / TRACE_STEPS as f32).clamp(0.3, 1.0);

    (0.6 * border_factor + 0.4 * trace_factor).clamp(0.0, 1.0)
}
