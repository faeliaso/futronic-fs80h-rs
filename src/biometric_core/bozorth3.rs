//! Ergonomic integration with the NIST BOZORTH3 minutiae-matching algorithm.

use super::BiometricMinutia;
pub use fprint_bozorth3::MIN_COMPUTABLE_BOZORTH_MINUTIAE;

/// Compares two sets of canonical [`BiometricMinutia`] using NIST BOZORTH3.
///
/// Returns an integer match score where higher values denote greater structural similarity.
/// Typically, scores above 40 indicate a likely match in NIST 1:1 benchmarks.
///
/// If either set has fewer minutiae than [`MIN_COMPUTABLE_BOZORTH_MINUTIAE`] (typically 10),
/// 0 is returned because Bozorth3 requires a minimal topological constellation.
pub fn match_score(probe: &[BiometricMinutia], gallery: &[BiometricMinutia]) -> u32 {
    if probe.len() < MIN_COMPUTABLE_BOZORTH_MINUTIAE
        || gallery.len() < MIN_COMPUTABLE_BOZORTH_MINUTIAE
    {
        return 0;
    }

    let bozorth_probe: Vec<fprint_bozorth3::Minutia> = probe.iter().map(Into::into).collect();
    let bozorth_gallery: Vec<fprint_bozorth3::Minutia> = gallery.iter().map(Into::into).collect();

    fprint_bozorth3::match_score(&bozorth_probe, &bozorth_gallery)
}
