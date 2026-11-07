//! Bit-based Minutia Cylinder representation with zero-allocation POPCNT similarity.

use super::params::{MCC_ND, MCC_NS, MCC_TOTAL_CELLS, MCC_WORDS, MIN_VALID_CELLS_RATIO};

/// Fixed-size, cache-aligned bit-based Minutia Cylinder-Code (MCC).
///
/// Discretizes the 3D neighborhood of a minutia into a cylinder of $16 \times 16 \times 6 = 1536$ cells,
/// represented as 24 continuous 64-bit unsigned integers (192 bytes for values, 192 bytes for mask).
///
/// Total struct footprint in memory is 390 bytes (without padding), ideal for contiguous cache arrays
/// and zero-allocation matching loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MccCylinder {
    /// Cartesian X coordinate of the central minutia in pixels.
    pub min_x: u16,
    /// Cartesian Y coordinate of the central minutia in pixels.
    pub min_y: u16,
    /// Orientation angle $\theta$ quantized to $[0, 359]$ degrees.
    pub min_theta: u16,
    /// 1536-bit cylinder cell values binarized via sigmoid ($24 \times 64$-bit words).
    pub bits: [u64; MCC_WORDS],
    /// 1536-bit spatial validity mask ($24 \times 64$-bit words).
    pub mask: [u64; MCC_WORDS],
}

impl MccCylinder {
    /// Creates a new empty [`MccCylinder`] with all bits and masks initialized to 0.
    #[inline]
    pub const fn empty(min_x: u16, min_y: u16, min_theta: u16) -> Self {
        Self {
            min_x,
            min_y,
            min_theta,
            bits: [0u64; MCC_WORDS],
            mask: [0u64; MCC_WORDS],
        }
    }

    /// Computes the linear index for cell $(i, j, k)$, where:
    /// - $i \in [0, 15]$ is the spatial horizontal index,
    /// - $j \in [0, 15]$ is the spatial vertical index,
    /// - $k \in [0, 5]$ is the directional slice index.
    #[inline(always)]
    pub const fn cell_index(i: usize, j: usize, k: usize) -> usize {
        debug_assert!(i < MCC_NS && j < MCC_NS && k < MCC_ND);
        (k * MCC_NS + j) * MCC_NS + i
    }

    /// Sets the value bit for cell $(i, j, k)$.
    #[inline(always)]
    pub fn set_bit(&mut self, i: usize, j: usize, k: usize, value: bool) {
        let idx = Self::cell_index(i, j, k);
        let word = idx / 64;
        let bit = idx % 64;
        if value {
            self.bits[word] |= 1u64 << bit;
        } else {
            self.bits[word] &= !(1u64 << bit);
        }
    }

    /// Gets the value bit for cell $(i, j, k)$.
    #[inline(always)]
    pub const fn get_bit(&self, i: usize, j: usize, k: usize) -> bool {
        let idx = Self::cell_index(i, j, k);
        let word = idx / 64;
        let bit = idx % 64;
        (self.bits[word] & (1u64 << bit)) != 0
    }

    /// Sets the validity mask bit for cell $(i, j, k)$.
    #[inline(always)]
    pub fn set_mask(&mut self, i: usize, j: usize, k: usize, valid: bool) {
        let idx = Self::cell_index(i, j, k);
        let word = idx / 64;
        let bit = idx % 64;
        if valid {
            self.mask[word] |= 1u64 << bit;
        } else {
            self.mask[word] &= !(1u64 << bit);
        }
    }

    /// Gets the validity mask bit for cell $(i, j, k)$.
    #[inline(always)]
    pub const fn get_mask(&self, i: usize, j: usize, k: usize) -> bool {
        let idx = Self::cell_index(i, j, k);
        let word = idx / 64;
        let bit = idx % 64;
        (self.mask[word] & (1u64 << bit)) != 0
    }

    /// Returns the number of spatially valid cells in this cylinder.
    #[inline]
    pub fn valid_cells_count(&self) -> u32 {
        let mut count = 0u32;
        for &m in &self.mask {
            count += m.count_ones();
        }
        count
    }

    /// Returns the ratio of valid cells relative to the total number of cells ($1536$).
    #[inline]
    pub fn valid_cells_ratio(&self) -> f32 {
        self.valid_cells_count() as f32 / MCC_TOTAL_CELLS as f32
    }

    /// Computes the local similarity between two cylinders using native CPU `POPCNT` instructions.
    ///
    /// Performs **zero dynamic heap allocations**. The operation executes strictly over registers
    /// and stack arrays.
    ///
    /// # Mathematical Formulation
    /// Let $M_{A}$ and $M_{B}$ be the validity masks, and $C_A, C_B$ be the bit vectors.
    /// The common validity mask is $M_{AB} = M_A \land M_B$.
    ///
    /// When the common validity ratio is below [`MIN_VALID_CELLS_RATIO`], $0.0$ is returned.
    ///
    /// For sparse binary biometric descriptors (where ~95% of cells are empty zeros), similarity is
    /// evaluated over the active minutiae features via the Jaccard/Tanimoto coefficient:
    /// $$\gamma(c_A, c_B) = \frac{\Vert{} (C_A \land C_B) \land M_{AB} \Vert{}_1}{\Vert{} (C_A \lor C_B) \land M_{AB} \Vert{}_1}$$
    ///
    /// Returns a similarity score in the inclusive range `0.0..=1.0`.
    #[inline(always)]
    pub fn local_similarity(&self, other: &Self) -> f32 {
        let mut intersection_bits = 0u32;
        let mut union_bits = 0u32;
        let mut valid_bits = 0u32;

        for i in 0..MCC_WORDS {
            let common_mask = self.mask[i] & other.mask[i];
            if common_mask == 0 {
                continue;
            }

            let and_bits = self.bits[i] & other.bits[i] & common_mask;
            let or_bits = (self.bits[i] | other.bits[i]) & common_mask;

            intersection_bits += and_bits.count_ones(); // CPU POPCNT instruction
            union_bits += or_bits.count_ones();
            valid_bits += common_mask.count_ones();
        }

        let min_required = (MCC_TOTAL_CELLS as f32 * MIN_VALID_CELLS_RATIO) as u32;
        if valid_bits < min_required || union_bits == 0 {
            return 0.0;
        }

        intersection_bits as f32 / union_bits as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_cylinder_self_similarity() {
        let mut c = MccCylinder::empty(100, 200, 45);
        // Set all mask bits to valid
        c.mask = [!0u64; MCC_WORDS];
        // Populate pseudo-random pattern
        for i in 0..MCC_WORDS {
            c.bits[i] = 0xAA55AA55_00FF00FF_u64 ^ (i as u64);
        }

        let sim = c.local_similarity(&c);
        assert!(
            (sim - 1.0).abs() < f32::EPSILON,
            "Self-similarity must be exactly 1.0, got: {sim}"
        );
    }

    #[test]
    fn test_opposite_cylinder_similarity() {
        let mut c1 = MccCylinder::empty(100, 200, 45);
        let mut c2 = MccCylinder::empty(100, 200, 45);
        c1.mask = [!0u64; MCC_WORDS];
        c2.mask = [!0u64; MCC_WORDS];

        // All 1s vs all 0s
        c1.bits = [!0u64; MCC_WORDS];
        c2.bits = [0u64; MCC_WORDS];

        let sim = c1.local_similarity(&c2);
        assert!(
            (sim - 0.0).abs() < f32::EPSILON,
            "Opposite bits must give 0.0 similarity, got: {sim}"
        );
    }

    #[test]
    fn test_symmetry() {
        let mut c1 = MccCylinder::empty(100, 200, 45);
        let mut c2 = MccCylinder::empty(100, 200, 45);
        c1.mask = [!0u64; MCC_WORDS];
        c2.mask = [!0u64; MCC_WORDS];

        for i in 0..MCC_WORDS {
            c1.bits[i] = 0x12345678_9ABCDEF0_u64.wrapping_mul(i as u64 + 1);
            c2.bits[i] = 0x0FEDCBA9_87654321_u64.wrapping_mul(i as u64 + 1);
        }

        assert_eq!(c1.local_similarity(&c2), c2.local_similarity(&c1));
    }

    #[test]
    fn test_insufficient_valid_cells_returns_zero() {
        let mut c1 = MccCylinder::empty(100, 200, 45);
        let mut c2 = MccCylinder::empty(100, 200, 45);

        // Set only 1 word valid out of 24 (64 / 1536 = 0.041 < 0.25)
        c1.mask[0] = !0u64;
        c2.mask[0] = !0u64;
        c1.bits[0] = 0xFFFF;
        c2.bits[0] = 0xFFFF;

        let sim = c1.local_similarity(&c2);
        assert_eq!(sim, 0.0, "Insufficient valid cells must yield 0.0");
    }

    #[test]
    fn test_cell_indexing_roundtrip() {
        let mut c = MccCylinder::empty(50, 60, 90);
        assert!(!c.get_bit(5, 7, 3));
        c.set_bit(5, 7, 3, true);
        assert!(c.get_bit(5, 7, 3));
        c.set_bit(5, 7, 3, false);
        assert!(!c.get_bit(5, 7, 3));

        assert!(!c.get_mask(15, 15, 5));
        c.set_mask(15, 15, 5, true);
        assert!(c.get_mask(15, 15, 5));
    }
}
