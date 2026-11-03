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
}
