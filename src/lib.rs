//! Native userspace USB capture for Futronic FS80H and FS81H scanners.
//!
//! The crate returns an uncompressed 8-bit grayscale [`RawFrame`], with
//! built-in support for FBI-compliant WSQ compression and decompression.
//!
//! ```no_run
//! use futronic_fs80h_rs::FutronicDevice;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut scanner = FutronicDevice::open_first()?;
//! let frame = scanner.capture_frame()?;
//! println!("{}x{} at {} DPI", frame.width(), frame.height(), frame.dpi());
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "biometric-core")]
pub mod biometric_core;
mod device;
pub mod discovery;
pub mod error;
pub mod frame;
#[cfg(feature = "native")]
mod ffi;
#[cfg(feature = "lfd-research")]
pub mod lfd;
#[cfg(feature = "matcher")]
pub mod matcher;
#[cfg(feature = "nfiq2")]
pub mod nfiq2;
pub mod presence;
mod protocol;
#[cfg(feature = "templates")]
pub mod template;
#[cfg(feature = "wsq")]
pub mod wsq;

#[cfg(feature = "mcc")]
pub use biometric_core::mcc;
#[cfg(feature = "biometric-core")]
pub use biometric_core::{
    extract_minutiae, render_minutiae_overlay, render_minutiae_overlay_inverted, BiometricError,
    BiometricMinutia, MinutiaType,
};
pub use device::{AutoCaptureOptions, CaptureFeedback, DeviceInfo, FutronicDevice};
pub use discovery::{
    list_devices, open_device_by_id, open_single_device, DeviceId, DiscoveredDevice,
    ParseDeviceIdError,
};
#[cfg(feature = "wsq")]
pub use error::WsqError;
pub use error::{ScannerError, UsbOperation};
pub use frame::{resample_bilinear, RawFrame};
#[cfg(feature = "lfd-research")]
pub use lfd::{
    analyze_lfd_auxiliary_frames, analyze_lfd_auxiliary_frames_with_threshold,
    assess_lfd_auxiliary_triplet, cmos_register_command, LfdAuxiliaryAnalysisError,
    LfdAuxiliaryFrameMetrics, LfdAuxiliaryTriplet, LfdColoredCmosDryRunPlan,
    LfdColoredCmosDryRunStep, LfdFirmwareColorConfig, LfdResearchAcquisitionPlan,
    LfdResearchAssessment, LfdResearchCaptureStep, LfdResearchCaptureTiming, LfdResearchMetrics,
    LfdResearchOptions, LfdResearchOutcome, LfdResearchSample, LfdResearchSequence,
    BL2_CMOS_RESTORE_COMMAND, DEFAULT_AUXILIARY_SIGNAL_THRESHOLD, FIRMWARE_COLOR_CONFIG_QUERY,
    PREVIEW_LFD_DOSE,
};
#[cfg(feature = "lfd-experimental-decision")]
pub use lfd::{
    decide_lfd_experimental, LfdExperimentalDecision, LfdExperimentalDecisionOptions,
    LfdExperimentalEvaluation, LfdExperimentalPolicy, LfdExperimentalPolicyError,
    DEFAULT_EXPERIMENTAL_MIN_CORRELATION,
};
#[cfg(feature = "matcher")]
pub use matcher::{
    CandidateMatch, FingerprintTemplate, MatchResult, Matcher, MatcherError, MatcherOptions,
};
#[cfg(feature = "nfiq2")]
pub use nfiq2::{ActionableQualityFeedback, Nfiq2, Nfiq2Assessment, Nfiq2Error, Nfiq2Options};
pub use presence::{
    evaluate_frame_presence, FingerPresenceMetrics, DEFAULT_MIN_CONTACT_RATIO,
    MIN_BLOCK_DYNAMIC_RANGE, MIN_BLOCK_PEAK_INTENSITY, PRESENCE_BLOCK_SIZE, TOTAL_SENSOR_BLOCKS,
};
pub use protocol::{
    BULK_CHUNK_SIZE, DEFAULT_IMAGE_DOSE, RAW_FRAME_SIZE, RAW_SENSOR_HEIGHT, RAW_SENSOR_STREAM_SIZE,
    RAW_SENSOR_WIDTH, SENSOR_DPI, SENSOR_HEIGHT, SENSOR_WIDTH,
};
#[cfg(feature = "ansi-378")]
pub use template::ansi::{AnsiFormat, AnsiTemplate};
#[cfg(feature = "templates")]
pub use template::{
    FingerPosition, IsoFormat, IsoTemplate, Minutia, MinutiaKind, SingularPoint, SingularPointKind,
    TemplateError, TemplateOptions,
};
