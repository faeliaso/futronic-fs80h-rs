//! ISO/IEC 19794-2:2011 minutiae templates.
//!
//! The binary layout implemented here is the normal, off-card representation. It deliberately
//! stores no image data. Singular points are estimated from MINDTCT's orientation map and are
//! therefore optional and experimental.

use crate::RawFrame;
use fprint_mindtct::{debug_maps, debug_removed_minutiae, detect_minutiae, GrayImage};
use std::collections::HashMap;
use thiserror::Error;

#[cfg(feature = "ansi-378")]
pub mod ansi;

const WIDTH: usize = 320;
const HEIGHT: usize = 480;
const DPI: u16 = 500;
const RESOLUTION_PPCM: u16 = 197;

/// Interoperable normal/off-card ISO fingerprint-minutiae encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoFormat {
    /// ISO/IEC 19794-2:2005 with Technical Corrigendum 1:2009.
    Iso2005,
    /// ISO/IEC 19794-2:2011 with Technical Corrigendum 1:2012.
    Iso2011,
}

/// Options controlling quality gating and singular-point estimation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateOptions {
    /// Minimum retained minutiae required to accept a capture.
    pub min_minutiae: usize,
    /// Minimum MINDTCT reliability required for an individual minutia.
    pub min_minutia_quality: u8,
    /// Minimum mean quality of retained minutiae required to accept a capture.
    pub min_mean_quality: u8,
    /// Minimum 0-100 confidence required to include an estimated core or delta.
    pub singular_point_confidence: u8,
    /// Finger position to encode in the ISO record.
    pub finger_position: FingerPosition,
}

impl Default for TemplateOptions {
    fn default() -> Self {
        Self {
            min_minutiae: 20,
            min_minutia_quality: 20,
            min_mean_quality: 40,
            singular_point_confidence: 75,
            finger_position: FingerPosition::Unknown,
        }
    }
}

/// ISO finger-position values supported by a single-finger capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FingerPosition {
    /// The captured finger is not identified.
    Unknown = 0,
    RightThumb = 1,
    RightIndex = 2,
    RightMiddle = 3,
    RightRing = 4,
    RightLittle = 5,
    LeftThumb = 6,
    LeftIndex = 7,
    LeftMiddle = 8,
    LeftRing = 9,
    LeftLittle = 10,
}

/// Type of a fingerprint minutia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinutiaKind {
    /// Ridge ending.
    RidgeEnding,
    /// Ridge bifurcation.
    RidgeBifurcation,
    /// A type that cannot be reliably classified.
    Other,
}

impl MinutiaKind {
    fn bits(self) -> u16 {
        match self {
            Self::Other => 0,
            Self::RidgeEnding => 1,
            Self::RidgeBifurcation => 2,
        }
    }
    fn from_bits(bits: u16) -> Result<Self, TemplateError> {
        match bits {
            0 => Ok(Self::Other),
            1 => Ok(Self::RidgeEnding),
            2 => Ok(Self::RidgeBifurcation),
            _ => Err(TemplateError::InvalidTemplate("reserved minutia type")),
        }
    }
}

/// One minutia in top-left image coordinates and ISO angle units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minutia {
    /// Horizontal pixel coordinate.
    pub x: u16,
    /// Vertical pixel coordinate.
    pub y: u16,
    /// Ridge ending, bifurcation, or other.
    pub kind: MinutiaKind,
    /// Quantized counter-clockwise orientation; one unit is 360/256 degrees.
    pub angle: u8,
    /// MINDTCT reliability, 0 through 100.
    pub quality: u8,
}

/// Type of optional singular point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingularPointKind {
    /// Fingerprint core.
    Core,
    /// Fingerprint delta.
    Delta,
}

/// An experimental core or delta estimate in top-left image coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingularPoint {
    /// Core or delta.
    pub kind: SingularPointKind,
    /// Horizontal pixel coordinate.
    pub x: u16,
    /// Vertical pixel coordinate.
    pub y: u16,
    /// Estimator confidence on a 0-100 scale.
    pub confidence: u8,
}

/// Extracted template and its ISO/IEC 19794-2 binary representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoTemplate {
    bytes: Vec<u8>,
    minutiae: Vec<Minutia>,
    singular_points: Vec<SingularPoint>,
    mean_quality: u8,
    finger_position: FingerPosition,
    format: IsoFormat,
}

impl IsoTemplate {
    /// Returns the complete ISO/IEC 19794-2 normal off-card record.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consumes the template and returns its binary record.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Returns extracted minutiae.
    #[must_use]
    pub fn minutiae(&self) -> &[Minutia] {
        &self.minutiae
    }
    /// Returns experimental core/delta estimates that met the confidence threshold.
    #[must_use]
    pub fn singular_points(&self) -> &[SingularPoint] {
        &self.singular_points
    }
    /// Returns mean quality of the minutiae retained in this template.
    #[must_use]
    pub const fn mean_quality(&self) -> u8 {
        self.mean_quality
    }
    /// Returns the ISO revision used by this record.
    #[must_use]
    pub const fn format(&self) -> IsoFormat {
        self.format
    }
    /// Validates and parses records emitted by this module.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TemplateError> {
        parse(bytes)
    }
}

/// Template extraction and serialization failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The detector rejected the image supplied by the frame abstraction.
    #[error("fingerprint image is not suitable for minutiae extraction: {0}")]
    Image(String),
    /// The capture does not satisfy the selected enrollment threshold.
    #[error("fingerprint quality too low: retained {retained} minutiae with mean quality {mean_quality}; need at least {minimum_minutiae} and {minimum_quality}")]
    InsufficientQuality {
        retained: usize,
        mean_quality: u8,
        minimum_minutiae: usize,
        minimum_quality: u8,
    },
    /// The ISO record is malformed or outside the supported profile.
    #[error("invalid ISO/IEC 19794-2 template: {0}")]
    InvalidTemplate(&'static str),
    /// The ANSI INCITS 378 record is malformed or outside the supported one-finger profile.
    #[error("invalid ANSI INCITS 378 template: {0}")]
    InvalidAnsiTemplate(&'static str),
}

pub(crate) fn extract(
    frame: &RawFrame,
    options: &TemplateOptions,
) -> Result<IsoTemplate, TemplateError> {
    extract_as(frame, IsoFormat::Iso2011, options)
}

/// Extracts and encodes a template in the requested ISO revision.
pub(crate) fn extract_as(
    frame: &RawFrame,
    format: IsoFormat,
    options: &TemplateOptions,
) -> Result<IsoTemplate, TemplateError> {
    let inverted = frame.inverted();
    let image = GrayImage::new(inverted.as_bytes(), WIDTH, HEIGHT, DPI)
        .map_err(|error| TemplateError::Image(error.to_string()))?;
    let kinds: HashMap<(i32, i32), MinutiaKind> = debug_removed_minutiae(image)
        .into_iter()
        .map(|raw| {
            (
                (raw.x, HEIGHT as i32 - raw.y),
                if raw.kind == 0 {
                    MinutiaKind::RidgeBifurcation
                } else {
                    MinutiaKind::RidgeEnding
                },
            )
        })
        .collect();
    let mut minutiae: Vec<Minutia> = detect_minutiae(image)
        .into_iter()
        .filter(|m| m.quality >= i32::from(options.min_minutia_quality))
        .filter_map(|m| {
            let x = u16::try_from(m.x).ok()?;
            let y = u16::try_from(HEIGHT as i32 - m.y).ok()?;
            Some(Minutia {
                x,
                y,
                kind: kinds
                    .get(&(m.x, m.y))
                    .copied()
                    .unwrap_or(MinutiaKind::Other),
                angle: quantize_angle(m.theta),
                quality: m.quality.clamp(0, 100) as u8,
            })
        })
        .collect();
    minutiae.sort_by_key(|minutia| std::cmp::Reverse(minutia.quality));
    minutiae.truncate(u8::MAX as usize);
    let mean = mean_quality(&minutiae);
    if minutiae.len() < options.min_minutiae || mean < options.min_mean_quality {
        return Err(TemplateError::InsufficientQuality {
            retained: minutiae.len(),
            mean_quality: mean,
            minimum_minutiae: options.min_minutiae,
            minimum_quality: options.min_mean_quality,
        });
    }
    let points = estimate_singular_points(debug_maps(image), options.singular_point_confidence);
    let bytes = match format {
        IsoFormat::Iso2005 => serialize_2005(&minutiae, &points, mean, options.finger_position),
        IsoFormat::Iso2011 => serialize_2011(&minutiae, &points, mean, options.finger_position),
    };
    Ok(IsoTemplate {
        bytes,
        minutiae,
        singular_points: points,
        mean_quality: mean,
        finger_position: options.finger_position,
        format,
    })
}

fn quantize_angle(theta: i32) -> u8 {
    ((theta.rem_euclid(360) * 256 + 180) / 360) as u8
}
fn mean_quality(minutiae: &[Minutia]) -> u8 {
    if minutiae.is_empty() {
        0
    } else {
        (minutiae
            .iter()
            .map(|m| usize::from(m.quality))
            .sum::<usize>()
            / minutiae.len()) as u8
    }
}

fn estimate_singular_points(maps: fprint_mindtct::DebugMaps, threshold: u8) -> Vec<SingularPoint> {
    let mut candidates = Vec::new();
    if maps.map_w < 3 || maps.map_h < 3 {
        return candidates;
    }
    for y in 1..maps.map_h - 1 {
        for x in 1..maps.map_w - 1 {
            let ring = [
                (x - 1, y - 1),
                (x, y - 1),
                (x + 1, y - 1),
                (x + 1, y),
                (x + 1, y + 1),
                (x, y + 1),
                (x - 1, y + 1),
                (x - 1, y),
            ];
            let dirs: Option<Vec<i32>> = ring
                .iter()
                .map(|&(a, b)| {
                    let i = b * maps.map_w + a;
                    (maps.direction_map[i] >= 0
                        && maps.low_flow_map[i] == 0
                        && maps.low_contrast_map[i] == 0)
                        .then_some(maps.direction_map[i])
                })
                .collect();
            let Some(dirs) = dirs else {
                continue;
            };
            let mut winding = 0;
            for i in 0..dirs.len() {
                let mut d = dirs[(i + 1) % dirs.len()] - dirs[i];
                while d > 8 {
                    d -= 16;
                }
                while d < -8 {
                    d += 16;
                }
                winding += d;
            }
            let confidence = ((winding.unsigned_abs() * 100) / 16).min(100) as u8;
            let kind = if winding >= 12 {
                Some(SingularPointKind::Core)
            } else if winding <= -12 {
                Some(SingularPointKind::Delta)
            } else {
                None
            };
            if let Some(kind) = kind.filter(|_| confidence >= threshold) {
                candidates.push(SingularPoint {
                    kind,
                    x: ((x * WIDTH) / maps.map_w) as u16,
                    y: ((y * HEIGHT) / maps.map_h) as u16,
                    confidence,
                });
            }
        }
    }
    candidates.sort_by_key(|point| std::cmp::Reverse(point.confidence));
    let mut out = Vec::new();
    for p in candidates {
        if out
            .iter()
            .filter(|q: &&SingularPoint| q.kind == p.kind)
            .count()
            >= if p.kind == SingularPointKind::Core {
                1
            } else {
                2
            }
        {
            continue;
        }
        if out.iter().all(|q: &SingularPoint| {
            i32::from(q.x).abs_diff(i32::from(p.x)) > 48
                || i32::from(q.y).abs_diff(i32::from(p.y)) > 48
        }) {
            out.push(p);
        }
    }
    out
}

fn serialize_2011(
    minutiae: &[Minutia],
    points: &[SingularPoint],
    mean: u8,
    position: FingerPosition,
) -> Vec<u8> {
    let mut extensions = Vec::new();
    if !points.is_empty() {
        let mut data = Vec::new();
        let cores: Vec<_> = points
            .iter()
            .filter(|p| p.kind == SingularPointKind::Core)
            .collect();
        data.push(cores.len() as u8);
        for p in cores {
            point(&mut data, p);
        }
        let deltas: Vec<_> = points
            .iter()
            .filter(|p| p.kind == SingularPointKind::Delta)
            .collect();
        data.push(deltas.len() as u8);
        for p in deltas {
            point(&mut data, p);
        }
        put_u16(&mut extensions, 2);
        put_u16(&mut extensions, (data.len() + 4) as u16);
        extensions.extend(data);
    }
    let fp_len = 34 + minutiae.len() * 6 + extensions.len();
    let total = 15 + fp_len;
    let mut out = Vec::with_capacity(total);
    out.extend(*b"FMR\0");
    out.extend(*b"030\0");
    put_u32(&mut out, total as u32);
    put_u16(&mut out, 1);
    out.push(0);
    put_u32(&mut out, fp_len as u32);
    out.extend([0xff; 9]);
    out.extend([1, 0, 0, 0, 0]);
    out.push(0);
    out.push(position as u8);
    out.push(0);
    put_u16(&mut out, RESOLUTION_PPCM);
    put_u16(&mut out, RESOLUTION_PPCM);
    out.push(0);
    put_u16(&mut out, WIDTH as u16);
    put_u16(&mut out, HEIGHT as u16);
    out.push(0x61);
    out.push(minutiae.len() as u8);
    for m in minutiae {
        put_u16(&mut out, (m.kind.bits() << 14) | m.x);
        put_u16(&mut out, m.y);
        out.push(m.angle);
        out.push(m.quality);
    }
    put_u16(&mut out, extensions.len() as u16);
    out.extend(extensions);
    debug_assert_eq!(out.len(), total);
    let _ = mean;
    out
}

fn serialize_2005(
    minutiae: &[Minutia],
    points: &[SingularPoint],
    mean: u8,
    position: FingerPosition,
) -> Vec<u8> {
    let mut extensions = Vec::new();
    if !points.is_empty() {
        let mut data = Vec::new();
        let cores: Vec<_> = points
            .iter()
            .filter(|p| p.kind == SingularPointKind::Core)
            .collect();
        data.push(cores.len() as u8);
        for point_value in cores {
            point(&mut data, point_value);
        }
        let deltas: Vec<_> = points
            .iter()
            .filter(|p| p.kind == SingularPointKind::Delta)
            .collect();
        data.push(deltas.len() as u8);
        for point_value in deltas {
            point(&mut data, point_value);
        }
        put_u16(&mut extensions, 2);
        put_u16(&mut extensions, data.len() as u16); // 2005 EXTLEN excludes extension header.
        extensions.extend(data);
    }
    let total = 30 + minutiae.len() * 6 + extensions.len();
    let mut out = Vec::with_capacity(total);
    out.extend(*b"FMR\0");
    out.extend(*b" 20\0");
    put_u32(&mut out, total as u32);
    put_u16(&mut out, 0); // DEVSTAMP + DEVID: unknown/not certified.
    put_u16(&mut out, WIDTH as u16);
    put_u16(&mut out, HEIGHT as u16);
    put_u16(&mut out, RESOLUTION_PPCM);
    put_u16(&mut out, RESOLUTION_PPCM);
    out.extend([1, 0]); // one finger, reserved byte
    out.push(position as u8);
    out.push(0); // view 0, live/plain
    out.push(mean);
    out.push(minutiae.len() as u8);
    for m in minutiae {
        put_u16(&mut out, (m.kind.bits() << 14) | m.x);
        put_u16(&mut out, m.y);
        out.push(m.angle);
        out.push(m.quality);
    }
    put_u16(&mut out, extensions.len() as u16);
    out.extend(extensions);
    debug_assert_eq!(out.len(), total);
    out
}
fn point(out: &mut Vec<u8>, p: &SingularPoint) {
    put_u16(out, p.x & 0x3fff);
    put_u16(out, p.y & 0x3fff);
}
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_be_bytes())
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_be_bytes())
}

fn parse(bytes: &[u8]) -> Result<IsoTemplate, TemplateError> {
    if bytes.len() < 8 || &bytes[..4] != b"FMR\0" {
        return Err(TemplateError::InvalidTemplate("unsupported header"));
    }
    if &bytes[4..8] == b" 20\0" {
        return parse_2005(bytes);
    }
    if &bytes[4..8] != b"030\0" || bytes.len() < 54 {
        return Err(TemplateError::InvalidTemplate("unsupported header"));
    }
    let total = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if total != bytes.len()
        || u16::from_be_bytes(bytes[12..14].try_into().unwrap()) != 1
        || bytes[14] != 0
    {
        return Err(TemplateError::InvalidTemplate(
            "unsupported record count or certification",
        ));
    }
    let fp = u32::from_be_bytes(bytes[15..19].try_into().unwrap()) as usize;
    if fp + 15 != total {
        return Err(TemplateError::InvalidTemplate("invalid fingerprint length"));
    }
    let mut i = 19 + 9 + 1 + 2 + 2;
    let qcount = bytes[i];
    i += 1 + usize::from(qcount) * 5;
    if i + 14 > bytes.len() {
        return Err(TemplateError::InvalidTemplate(
            "truncated fingerprint header",
        ));
    }
    let pos = match bytes[i] {
        0 => FingerPosition::Unknown,
        1 => FingerPosition::RightThumb,
        2 => FingerPosition::RightIndex,
        3 => FingerPosition::RightMiddle,
        4 => FingerPosition::RightRing,
        5 => FingerPosition::RightLittle,
        6 => FingerPosition::LeftThumb,
        7 => FingerPosition::LeftIndex,
        8 => FingerPosition::LeftMiddle,
        9 => FingerPosition::LeftRing,
        10 => FingerPosition::LeftLittle,
        _ => {
            return Err(TemplateError::InvalidTemplate(
                "unsupported finger position",
            ))
        }
    };
    i += 1 + 1 + 2 + 2 + 1 + 2 + 2;
    if bytes[i] != 0x61 {
        return Err(TemplateError::InvalidTemplate("unsupported minutia layout"));
    }
    i += 1;
    let count = usize::from(bytes[i]);
    i += 1;
    if count == 0 || i + count * 6 + 2 > bytes.len() {
        return Err(TemplateError::InvalidTemplate("truncated minutiae"));
    }
    let mut ms = Vec::with_capacity(count);
    for _ in 0..count {
        let a = u16::from_be_bytes(bytes[i..i + 2].try_into().unwrap());
        let y = u16::from_be_bytes(bytes[i + 2..i + 4].try_into().unwrap());
        ms.push(Minutia {
            x: a & 0x3fff,
            y: y & 0x3fff,
            kind: MinutiaKind::from_bits(a >> 14)?,
            angle: bytes[i + 4],
            quality: bytes[i + 5],
        });
        i += 6
    }
    let ext = usize::from(u16::from_be_bytes(bytes[i..i + 2].try_into().unwrap()));
    i += 2;
    if i + ext != bytes.len() {
        return Err(TemplateError::InvalidTemplate("invalid extension length"));
    }
    let mut points = Vec::new();
    if ext > 0 {
        if ext < 6 || bytes[i..i + 2] != [0, 2] {
            return Err(TemplateError::InvalidTemplate("unsupported extension"));
        }
        let len = usize::from(u16::from_be_bytes(bytes[i + 2..i + 4].try_into().unwrap()));
        if len != ext {
            return Err(TemplateError::InvalidTemplate(
                "invalid core/delta extension",
            ));
        }
        let mut j = i + 4;
        let nc = usize::from(bytes[j]);
        j += 1;
        for _ in 0..nc {
            points.push(SingularPoint {
                kind: SingularPointKind::Core,
                x: u16::from_be_bytes(bytes[j..j + 2].try_into().unwrap()) & 0x3fff,
                y: u16::from_be_bytes(bytes[j + 2..j + 4].try_into().unwrap()) & 0x3fff,
                confidence: 100,
            });
            j += 4
        }
        let nd = usize::from(bytes[j]);
        j += 1;
        for _ in 0..nd {
            points.push(SingularPoint {
                kind: SingularPointKind::Delta,
                x: u16::from_be_bytes(bytes[j..j + 2].try_into().unwrap()) & 0x3fff,
                y: u16::from_be_bytes(bytes[j + 2..j + 4].try_into().unwrap()) & 0x3fff,
                confidence: 100,
            });
            j += 4
        }
        if j != bytes.len() {
            return Err(TemplateError::InvalidTemplate("trailing extension data"));
        }
    }
    let mean = mean_quality(&ms);
    Ok(IsoTemplate {
        bytes: bytes.to_vec(),
        minutiae: ms,
        singular_points: points,
        mean_quality: mean,
        finger_position: pos,
        format: IsoFormat::Iso2011,
    })
}

fn parse_2005(bytes: &[u8]) -> Result<IsoTemplate, TemplateError> {
    if bytes.len() < 30 {
        return Err(TemplateError::InvalidTemplate("truncated ISO 2005 record"));
    }
    let total = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if total != bytes.len() || bytes[22] != 1 || bytes[23] != 0 {
        return Err(TemplateError::InvalidTemplate(
            "unsupported ISO 2005 record",
        ));
    }
    let pos = finger_position(bytes[24])?;
    let count = usize::from(bytes[27]);
    if 28 + count * 6 + 2 != bytes.len() {
        return Err(TemplateError::InvalidTemplate(
            "invalid ISO 2005 minutiae length",
        ));
    }
    let mut minutiae = Vec::with_capacity(count);
    let mut i = 28;
    for _ in 0..count {
        let a = u16::from_be_bytes(bytes[i..i + 2].try_into().unwrap());
        minutiae.push(Minutia {
            x: a & 0x3fff,
            y: u16::from_be_bytes(bytes[i + 2..i + 4].try_into().unwrap()) & 0x3fff,
            kind: MinutiaKind::from_bits(a >> 14)?,
            angle: bytes[i + 4],
            quality: bytes[i + 5],
        });
        i += 6;
    }
    let ext = usize::from(u16::from_be_bytes(bytes[i..i + 2].try_into().unwrap()));
    i += 2;
    if i + ext != bytes.len() {
        return Err(TemplateError::InvalidTemplate(
            "invalid ISO 2005 extension length",
        ));
    }
    let mut singular_points = Vec::new();
    if ext > 0 {
        if ext < 6 || bytes[i..i + 2] != [0, 2] {
            return Err(TemplateError::InvalidTemplate(
                "unsupported ISO 2005 extension",
            ));
        }
        let declared = usize::from(u16::from_be_bytes(bytes[i + 2..i + 4].try_into().unwrap()));
        if declared + 4 != ext && declared != ext {
            return Err(TemplateError::InvalidTemplate(
                "invalid ISO 2005 core/delta extension",
            ));
        }
        let mut j = i + 4;
        let nc = usize::from(bytes[j]);
        j += 1;
        for _ in 0..nc {
            singular_points.push(SingularPoint {
                kind: SingularPointKind::Core,
                x: u16::from_be_bytes(bytes[j..j + 2].try_into().unwrap()) & 0x3fff,
                y: u16::from_be_bytes(bytes[j + 2..j + 4].try_into().unwrap()) & 0x3fff,
                confidence: 100,
            });
            j += 4;
        }
        let nd = usize::from(bytes[j]);
        j += 1;
        for _ in 0..nd {
            singular_points.push(SingularPoint {
                kind: SingularPointKind::Delta,
                x: u16::from_be_bytes(bytes[j..j + 2].try_into().unwrap()) & 0x3fff,
                y: u16::from_be_bytes(bytes[j + 2..j + 4].try_into().unwrap()) & 0x3fff,
                confidence: 100,
            });
            j += 4;
        }
        if j != bytes.len() {
            return Err(TemplateError::InvalidTemplate(
                "trailing ISO 2005 extension data",
            ));
        }
    }
    Ok(IsoTemplate {
        bytes: bytes.to_vec(),
        mean_quality: mean_quality(&minutiae),
        minutiae,
        singular_points,
        finger_position: pos,
        format: IsoFormat::Iso2005,
    })
}

fn finger_position(value: u8) -> Result<FingerPosition, TemplateError> {
    match value {
        0 => Ok(FingerPosition::Unknown),
        1 => Ok(FingerPosition::RightThumb),
        2 => Ok(FingerPosition::RightIndex),
        3 => Ok(FingerPosition::RightMiddle),
        4 => Ok(FingerPosition::RightRing),
        5 => Ok(FingerPosition::RightLittle),
        6 => Ok(FingerPosition::LeftThumb),
        7 => Ok(FingerPosition::LeftIndex),
        8 => Ok(FingerPosition::LeftMiddle),
        9 => Ok(FingerPosition::LeftRing),
        10 => Ok(FingerPosition::LeftLittle),
        _ => Err(TemplateError::InvalidTemplate(
            "unsupported finger position",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iso_round_trip() {
        let ms = vec![Minutia {
            x: 10,
            y: 20,
            kind: MinutiaKind::RidgeEnding,
            angle: 64,
            quality: 80,
        }];
        let ps = vec![SingularPoint {
            kind: SingularPointKind::Core,
            x: 100,
            y: 120,
            confidence: 90,
        }];
        let bytes = serialize_2011(&ms, &ps, 80, FingerPosition::RightIndex);
        let parsed = IsoTemplate::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.minutiae(), ms);
        assert_eq!(parsed.singular_points()[0].kind, SingularPointKind::Core);
        assert_eq!(&bytes[..8], b"FMR\0\x30\x33\x30\0");
        assert_eq!(parsed.format(), IsoFormat::Iso2011);
    }

    #[test]
    fn iso_2005_round_trip() {
        let ms = vec![Minutia {
            x: 10,
            y: 20,
            kind: MinutiaKind::RidgeBifurcation,
            angle: 64,
            quality: 80,
        }];
        let bytes = serialize_2005(&ms, &[], 80, FingerPosition::LeftIndex);
        let parsed = IsoTemplate::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.minutiae(), ms);
        assert_eq!(parsed.format(), IsoFormat::Iso2005);
        assert_eq!(&bytes[..8], b"FMR\0 20\0");
    }
}
