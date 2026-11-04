//! ANSI INCITS 378 normal fingerprint minutiae templates.
//!
//! This module implements the interoperable, one-fingerprint profile used by this crate. ANSI
//! 378 is deliberately kept separate from ISO/IEC 19794-2: some revisions share their `FMR`
//! signature and version field, but their binary layouts are not interchangeable.

use super::{
    FingerPosition, IsoFormat, Minutia, MinutiaKind, SingularPoint, SingularPointKind,
    TemplateError, TemplateOptions,
};
use crate::RawFrame;

const WIDTH: u16 = 320;
const HEIGHT: u16 = 480;
const RESOLUTION_PPCM: u16 = 197;
const VENDOR_UNKNOWN: u16 = 0x0103;

/// ANSI INCITS 378 normal/off-card revisions supported by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiFormat {
    /// ANSI INCITS 378-2004.
    Ansi3782004,
    /// ANSI INCITS 378-2009.
    Ansi3782009,
    /// ANSI INCITS 378:2009/AM 1:2010.
    Ansi3782009Am1,
}

/// Parsed or extracted ANSI INCITS 378 template in the supported one-fingerprint profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiTemplate {
    bytes: Vec<u8>,
    minutiae: Vec<Minutia>,
    singular_points: Vec<SingularPoint>,
    mean_quality: u8,
    finger_position: FingerPosition,
    format: AnsiFormat,
}

impl AnsiTemplate {
    /// Returns the complete ANSI INCITS 378 record.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consumes the template and returns its binary record.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Returns the extracted minutiae.
    #[must_use]
    pub fn minutiae(&self) -> &[Minutia] {
        &self.minutiae
    }
    /// Returns included core/delta estimates.
    #[must_use]
    pub fn singular_points(&self) -> &[SingularPoint] {
        &self.singular_points
    }
    /// Returns mean retained minutia quality.
    #[must_use]
    pub const fn mean_quality(&self) -> u8 {
        self.mean_quality
    }
    /// Returns the ANSI revision used by this record.
    #[must_use]
    pub const fn format(&self) -> AnsiFormat {
        self.format
    }
    /// Returns the encoded finger position.
    #[must_use]
    pub const fn finger_position(&self) -> FingerPosition {
        self.finger_position
    }
    /// Parses an ANSI record in this crate's strict single-fingerprint profile.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TemplateError> {
        parse(bytes)
    }
}

pub(crate) fn extract(
    frame: &RawFrame,
    format: AnsiFormat,
    options: &TemplateOptions,
) -> Result<AnsiTemplate, TemplateError> {
    // ISO extraction is used only as the shared MINDTCT feature-extraction pipeline. ANSI bytes
    // are serialized independently below.
    let extracted = super::extract_as(frame, IsoFormat::Iso2011, options)?;
    let minutiae = extracted.minutiae().to_vec();
    let points = extracted.singular_points().to_vec();
    let bytes = serialize(
        format,
        &minutiae,
        &points,
        extracted.mean_quality(),
        options.finger_position,
    );
    Ok(AnsiTemplate {
        bytes,
        minutiae,
        singular_points: points,
        mean_quality: extracted.mean_quality(),
        finger_position: options.finger_position,
        format,
    })
}

fn serialize(
    format: AnsiFormat,
    minutiae: &[Minutia],
    points: &[SingularPoint],
    mean: u8,
    position: FingerPosition,
) -> Vec<u8> {
    match format {
        AnsiFormat::Ansi3782004 => serialize_2004(minutiae, points, mean, position),
        AnsiFormat::Ansi3782009 => serialize_2009(b"030\0", minutiae, points, mean, position),
        AnsiFormat::Ansi3782009Am1 => serialize_2009(b"035\0", minutiae, points, mean, position),
    }
}

fn extensions(points: &[SingularPoint]) -> Vec<u8> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut data = Vec::new();
    let cores: Vec<_> = points
        .iter()
        .filter(|p| p.kind == SingularPointKind::Core)
        .collect();
    data.push(cores.len() as u8); // no optional core angles
    for p in cores {
        put_point(&mut data, p);
    }
    let deltas: Vec<_> = points
        .iter()
        .filter(|p| p.kind == SingularPointKind::Delta)
        .collect();
    data.push(deltas.len() as u8); // no optional delta angles
    for p in deltas {
        put_point(&mut data, p);
    }
    let mut out = Vec::with_capacity(data.len() + 4);
    put_u16(&mut out, 2);
    put_u16(&mut out, (data.len() + 4) as u16);
    out.extend(data);
    out
}

fn serialize_2004(
    minutiae: &[Minutia],
    points: &[SingularPoint],
    mean: u8,
    position: FingerPosition,
) -> Vec<u8> {
    let ext = extensions(points);
    let total = 26 + 4 + minutiae.len() * 6 + 2 + ext.len();
    // The supported single-finger profile always fits ANSI 378-2004's compact length field.
    debug_assert!(total <= u16::MAX as usize);
    let mut out = Vec::with_capacity(total);
    out.extend(*b"FMR\0");
    out.extend(*b" 20\0");
    put_u16(&mut out, total as u16);
    put_u16(&mut out, VENDOR_UNKNOWN);
    put_u16(&mut out, 0);
    put_u16(&mut out, 0);
    put_u16(&mut out, WIDTH);
    put_u16(&mut out, HEIGHT);
    put_u16(&mut out, RESOLUTION_PPCM);
    put_u16(&mut out, RESOLUTION_PPCM);
    out.extend([1, 0]);
    out.push(position as u8);
    out.push(0);
    out.push(mean);
    out.push(minutiae.len() as u8);
    put_minutiae(&mut out, minutiae);
    put_u16(&mut out, ext.len() as u16);
    out.extend(ext);
    debug_assert_eq!(out.len(), total);
    out
}

fn serialize_2009(
    version: &[u8; 4],
    minutiae: &[Minutia],
    points: &[SingularPoint],
    mean: u8,
    position: FingerPosition,
) -> Vec<u8> {
    let ext = extensions(points);
    let total = 21 + 17 + minutiae.len() * 6 + 2 + ext.len();
    let mut out = Vec::with_capacity(total);
    out.extend(*b"FMR\0");
    out.extend(*version);
    put_u32(&mut out, total as u32);
    put_u16(&mut out, VENDOR_UNKNOWN);
    put_u16(&mut out, 0);
    out.push(0);
    put_u16(&mut out, 0);
    out.extend([1, 0]);
    out.push(position as u8);
    out.push(0);
    out.push(0);
    out.push(mean);
    put_u16(&mut out, 0);
    put_u16(&mut out, 0);
    put_u16(&mut out, WIDTH);
    put_u16(&mut out, HEIGHT);
    put_u16(&mut out, RESOLUTION_PPCM);
    put_u16(&mut out, RESOLUTION_PPCM);
    out.push(minutiae.len() as u8);
    put_minutiae(&mut out, minutiae);
    put_u16(&mut out, ext.len() as u16);
    out.extend(ext);
    debug_assert_eq!(out.len(), total);
    out
}

fn put_minutiae(out: &mut Vec<u8>, minutiae: &[Minutia]) {
    for m in minutiae {
        let kind = match m.kind {
            MinutiaKind::RidgeEnding => 1,
            MinutiaKind::RidgeBifurcation => 2,
            MinutiaKind::Other => 0,
        };
        put_u16(out, (kind << 14) | (m.x & 0x3fff));
        put_u16(out, m.y & 0x3fff);
        out.push(to_ansi_angle(m.angle));
        out.push(m.quality);
    }
}

fn put_point(out: &mut Vec<u8>, p: &SingularPoint) {
    put_u16(out, p.x & 0x3fff);
    put_u16(out, p.y & 0x3fff);
}
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_be_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_be_bytes());
}
fn to_ansi_angle(angle: u8) -> u8 {
    ((u16::from(angle) * 180).div_ceil(256) % 180) as u8
}
fn from_ansi_angle(angle: u8) -> u8 {
    (((u16::from(angle) * 256 + 90) / 180) % 256) as u8
}

fn parse(bytes: &[u8]) -> Result<AnsiTemplate, TemplateError> {
    if bytes.len() < 12 || &bytes[..4] != b"FMR\0" {
        return Err(invalid("unsupported header"));
    }
    match &bytes[4..8] {
        b" 20\0" => parse_2004(bytes),
        b"030\0" => parse_2009(bytes, AnsiFormat::Ansi3782009),
        b"035\0" => parse_2009(bytes, AnsiFormat::Ansi3782009Am1),
        _ => Err(invalid("unsupported version")),
    }
}

fn parse_2004(bytes: &[u8]) -> Result<AnsiTemplate, TemplateError> {
    if bytes.len() < 26 {
        return Err(invalid("truncated 2004 header"));
    }
    let short = be16(bytes, 8)? as usize;
    // A short ANSI length identifies the format. A leading zero followed by a plausible ISO
    // total length is ISO 19794-2:2005, which intentionally shares this header.
    if short == 0 {
        return Err(invalid(
            "record is ISO 19794-2:2005 or unsupported extended ANSI length",
        ));
    }
    if short != bytes.len() || short < 26 {
        return Err(invalid("invalid 2004 total length"));
    }
    if be16(bytes, 10)? == 0 {
        return Err(invalid("zero vendor id"));
    }
    if bytes[24] != 1 || bytes[25] != 0 {
        return Err(invalid("only one fingerprint is supported"));
    }
    let (position, minutiae, points, mean) = parse_fingerprint(bytes, 26, false)?;
    finish(
        bytes,
        AnsiFormat::Ansi3782004,
        position,
        minutiae,
        points,
        mean,
    )
}

fn parse_2009(bytes: &[u8], format: AnsiFormat) -> Result<AnsiTemplate, TemplateError> {
    if bytes.len() < 21 {
        return Err(invalid("truncated 2009 header"));
    }
    let total = be32(bytes, 8)? as usize;
    if total != bytes.len() {
        return Err(invalid("invalid 2009 total length"));
    }
    // ISO 19794-2:2011 shares FMR/030. Its record count/certification and per-finger byte length
    // make it detectable before ANSI fields are interpreted.
    if format == AnsiFormat::Ansi3782009
        && bytes.len() >= 19
        && be16(bytes, 12)? == 1
        && bytes[14] == 0
        && be32(bytes, 15)? as usize + 15 == total
    {
        return Err(invalid("record is ISO/IEC 19794-2:2011"));
    }
    if be16(bytes, 12)? == 0 || bytes[20] != 0 {
        return Err(invalid("invalid 2009 header"));
    }
    if bytes[19] != 1 {
        return Err(invalid("only one fingerprint is supported"));
    }
    if format == AnsiFormat::Ansi3782009Am1 && total < 40 {
        return Err(invalid("AM1 record is too short"));
    }
    let (position, minutiae, points, mean) = parse_fingerprint(bytes, 21, true)?;
    finish(bytes, format, position, minutiae, points, mean)
}

fn parse_fingerprint(
    bytes: &[u8],
    start: usize,
    modern: bool,
) -> Result<(FingerPosition, Vec<Minutia>, Vec<SingularPoint>, u8), TemplateError> {
    let fixed = if modern { 17 } else { 4 };
    if start + fixed + 2 > bytes.len() {
        return Err(invalid("truncated fingerprint record"));
    }
    let position = finger_position(bytes[start])?;
    let (quality_at, count_at, minutiae_at) = if modern {
        (start + 3, start + 16, start + 17)
    } else {
        (start + 2, start + 3, start + 4)
    };
    let count = bytes[count_at] as usize;
    let ext_at = minutiae_at
        .checked_add(count * 6)
        .ok_or_else(|| invalid("minutia count overflow"))?;
    if ext_at + 2 > bytes.len() {
        return Err(invalid("truncated minutiae"));
    }
    let mut minutiae = Vec::with_capacity(count);
    for i in 0..count {
        let at = minutiae_at + i * 6;
        let packed = be16(bytes, at)?;
        let y = be16(bytes, at + 2)?;
        if y & 0xc000 != 0 || packed & 0x3fff >= WIDTH || y & 0x3fff >= HEIGHT {
            return Err(invalid("minutia coordinate outside image"));
        }
        let kind = match packed >> 14 {
            0 => MinutiaKind::Other,
            1 => MinutiaKind::RidgeEnding,
            2 => MinutiaKind::RidgeBifurcation,
            _ => return Err(invalid("reserved minutia type")),
        };
        if bytes[at + 4] >= 180 {
            return Err(invalid("invalid minutia angle"));
        }
        minutiae.push(Minutia {
            x: packed & 0x3fff,
            y: y & 0x3fff,
            kind,
            angle: from_ansi_angle(bytes[at + 4]),
            quality: bytes[at + 5],
        });
    }
    let ext_len = be16(bytes, ext_at)? as usize;
    let end = ext_at + 2 + ext_len;
    if end != bytes.len() {
        return Err(invalid("invalid extension length"));
    }
    let points = parse_extensions(&bytes[ext_at + 2..end])?;
    Ok((position, minutiae, points, bytes[quality_at]))
}

fn parse_extensions(bytes: &[u8]) -> Result<Vec<SingularPoint>, TemplateError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.len() < 6 || be16(bytes, 0)? != 2 || be16(bytes, 2)? as usize != bytes.len() {
        return Err(invalid("unsupported extension"));
    }
    let mut at = 4;
    let core_header = *bytes
        .get(at)
        .ok_or_else(|| invalid("truncated core data"))?;
    at += 1;
    if core_header & 0xb0 != 0 {
        return Err(invalid("invalid core data"));
    }
    let cores = (core_header & 0x0f) as usize;
    let core_has_angle = core_header & 0x40 != 0;
    let mut out = Vec::with_capacity(cores + 2);
    for _ in 0..cores {
        out.push(read_point(bytes, &mut at, SingularPointKind::Core)?);
        if core_has_angle {
            skip(bytes, &mut at, 1, "truncated core angle")?;
        }
    }
    let delta_header = *bytes
        .get(at)
        .ok_or_else(|| invalid("truncated delta data"))?;
    at += 1;
    if delta_header & 0xb0 != 0 {
        return Err(invalid("invalid delta data"));
    }
    let deltas = (delta_header & 0x0f) as usize;
    let delta_has_angles = delta_header & 0x40 != 0;
    for _ in 0..deltas {
        out.push(read_point(bytes, &mut at, SingularPointKind::Delta)?);
        if delta_has_angles {
            skip(bytes, &mut at, 3, "truncated delta angles")?;
        }
    }
    if at != bytes.len() {
        return Err(invalid("trailing extension data"));
    }
    Ok(out)
}

fn read_point(
    bytes: &[u8],
    at: &mut usize,
    kind: SingularPointKind,
) -> Result<SingularPoint, TemplateError> {
    if *at + 4 > bytes.len() {
        return Err(invalid("truncated singular point"));
    }
    let x = be16(bytes, *at)?;
    let y = be16(bytes, *at + 2)?;
    *at += 4;
    if x & 0xc000 != 0 || y & 0xc000 != 0 || x >= WIDTH || y >= HEIGHT {
        return Err(invalid("singular point outside image"));
    }
    Ok(SingularPoint {
        kind,
        x,
        y,
        confidence: 100,
    })
}

fn skip(
    bytes: &[u8],
    at: &mut usize,
    count: usize,
    message: &'static str,
) -> Result<(), TemplateError> {
    let end = at.checked_add(count).ok_or_else(|| invalid(message))?;
    if end > bytes.len() {
        return Err(invalid(message));
    }
    *at = end;
    Ok(())
}

fn finish(
    bytes: &[u8],
    format: AnsiFormat,
    finger_position: FingerPosition,
    minutiae: Vec<Minutia>,
    singular_points: Vec<SingularPoint>,
    mean_quality: u8,
) -> Result<AnsiTemplate, TemplateError> {
    Ok(AnsiTemplate {
        bytes: bytes.to_vec(),
        minutiae,
        singular_points,
        mean_quality,
        finger_position,
        format,
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
        _ => Err(invalid("unsupported finger position")),
    }
}
fn be16(bytes: &[u8], at: usize) -> Result<u16, TemplateError> {
    bytes
        .get(at..at + 2)
        .ok_or_else(|| invalid("truncated field"))
        .map(|v| u16::from_be_bytes(v.try_into().unwrap()))
}
fn be32(bytes: &[u8], at: usize) -> Result<u32, TemplateError> {
    bytes
        .get(at..at + 4)
        .ok_or_else(|| invalid("truncated field"))
        .map(|v| u32::from_be_bytes(v.try_into().unwrap()))
}
fn invalid(message: &'static str) -> TemplateError {
    TemplateError::InvalidAnsiTemplate(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn minutiae() -> Vec<Minutia> {
        vec![Minutia {
            x: 10,
            y: 20,
            kind: MinutiaKind::RidgeEnding,
            angle: 64,
            quality: 80,
        }]
    }
    fn points() -> Vec<SingularPoint> {
        vec![SingularPoint {
            kind: SingularPointKind::Core,
            x: 100,
            y: 120,
            confidence: 90,
        }]
    }
    #[test]
    fn round_trips_all_ansi_revisions() {
        for format in [
            AnsiFormat::Ansi3782004,
            AnsiFormat::Ansi3782009,
            AnsiFormat::Ansi3782009Am1,
        ] {
            let bytes = serialize(
                format,
                &minutiae(),
                &points(),
                80,
                FingerPosition::RightIndex,
            );
            let parsed = AnsiTemplate::from_bytes(&bytes).unwrap();
            assert_eq!(parsed.format(), format);
            assert_eq!(parsed.minutiae(), minutiae());
            assert_eq!(parsed.singular_points().len(), 1);
        }
    }
    #[test]
    fn writes_expected_revision_markers() {
        assert_eq!(
            &serialize(
                AnsiFormat::Ansi3782004,
                &minutiae(),
                &[],
                80,
                FingerPosition::Unknown
            )[4..8],
            b" 20\0"
        );
        assert_eq!(
            &serialize(
                AnsiFormat::Ansi3782009,
                &minutiae(),
                &[],
                80,
                FingerPosition::Unknown
            )[4..8],
            b"030\0"
        );
        assert_eq!(
            &serialize(
                AnsiFormat::Ansi3782009Am1,
                &minutiae(),
                &[],
                80,
                FingerPosition::Unknown
            )[4..8],
            b"035\0"
        );
    }
    #[test]
    fn rejects_multifinger_and_unknown_extensions() {
        let mut bytes = serialize(
            AnsiFormat::Ansi3782009Am1,
            &minutiae(),
            &[],
            80,
            FingerPosition::Unknown,
        );
        bytes[19] = 2;
        assert!(AnsiTemplate::from_bytes(&bytes).is_err());
        let mut bytes = serialize(
            AnsiFormat::Ansi3782009Am1,
            &minutiae(),
            &[],
            80,
            FingerPosition::Unknown,
        );
        let end = bytes.len();
        bytes[end - 2..].copy_from_slice(&4u16.to_be_bytes());
        bytes.extend([0, 1, 0, 4]);
        let total = bytes.len() as u32;
        bytes[8..12].copy_from_slice(&total.to_be_bytes());
        assert!(AnsiTemplate::from_bytes(&bytes).is_err());
    }

    #[test]
    fn rejects_iso_records_with_shared_headers() {
        let iso_2005 = super::super::serialize_2005(&minutiae(), &[], 80, FingerPosition::Unknown);
        let iso_2011 = super::super::serialize_2011(&minutiae(), &[], 80, FingerPosition::Unknown);
        assert!(AnsiTemplate::from_bytes(&iso_2005).is_err());
        assert!(AnsiTemplate::from_bytes(&iso_2011).is_err());
    }
}
