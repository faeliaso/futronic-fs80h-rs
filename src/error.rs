use thiserror::Error;

/// The USB operation which failed, without exposing an implementation-specific
/// USB error type in the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbOperation {
    Enumerate,
    ReadDescriptor,
    OpenDevice,
    ClaimInterface,
    ReleaseInterface,
    DetachKernelDriver,
    AttachKernelDriver,
    SetDiodes,
    ConfigureFrameMode,
    QueryControlBytes,
    ProbeLfdAuxiliaryFrame,
    TriggerFrame,
    ReadFrame,
}

impl core::fmt::Display for UsbOperation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            Self::Enumerate => "enumerate devices",
            Self::ReadDescriptor => "read USB descriptor",
            Self::OpenDevice => "open device",
            Self::ClaimInterface => "claim interface",
            Self::ReleaseInterface => "release interface",
            Self::DetachKernelDriver => "detach kernel driver",
            Self::AttachKernelDriver => "attach kernel driver",
            Self::SetDiodes => "set scanner diodes",
            Self::ConfigureFrameMode => "configure scanner frame mode",
            Self::QueryControlBytes => "query scanner control bytes",
            Self::ProbeLfdAuxiliaryFrame => "probe an experimental LFD auxiliary frame",
            Self::TriggerFrame => "trigger frame",
            Self::ReadFrame => "read frame",
        };
        formatter.write_str(name)
    }
}

/// Failures returned by the scanner driver.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ScannerError {
    #[error("no Futronic FS80H/FS81H scanner was found on the USB bus")]
    DeviceNotFound,

    #[error("USB operation '{operation}' failed: {message}")]
    Usb {
        operation: UsbOperation,
        message: String,
    },

    #[error("the scanner USB interface is busy")]
    InterfaceBusy,

    #[error("the scanner has no unambiguous bulk-IN interface for frame capture")]
    UnsupportedUsbLayout,

    #[error("timed out while waiting for the scanner")]
    Timeout,

    #[error("incomplete fingerprint frame: received {received} of {expected} bytes")]
    IncompleteFrame { received: usize, expected: usize },

    #[error("incomplete scanner response: received {received} of at least {expected} bytes")]
    IncompleteResponse { received: usize, expected: usize },

    #[error("incomplete USB command write: sent {written} of {expected} bytes")]
    IncompleteCommand { written: usize, expected: usize },

    #[cfg(feature = "lfd-research")]
    #[error("LFD research is unsupported for product ID {product_id:#06X}; only FS80H (0x0020) is allowed")]
    UnsupportedLfdResearch { product_id: u16 },

    #[cfg(feature = "lfd-research")]
    #[error("unvalidated LFD probe geometry {width}x{height} does not match the known {expected_bytes}-byte FS80H stream")]
    UnexpectedLfdProbeGeometry {
        width: u16,
        height: u16,
        expected_bytes: usize,
    },

    #[cfg(feature = "wsq")]
    #[error(transparent)]
    Wsq(#[from] WsqError),
}

/// Failures returned by the WSQ compression and decompression routines.
#[cfg(feature = "wsq")]
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WsqError {
    #[error("WSQ compression failed with code {0}")]
    EncodeFailed(i32),

    #[error("WSQ decompression failed with code {0}")]
    DecodeFailed(i32),

    #[error("invalid bitrate {0}: must be positive and between 0.1 and 2.5")]
    InvalidBitrate(String),

    #[error("unsupported pixel depth {0}: expected 8-bit grayscale")]
    UnsupportedDepth(i32),

    #[error("decompressed frame size mismatch: got {received} bytes, expected {expected}")]
    SizeMismatch { received: usize, expected: usize },

    #[error("input WSQ buffer is empty or corrupted")]
    EmptyBuffer,
}
