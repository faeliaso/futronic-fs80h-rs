pub const FUTRONIC_VID: u16 = 0x1491;
pub const FS80_PID: u16 = 0x0020;
pub const FS81_PID: u16 = 0x0025;

/// Physical CMOS sensor line width transmitted over USB Bulk IN.
pub const RAW_SENSOR_WIDTH: usize = 384;
/// Physical CMOS sensor line count transmitted over USB Bulk IN.
pub const RAW_SENSOR_HEIGHT: usize = 800;
/// Total bytes in the complete raw CMOS sensor stream (384 x 800 = 307,200 bytes).
pub const RAW_SENSOR_STREAM_SIZE: usize = RAW_SENSOR_WIDTH * RAW_SENSOR_HEIGHT;

/// Standard nominal biometric frame width (pixels).
pub const SENSOR_WIDTH: usize = 320;
/// Standard nominal biometric frame height (pixels).
pub const SENSOR_HEIGHT: usize = 480;
/// Standard nominal biometric frame resolution (DPI).
pub const SENSOR_DPI: u16 = 500;
/// Total bytes in the standard nominal biometric frame (320 x 480 = 153,600 bytes).
pub const RAW_FRAME_SIZE: usize = SENSOR_WIDTH * SENSOR_HEIGHT;

pub const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2_500);
pub const FRAME_CHUNK_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1_500);
pub const EXPECTED_BULK_OUT_ENDPOINT: u8 = 0x01;
pub const EXPECTED_BULK_IN_ENDPOINT: u8 = 0x82;

/// Standard bulk transfer chunk size used by vendor SDK (4096 bytes / 0x1000).
pub const BULK_CHUNK_SIZE: usize = 4096;

/// FS80H image acquisition command recovered from the vendor SDK's Bulk path.
pub(crate) const GET_IMAGE_VARIABLE_DOSE: u8 = 0x6A;
/// FS80H status-diode command. Its two payload bytes are binary green/red
/// states as used by the vendor SDK.
pub(crate) const SET_DIODES: u8 = 0xE3;
/// Alternate status-diode opcode found in macOS vendor SDK (`cmdSetDiodes`).
#[allow(dead_code)]
pub(crate) const SET_DIODES_ALT: u8 = 0xDD;
/// Frame-mode gain register used by the Android SDK's `GetFrame` path.
pub(crate) const SET_FRAME_MODE_GAIN: u8 = 0x49;
/// Value used by the Android SDK to restore the frame-mode gain register.
pub(crate) const FRAME_MODE_GAIN_RESET: u8 = 0x04;
/// Read-only request for the seven FS80H control bytes used by the vendor SDK
/// during device initialization.
pub(crate) const GET_CONTROL_BYTES: u8 = 0x50;
/// Descriptor query command used during initialization.
#[allow(dead_code)]
pub(crate) const GET_DESCRIPTOR: u8 = 0xE0;
/// Preliminary sampling/preview dose used by vendor SDK for auto-exposure (45 / 0x2D).
#[allow(dead_code)]
pub(crate) const PREVIEW_MEASURE_DOSE: u8 = 0x2D;
/// Exposure measured by the vendor SDK on FS80H (166 / 0xA6).
///
/// This is the calibrated integration dose reported by FTRSDK_Coca (`Measure value: 166`).
pub const DEFAULT_IMAGE_DOSE: u8 = 166;

#[must_use]
pub(crate) const fn is_supported_product(product_id: u16) -> bool {
    product_id == FS80_PID || product_id == FS81_PID
}
