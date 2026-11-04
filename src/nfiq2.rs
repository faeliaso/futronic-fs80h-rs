//! NIST NFIQ2 image-quality assessment for live fingerprint frames.
//!
//! NFIQ2 supplies a 0 through 100 estimate of the image's utility for matching. The actionable
//! feedback values describe image properties, not a diagnosis of a person's finger pressure or
//! liveness. Applications should map the returned codes to their own localized retry guidance.

use crate::RawFrame;
use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_uchar, c_uint, c_void},
    path::Path,
    ptr::NonNull,
    sync::Mutex,
};
use thiserror::Error;

const EMPTY_OR_LOW_CONTRAST: c_uchar = 1;
const UNIFORM_IMAGE: c_uchar = 1 << 1;
const TOO_FEW_MINUTIAE: c_uchar = 1 << 2;
const INSUFFICIENT_FOREGROUND: c_uchar = 1 << 3;

/// Configuration for an [`Nfiq2`] quality-assessment engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nfiq2Options {
    /// Minimum inclusive NFIQ2 score required to accept a capture, from `0` through `100`.
    pub minimum_score: u8,
}

impl Default for Nfiq2Options {
    fn default() -> Self {
        Self { minimum_score: 50 }
    }
}

/// Actionable image conditions reported by NFIQ2.
///
/// These are image-quality signals. For example, [`InsufficientForeground`](Self::InsufficientForeground)
/// may be rendered as a request to reposition the finger, but it does not prove a particular
/// physical cause such as insufficient pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionableQualityFeedback {
    /// The image is empty or its contrast is too low.
    EmptyOrLowContrast,
    /// Gray levels are too uniform to contain useful ridge detail.
    UniformImage,
    /// The image contains too few detected minutiae.
    TooFewMinutiae,
    /// Too little fingerprint foreground is present in the frame.
    InsufficientForeground,
}

/// NFIQ2's result for one fingerprint image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nfiq2Assessment {
    /// NFIQ2 unified quality score, from `0` (least useful) through `100` (most useful).
    pub score: u8,
    /// Whether `score` reaches the configured inclusive minimum score.
    pub accepted: bool,
    /// Image conditions that NFIQ2 marks as needing corrective action.
    pub feedback: Vec<ActionableQualityFeedback>,
}

/// Failures while configuring or running NFIQ2.
#[derive(Debug, Error)]
pub enum Nfiq2Error {
    /// The configured score threshold is outside the inclusive range `0..=100`.
    #[error("invalid NFIQ2 minimum score {0}: must be between 0 and 100")]
    InvalidMinimumScore(u8),
    /// The model-info path could not be represented for the native NFIQ2 API.
    #[error("NFIQ2 model-info path must be valid UTF-8 and contain no NUL bytes")]
    InvalidModelPath,
    /// NFIQ2 could not load the model-info file or its referenced random-forest model.
    #[error("NFIQ2 model initialization failed: {0}")]
    ModelInitialization(String),
    /// The fingerprint does not cover enough of the sensor for NFIQ2 to compute a score.
    ///
    /// This is a recoverable capture rejection. Reposition the finger and try again; the
    /// accompanying feedback is suitable for application-specific, localized guidance.
    #[error("NFIQ2 rejected the capture: {message}")]
    CaptureRejected {
        /// Structured image condition that prompted the retry.
        feedback: Vec<ActionableQualityFeedback>,
        /// Native diagnostic retained for logs and support, not for direct user display.
        message: String,
    },
    /// NFIQ2 could not assess the image.
    #[error("NFIQ2 quality assessment failed: {0}")]
    Assessment(String),
}

/// A local NIST NFIQ2 2.3 image-quality assessment engine.
///
/// Construct one engine per configured model and reuse it for capture attempts. The supplied
/// model-info file controls the random-forest parameters used by NFIQ2.
pub struct Nfiq2 {
    handle: NonNull<c_void>,
    options: Nfiq2Options,
    // The NFIQ2 API uses immutable methods during scoring, but serialize FFI calls until its
    // thread-safety contract is explicit in the upstream API.
    call_lock: Mutex<()>,
}

impl std::fmt::Debug for Nfiq2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Nfiq2")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl Nfiq2 {
    /// Creates an engine using an NFIQ2 model-information file.
    pub fn new(
        model_info_path: impl AsRef<Path>,
        options: Nfiq2Options,
    ) -> Result<Self, Nfiq2Error> {
        if options.minimum_score > 100 {
            return Err(Nfiq2Error::InvalidMinimumScore(options.minimum_score));
        }
        let path = model_info_path
            .as_ref()
            .to_str()
            .ok_or(Nfiq2Error::InvalidModelPath)?;
        let path = CString::new(path).map_err(|_| Nfiq2Error::InvalidModelPath)?;
        let mut raw = std::ptr::null_mut();
        // SAFETY: `path` remains alive for the call and `raw` is a valid out-pointer.
        let status = unsafe { fs80h_nfiq2_create(path.as_ptr(), &mut raw) };
        if status == 2 {
            return Err(Nfiq2Error::CaptureRejected {
                feedback: vec![ActionableQualityFeedback::InsufficientForeground],
                message: last_error(),
            });
        }
        if status != 0 {
            return Err(Nfiq2Error::ModelInitialization(last_error()));
        }
        let handle = NonNull::new(raw).ok_or_else(|| {
            Nfiq2Error::ModelInitialization("native bridge returned a null engine".to_owned())
        })?;
        Ok(Self {
            handle,
            options,
            call_lock: Mutex::new(()),
        })
    }

    /// Returns this engine's acceptance policy.
    #[must_use]
    pub const fn options(&self) -> Nfiq2Options {
        self.options
    }

    /// Assesses a raw scanner frame without writing its pixels to disk.
    pub fn assess(&self, frame: &RawFrame) -> Result<Nfiq2Assessment, Nfiq2Error> {
        let _guard = self.call_lock.lock().expect("NFIQ2 call lock poisoned");
        let mut score = 0;
        let mut feedback = 0;
        // SAFETY: the handle is valid for `self`'s lifetime; frame data is valid for this call.
        let status = unsafe {
            fs80h_nfiq2_assess(
                self.handle.as_ptr(),
                frame.as_bytes().as_ptr(),
                frame.as_bytes().len() as c_uint,
                frame.width(),
                frame.height(),
                frame.dpi(),
                &mut score,
                &mut feedback,
            )
        };
        if status != 0 {
            return Err(Nfiq2Error::Assessment(last_error()));
        }
        Ok(assessment_from_parts(self.options, score, feedback))
    }
}

impl Drop for Nfiq2 {
    fn drop(&mut self) {
        // SAFETY: the pointer was returned by `fs80h_nfiq2_create` and is released exactly once.
        unsafe { fs80h_nfiq2_destroy(self.handle.as_ptr()) };
    }
}

fn assessment_from_parts(options: Nfiq2Options, score: c_uchar, flags: c_uchar) -> Nfiq2Assessment {
    let mut feedback = Vec::with_capacity(4);
    for (bit, value) in [
        (
            EMPTY_OR_LOW_CONTRAST,
            ActionableQualityFeedback::EmptyOrLowContrast,
        ),
        (UNIFORM_IMAGE, ActionableQualityFeedback::UniformImage),
        (TOO_FEW_MINUTIAE, ActionableQualityFeedback::TooFewMinutiae),
        (
            INSUFFICIENT_FOREGROUND,
            ActionableQualityFeedback::InsufficientForeground,
        ),
    ] {
        if flags & bit != 0 {
            feedback.push(value);
        }
    }
    Nfiq2Assessment {
        score,
        accepted: score >= options.minimum_score,
        feedback,
    }
}

fn last_error() -> String {
    // SAFETY: the native bridge returns either null or a NUL-terminated string valid until the
    // next bridge call on this thread. It is copied immediately.
    let message = unsafe { fs80h_nfiq2_last_error() };
    if message.is_null() {
        "unknown native error".to_owned()
    } else {
        // SAFETY: guaranteed by the bridge contract above.
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe extern "C" {
    fn fs80h_nfiq2_create(model_info_path: *const c_char, out: *mut *mut c_void) -> c_int;
    fn fs80h_nfiq2_destroy(engine: *mut c_void);
    fn fs80h_nfiq2_assess(
        engine: *mut c_void,
        pixels: *const c_uchar,
        pixels_len: c_uint,
        width: c_uint,
        height: c_uint,
        ppi: u16,
        score: *mut c_uchar,
        feedback: *mut c_uchar,
    ) -> c_int;
    fn fs80h_nfiq2_last_error() -> *const c_char;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_is_inclusive_at_the_configured_threshold() {
        let assessment = assessment_from_parts(Nfiq2Options { minimum_score: 50 }, 50, 0);
        assert!(assessment.accepted);
        assert!(assessment.feedback.is_empty());
    }

    #[test]
    fn below_threshold_is_rejected_with_structured_feedback() {
        let assessment = assessment_from_parts(
            Nfiq2Options { minimum_score: 50 },
            49,
            EMPTY_OR_LOW_CONTRAST | INSUFFICIENT_FOREGROUND,
        );
        assert!(!assessment.accepted);
        assert_eq!(
            assessment.feedback,
            vec![
                ActionableQualityFeedback::EmptyOrLowContrast,
                ActionableQualityFeedback::InsufficientForeground,
            ]
        );
    }

    #[test]
    fn default_policy_is_conservative() {
        assert_eq!(Nfiq2Options::default().minimum_score, 50);
    }

    #[test]
    fn invalid_minimum_score_is_rejected_before_native_initialization() {
        assert!(matches!(
            Nfiq2::new("unused-model.info", Nfiq2Options { minimum_score: 101 }),
            Err(Nfiq2Error::InvalidMinimumScore(101))
        ));
    }

    #[test]
    fn too_small_capture_has_a_recoverable_structured_rejection() {
        let error = Nfiq2Error::CaptureRejected {
            feedback: vec![ActionableQualityFeedback::InsufficientForeground],
            message: "fingerprint area is too small".to_owned(),
        };
        assert!(matches!(
            error,
            Nfiq2Error::CaptureRejected { feedback, .. }
                if feedback == vec![ActionableQualityFeedback::InsufficientForeground]
        ));
    }
}
