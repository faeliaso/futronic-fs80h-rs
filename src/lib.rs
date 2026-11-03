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

mod device;
pub mod error;
pub mod frame;
mod protocol;
#[cfg(feature = "wsq")]
pub mod wsq;

pub use device::{DeviceInfo, FutronicDevice};
#[cfg(feature = "wsq")]
pub use error::WsqError;
pub use error::{ScannerError, UsbOperation};
pub use frame::{resample_bilinear, RawFrame};
pub use protocol::{
    BULK_CHUNK_SIZE, DEFAULT_IMAGE_DOSE, RAW_FRAME_SIZE, RAW_SENSOR_HEIGHT, RAW_SENSOR_STREAM_SIZE,
    RAW_SENSOR_WIDTH, SENSOR_DPI, SENSOR_HEIGHT, SENSOR_WIDTH,
};
