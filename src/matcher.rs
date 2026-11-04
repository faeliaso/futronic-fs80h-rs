//! Local minutiae-based fingerprint verification and identification.
//!
//! The matcher compares fingerprint minutiae, never raw image pixels. Its scores are useful for
//! application-specific decisions, but must be calibrated with representative biometric data
//! before use in a security-sensitive workflow. It does not perform liveness detection.

#[cfg(feature = "ansi-378")]
use crate::AnsiTemplate;
use crate::{
    FingerPosition, IsoTemplate, Minutia, MinutiaKind, RawFrame, TemplateError, TemplateOptions,
};
use thiserror::Error;

const SEED_MINUTIAE: usize = 16;
const POSITION_TOLERANCE: f32 = 16.0;
const ANGLE_TOLERANCE: u8 = 24;

/// A parsed template that can be compared by [`Matcher`].
pub trait FingerprintTemplate {
    /// Returns the fingerprint minutiae used by the matcher.
    fn minutiae(&self) -> &[Minutia];

    /// Returns the recorded finger position, if known.
    fn finger_position(&self) -> FingerPosition;
}

impl FingerprintTemplate for IsoTemplate {
    fn minutiae(&self) -> &[Minutia] {
        self.minutiae()
    }

    fn finger_position(&self) -> FingerPosition {
        self.finger_position()
    }
}

#[cfg(feature = "ansi-378")]
impl FingerprintTemplate for AnsiTemplate {
    fn minutiae(&self) -> &[Minutia] {
        self.minutiae()
    }

    fn finger_position(&self) -> FingerPosition {
        self.finger_position()
    }
}

/// Configuration for a fingerprint matcher.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatcherOptions {
    /// The inclusive score required for a positive match, from `0.0` through `1.0`.
    pub threshold: f32,
}

/// Result of a single fingerprint comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchResult {
    /// Similarity score normalized to the inclusive range `0.0..=1.0`.
    pub score: f32,
    /// Whether `score` meets the matcher's configured threshold.
    pub is_match: bool,
    /// Number of one-to-one geometrically consistent minutiae correspondences.
    pub matched_minutiae: usize,
}

/// A ranked 1:N candidate returned by [`Matcher::identify`].
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateMatch<Id> {
    /// The application-provided candidate identifier.
    pub id: Id,
    /// The candidate's comparison result.
    pub result: MatchResult,
}

/// Failures from matcher setup or source conversion.
#[derive(Debug, Error)]
pub enum MatcherError {
    /// The configured threshold is not a finite value in the inclusive range `0.0..=1.0`.
    #[error("invalid matcher threshold {0}: must be finite and between 0.0 and 1.0")]
    InvalidThreshold(f32),
    /// A frame could not be converted into a quality-gated fingerprint template.
    #[error(transparent)]
    Template(#[from] TemplateError),
    /// A WSQ source could not be decoded.
    #[cfg(feature = "wsq")]
    #[error(transparent)]
    Wsq(#[from] crate::WsqError),
    /// A Base64 source is invalid.
    #[cfg(feature = "matcher-base64")]
    #[error("invalid Base64 fingerprint source: {0}")]
    Base64(#[from] base64::DecodeError),
}

/// A local, deterministic minutiae matcher.
#[derive(Debug, Clone, Copy)]
pub struct Matcher {
    options: MatcherOptions,
}

impl Matcher {
    /// Creates a matcher with an application-calibrated decision threshold.
    pub fn new(options: MatcherOptions) -> Result<Self, MatcherError> {
        if !options.threshold.is_finite() || !(0.0..=1.0).contains(&options.threshold) {
            return Err(MatcherError::InvalidThreshold(options.threshold));
        }
        Ok(Self { options })
    }

    /// Returns this matcher's configuration.
    #[must_use]
    pub const fn options(&self) -> MatcherOptions {
        self.options
    }

    /// Verifies two parsed ISO or ANSI fingerprint templates.
    pub fn verify<E, P>(&self, enrolled: &E, probe: &P) -> Result<MatchResult, MatcherError>
    where
        E: FingerprintTemplate + ?Sized,
        P: FingerprintTemplate + ?Sized,
    {
        if conflicting_positions(enrolled.finger_position(), probe.finger_position()) {
            return Ok(self.result(0.0, 0));
        }
        let (score, matched_minutiae) = best_alignment(enrolled.minutiae(), probe.minutiae());
        Ok(self.result(score, matched_minutiae))
    }

    /// Extracts an ISO template from a live frame and verifies it against an enrolled template.
    pub fn verify_frame<E>(
        &self,
        enrolled: &E,
        probe: &RawFrame,
        extraction: &TemplateOptions,
    ) -> Result<MatchResult, MatcherError>
    where
        E: FingerprintTemplate + ?Sized,
    {
        let probe = probe.extract_iso_template(extraction)?;
        self.verify(enrolled, &probe)
    }

    /// Decodes a WSQ fingerprint image, extracts its ISO template, and verifies it.
    #[cfg(feature = "wsq")]
    pub fn verify_wsq<E>(
        &self,
        enrolled: &E,
        probe_wsq: &[u8],
        extraction: &TemplateOptions,
    ) -> Result<MatchResult, MatcherError>
    where
        E: FingerprintTemplate + ?Sized,
    {
        let frame = RawFrame::from_wsq(probe_wsq)?;
        self.verify_frame(enrolled, &frame, extraction)
    }

    /// Finds all candidates whose score reaches the configured threshold, in descending score order.
    pub fn identify<'a, Id, T, I>(
        &self,
        probe: &T,
        candidates: I,
    ) -> Result<Vec<CandidateMatch<Id>>, MatcherError>
    where
        T: FingerprintTemplate + ?Sized + 'a,
        I: IntoIterator<Item = (Id, &'a T)>,
    {
        let mut matches = Vec::new();
        for (id, candidate) in candidates {
            let result = self.verify(candidate, probe)?;
            if result.is_match {
                matches.push(CandidateMatch { id, result });
            }
        }
        matches.sort_by(|left, right| right.result.score.total_cmp(&left.result.score));
        Ok(matches)
    }

    /// Decodes and verifies two Base64-encoded ISO/IEC 19794-2 records.
    #[cfg(feature = "matcher-base64")]
    pub fn verify_iso_base64(
        &self,
        enrolled: &str,
        probe: &str,
    ) -> Result<MatchResult, MatcherError> {
        let enrolled = IsoTemplate::from_bytes(&decode_base64(enrolled)?)?;
        let probe = IsoTemplate::from_bytes(&decode_base64(probe)?)?;
        self.verify(&enrolled, &probe)
    }

    /// Decodes and verifies two Base64-encoded ANSI INCITS 378 records.
    #[cfg(all(feature = "matcher-base64", feature = "ansi-378"))]
    pub fn verify_ansi_base64(
        &self,
        enrolled: &str,
        probe: &str,
    ) -> Result<MatchResult, MatcherError> {
        let enrolled = AnsiTemplate::from_bytes(&decode_base64(enrolled)?)?;
        let probe = AnsiTemplate::from_bytes(&decode_base64(probe)?)?;
        self.verify(&enrolled, &probe)
    }

    /// Decodes Base64 WSQ bytes, extracts an ISO template, and verifies it.
    #[cfg(all(feature = "matcher-base64", feature = "wsq"))]
    pub fn verify_wsq_base64<E>(
        &self,
        enrolled: &E,
        probe_wsq: &str,
        extraction: &TemplateOptions,
    ) -> Result<MatchResult, MatcherError>
    where
        E: FingerprintTemplate + ?Sized,
    {
        self.verify_wsq(enrolled, &decode_base64(probe_wsq)?, extraction)
    }

    fn result(&self, score: f32, matched_minutiae: usize) -> MatchResult {
        MatchResult {
            score,
            is_match: score >= self.options.threshold,
            matched_minutiae,
        }
    }
}

#[cfg(feature = "matcher-base64")]
fn decode_base64(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    STANDARD.decode(value)
}

fn conflicting_positions(left: FingerPosition, right: FingerPosition) -> bool {
    left != FingerPosition::Unknown && right != FingerPosition::Unknown && left != right
}

fn best_alignment(enrolled: &[Minutia], probe: &[Minutia]) -> (f32, usize) {
    if enrolled.is_empty() || probe.is_empty() {
        return (0.0, 0);
    }
    let mut best = (0.0, 0);
    let enrolled_seeds = best_quality_indices(enrolled);
    let probe_seeds = best_quality_indices(probe);
    for &source_index in &enrolled_seeds {
        for &target_index in &probe_seeds {
            if !compatible_kind(enrolled[source_index].kind, probe[target_index].kind) {
                continue;
            }
            let source = enrolled[source_index];
            let target = probe[target_index];
            let rotation =
                angle_delta(target.angle, source.angle) as f32 * core::f32::consts::TAU / 256.0;
            let (sin, cos) = rotation.sin_cos();
            let translated_x =
                f32::from(target.x) - (f32::from(source.x) * cos - f32::from(source.y) * sin);
            let translated_y =
                f32::from(target.y) - (f32::from(source.x) * sin + f32::from(source.y) * cos);
            let candidate = score_alignment(
                enrolled,
                probe,
                cos,
                sin,
                translated_x,
                translated_y,
                rotation,
            );
            if candidate.0 > best.0 || (candidate.0 == best.0 && candidate.1 > best.1) {
                best = candidate;
            }
        }
    }
    best
}

fn best_quality_indices(minutiae: &[Minutia]) -> Vec<usize> {
    let mut indices = quality_indices(minutiae);
    indices.truncate(SEED_MINUTIAE);
    indices
}

fn quality_indices(minutiae: &[Minutia]) -> Vec<usize> {
    let mut indices: Vec<_> = (0..minutiae.len()).collect();
    indices.sort_by_key(|&index| std::cmp::Reverse(minutiae[index].quality));
    indices
}

fn score_alignment(
    enrolled: &[Minutia],
    probe: &[Minutia],
    cos: f32,
    sin: f32,
    translate_x: f32,
    translate_y: f32,
    rotation: f32,
) -> (f32, usize) {
    let mut used_probe = vec![false; probe.len()];
    let mut matches = 0usize;
    let mut quality_sum = 0.0f32;
    for source_index in quality_indices(enrolled) {
        let source = enrolled[source_index];
        let x = f32::from(source.x) * cos - f32::from(source.y) * sin + translate_x;
        let y = f32::from(source.x) * sin + f32::from(source.y) * cos + translate_y;
        let rotation_units = (rotation * 256.0 / core::f32::consts::TAU).round() as i16;
        let expected_angle = (i16::from(source.angle) + rotation_units).rem_euclid(256) as u8;
        let mut nearest: Option<(usize, f32)> = None;
        for (index, target) in probe.iter().enumerate() {
            if used_probe[index]
                || !compatible_kind(source.kind, target.kind)
                || circular_angle_distance(expected_angle, target.angle) > ANGLE_TOLERANCE
            {
                continue;
            }
            let distance = (x - f32::from(target.x)).hypot(y - f32::from(target.y));
            if distance <= POSITION_TOLERANCE
                && nearest.is_none_or(|(_, best_distance)| distance < best_distance)
            {
                nearest = Some((index, distance));
            }
        }
        if let Some((index, _)) = nearest {
            used_probe[index] = true;
            matches += 1;
            quality_sum += f32::from(source.quality.min(probe[index].quality)) / 100.0;
        }
    }
    if matches == 0 {
        return (0.0, 0);
    }
    let quality = quality_sum / matches as f32;
    let coverage =
        ((matches as f32 / enrolled.len() as f32) * (matches as f32 / probe.len() as f32)).sqrt();
    ((quality * coverage).clamp(0.0, 1.0), matches)
}

fn compatible_kind(left: MinutiaKind, right: MinutiaKind) -> bool {
    left == MinutiaKind::Other || right == MinutiaKind::Other || left == right
}

fn angle_delta(left: u8, right: u8) -> i16 {
    let difference = i16::from(left) - i16::from(right);
    (difference + 128).rem_euclid(256) - 128
}

fn circular_angle_distance(left: u8, right: u8) -> u8 {
    angle_delta(left, right).unsigned_abs() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestTemplate {
        minutiae: Vec<Minutia>,
        position: FingerPosition,
    }

    impl FingerprintTemplate for TestTemplate {
        fn minutiae(&self) -> &[Minutia] {
            &self.minutiae
        }
        fn finger_position(&self) -> FingerPosition {
            self.position
        }
    }

    fn template(offset_x: u16, offset_y: u16) -> TestTemplate {
        TestTemplate {
            minutiae: vec![
                Minutia {
                    x: 40 + offset_x,
                    y: 60 + offset_y,
                    kind: MinutiaKind::RidgeEnding,
                    angle: 12,
                    quality: 100,
                },
                Minutia {
                    x: 100 + offset_x,
                    y: 90 + offset_y,
                    kind: MinutiaKind::RidgeBifurcation,
                    angle: 44,
                    quality: 95,
                },
                Minutia {
                    x: 70 + offset_x,
                    y: 160 + offset_y,
                    kind: MinutiaKind::RidgeEnding,
                    angle: 110,
                    quality: 90,
                },
                Minutia {
                    x: 150 + offset_x,
                    y: 180 + offset_y,
                    kind: MinutiaKind::RidgeBifurcation,
                    angle: 180,
                    quality: 90,
                },
            ],
            position: FingerPosition::RightIndex,
        }
    }

    #[test]
    fn verifies_translated_minutiae() {
        let matcher = Matcher::new(MatcherOptions { threshold: 0.7 }).unwrap();
        let result = matcher.verify(&template(0, 0), &template(11, 8)).unwrap();
        assert!(result.is_match);
        assert_eq!(result.matched_minutiae, 4);
        assert!(result.score > 0.9);
    }

    #[test]
    fn verifies_rotated_minutiae() {
        let matcher = Matcher::new(MatcherOptions { threshold: 0.7 }).unwrap();
        let enrolled = template(0, 0);
        let probe = TestTemplate {
            minutiae: enrolled
                .minutiae
                .iter()
                .map(|m| Minutia {
                    x: 480 - m.y,
                    y: m.x,
                    kind: m.kind,
                    angle: m.angle.wrapping_add(64),
                    quality: m.quality,
                })
                .collect(),
            position: enrolled.position,
        };
        let result = matcher.verify(&enrolled, &probe).unwrap();
        assert!(result.is_match);
        assert_eq!(result.matched_minutiae, 4);
    }

    #[test]
    fn scores_every_minutia_not_only_alignment_seeds() {
        let matcher = Matcher::new(MatcherOptions { threshold: 0.9 }).unwrap();
        let mut enrolled = template(0, 0);
        for index in 0..20 {
            enrolled.minutiae.push(Minutia {
                x: 180 + index * 3,
                y: 220 + index * 4,
                kind: MinutiaKind::RidgeEnding,
                angle: index as u8 * 7,
                quality: 90,
            });
        }
        let mut probe = enrolled.clone();
        for minutia in &mut probe.minutiae {
            minutia.x += 7;
            minutia.y += 5;
        }
        let result = matcher.verify(&enrolled, &probe).unwrap();
        assert!(result.is_match);
        assert_eq!(result.matched_minutiae, enrolled.minutiae.len());
    }

    #[test]
    fn rejects_conflicting_known_finger_positions() {
        let matcher = Matcher::new(MatcherOptions { threshold: 0.1 }).unwrap();
        let mut probe = template(0, 0);
        probe.position = FingerPosition::LeftIndex;
        assert_eq!(matcher.verify(&template(0, 0), &probe).unwrap().score, 0.0);
    }

    #[test]
    fn identify_returns_ranked_matches() {
        let matcher = Matcher::new(MatcherOptions { threshold: 0.6 }).unwrap();
        let enrolled = template(0, 0);
        let close = template(5, 4);
        let mut weak = template(0, 0);
        weak.minutiae.truncate(1);
        let found = matcher
            .identify(&enrolled, [("close", &close), ("weak", &weak)])
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "close");
    }

    #[test]
    fn rejects_invalid_thresholds() {
        assert!(matches!(
            Matcher::new(MatcherOptions { threshold: 1.1 }),
            Err(MatcherError::InvalidThreshold(_))
        ));
    }

    #[cfg(feature = "matcher-base64")]
    #[test]
    fn verifies_explicit_base64_iso_records() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};

        let mut bytes = vec![0_u8; 36];
        bytes[..4].copy_from_slice(b"FMR\0");
        bytes[4..8].copy_from_slice(b" 20\0");
        bytes[8..12].copy_from_slice(&(36_u32).to_be_bytes());
        bytes[22] = 1;
        bytes[24] = FingerPosition::RightIndex as u8;
        bytes[27] = 1;
        bytes[28..30].copy_from_slice(&(50_u16).to_be_bytes());
        bytes[30..32].copy_from_slice(&(60_u16).to_be_bytes());
        bytes[32] = 20;
        bytes[33] = 100;
        let encoded = STANDARD.encode(bytes);

        let matcher = Matcher::new(MatcherOptions { threshold: 0.9 }).unwrap();
        let result = matcher.verify_iso_base64(&encoded, &encoded).unwrap();
        assert!(result.is_match);
        assert_eq!(result.matched_minutiae, 1);
    }
}
