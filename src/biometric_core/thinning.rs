//! Zhang-Suen morphological thinning algorithm for fingerprint skeletonization.

/// Applies Zhang-Suen thinning to reduce binary ridges (0 = ridge, 255 = valley)
/// to single-pixel wide skeletons.
///
/// Returns a boolean grid where `true` represents a skeleton ridge pixel.
pub fn zhang_suen_thinning(binary_pixels: &[u8], width: usize, height: usize) -> Vec<bool> {
    assert_eq!(binary_pixels.len(), width * height);

    // Initialize skeleton: true for ridge (pixel == 0), false for valley (pixel != 0)
    let mut grid: Vec<bool> = binary_pixels.iter().map(|&p| p == 0).collect();

    let mut changed = true;
    let mut to_remove = Vec::with_capacity(1024);

    while changed {
        changed = false;

        // --- Sub-iteration 1 ---
        to_remove.clear();
        for y in 1..height.saturating_sub(1) {
            let row = y * width;
            let prev_row = row - width;
            let next_row = row + width;

            for x in 1..width.saturating_sub(1) {
                let idx = row + x;
                if !grid[idx] {
                    continue;
                }

                // 8-neighborhood ordered clock-wise:
                // P9 P2 P3
                // P8 P1 P4
                // P7 P6 P5
                let p2 = grid[prev_row + x];
                let p3 = grid[prev_row + x + 1];
                let p4 = grid[row + x + 1];
                let p5 = grid[next_row + x + 1];
                let p6 = grid[next_row + x];
                let p7 = grid[next_row + x - 1];
                let p8 = grid[row + x - 1];
                let p9 = grid[prev_row + x - 1];

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];

                // B(P1): count of non-zero neighbors
                let b = neighbors.iter().filter(|&&v| v).count();
                if !(2..=6).contains(&b) {
                    continue;
                }

                // A(P1): number of 0 -> 1 transitions in cyclic order
                let mut a = 0;
                for i in 0..8 {
                    let curr = neighbors[i];
                    let next = neighbors[(i + 1) % 8];
                    if !curr && next {
                        a += 1;
                    }
                }
                if a != 1 {
                    continue;
                }

                // Condition c & d for sub-iteration 1:
                // P2 * P4 * P6 == 0
                // P4 * P6 * P8 == 0
                if (p2 && p4 && p6) || (p4 && p6 && p8) {
                    continue;
                }

                to_remove.push(idx);
            }
        }

        if !to_remove.is_empty() {
            changed = true;
            for &idx in &to_remove {
                grid[idx] = false;
            }
        }

        // --- Sub-iteration 2 ---
        to_remove.clear();
        for y in 1..height.saturating_sub(1) {
            let row = y * width;
            let prev_row = row - width;
            let next_row = row + width;

            for x in 1..width.saturating_sub(1) {
                let idx = row + x;
                if !grid[idx] {
                    continue;
                }

                let p2 = grid[prev_row + x];
                let p3 = grid[prev_row + x + 1];
                let p4 = grid[row + x + 1];
                let p5 = grid[next_row + x + 1];
                let p6 = grid[next_row + x];
                let p7 = grid[next_row + x - 1];
                let p8 = grid[row + x - 1];
                let p9 = grid[prev_row + x - 1];

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];

                let b = neighbors.iter().filter(|&&v| v).count();
                if !(2..=6).contains(&b) {
                    continue;
                }

                let mut a = 0;
                for i in 0..8 {
                    let curr = neighbors[i];
                    let next = neighbors[(i + 1) % 8];
                    if !curr && next {
                        a += 1;
                    }
                }
                if a != 1 {
                    continue;
                }

                // Condition c & d for sub-iteration 2:
                // P2 * P4 * P8 == 0
                // P2 * P6 * P8 == 0
                if (p2 && p4 && p8) || (p2 && p6 && p8) {
                    continue;
                }

                to_remove.push(idx);
            }
        }

        if !to_remove.is_empty() {
            changed = true;
            for &idx in &to_remove {
                grid[idx] = false;
            }
        }
    }

    grid
}
