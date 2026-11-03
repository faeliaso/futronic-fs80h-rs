use crate::error::{ScannerError, UsbOperation};
use crate::protocol::{
    is_supported_product, BULK_CHUNK_SIZE, DEFAULT_IMAGE_DOSE, DEFAULT_TIMEOUT,
    EXPECTED_BULK_IN_ENDPOINT, EXPECTED_BULK_OUT_ENDPOINT, FRAME_CHUNK_TIMEOUT,
    FRAME_MODE_GAIN_RESET, FUTRONIC_VID, GET_CONTROL_BYTES, GET_IMAGE_VARIABLE_DOSE,
    RAW_SENSOR_STREAM_SIZE, SET_DIODES, SET_FRAME_MODE_GAIN,
};
use crate::RawFrame;
use rusb::{Device, DeviceHandle, Direction, GlobalContext, TransferType};

/// Stable metadata identifying an opened Futronic scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus_number: u8,
    pub address: u8,
    pub product_name: Option<String>,
}

/// An exclusively claimed Futronic USB scanner.
pub struct FutronicDevice {
    handle: DeviceHandle<GlobalContext>,
    interface_number: u8,
    bulk_out_endpoint: u8,
    bulk_in_endpoint: u8,
    #[cfg(target_os = "linux")]
    detached_kernel_driver: bool,
    info: DeviceInfo,
}

impl FutronicDevice {
    /// Opens and initializes the first compatible scanner found on the USB bus.
    pub fn open_first() -> Result<Self, ScannerError> {
        let devices = rusb::devices().map_err(|error| usb_error(UsbOperation::Enumerate, error))?;

        for device in devices.iter() {
            let descriptor = device
                .device_descriptor()
                .map_err(|error| usb_error(UsbOperation::ReadDescriptor, error))?;
            if descriptor.vendor_id() != FUTRONIC_VID
                || !is_supported_product(descriptor.product_id())
            {
                continue;
            }

            let (interface_number, bulk_out_endpoint, bulk_in_endpoint) =
                find_capture_interface(&device)?;
            return Self::open_device(
                device,
                descriptor,
                interface_number,
                bulk_out_endpoint,
                bulk_in_endpoint,
            );
        }

        Err(ScannerError::DeviceNotFound)
    }

    fn open_device(
        device: Device<GlobalContext>,
        descriptor: rusb::DeviceDescriptor,
        interface_number: u8,
        bulk_out_endpoint: u8,
        bulk_in_endpoint: u8,
    ) -> Result<Self, ScannerError> {
        #[cfg(target_os = "linux")]
        let mut handle = device
            .open()
            .map_err(|error| usb_error(UsbOperation::OpenDevice, error))?;
        #[cfg(not(target_os = "linux"))]
        let handle = device
            .open()
            .map_err(|error| usb_error(UsbOperation::OpenDevice, error))?;

        #[cfg(target_os = "linux")]
        let detached_kernel_driver = detach_kernel_driver(&mut handle, interface_number)?;
        handle
            .claim_interface(interface_number)
            .map_err(|error| match error {
                rusb::Error::Busy => ScannerError::InterfaceBusy,
                other => usb_error(UsbOperation::ClaimInterface, other),
            })?;

        let product_name = handle.read_product_string_ascii(&descriptor).ok();
        let info = DeviceInfo {
            vendor_id: descriptor.vendor_id(),
            product_id: descriptor.product_id(),
            bus_number: device.bus_number(),
            address: device.address(),
            product_name,
        };
        Ok(Self {
            handle,
            interface_number,
            bulk_out_endpoint,
            bulk_in_endpoint,
            #[cfg(target_os = "linux")]
            detached_kernel_driver,
            info,
        })
    }

    /// Returns metadata for the claimed scanner.
    #[must_use]
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// Reads the seven control bytes queried by the vendor SDK at initialization.
    ///
    /// The operation is read-only: it sends command `0x50` and consumes its
    /// complete bulk packet. The returned bytes can identify scanner firmware
    /// behavior while investigating an open replacement for vendor calibration.
    pub fn read_control_bytes(&mut self) -> Result<[u8; 7], ScannerError> {
        let command = control_bytes_command();
        self.write_command(&command, UsbOperation::QueryControlBytes)?;
        // The Android SDK asks for 64 bytes, but this USB 2.0 device can send
        // the response in one 512-byte bulk packet. A 64-byte host buffer
        // causes libusb to report Overflow and discards the packet.
        let mut response = [0_u8; 512];
        let received = self
            .handle
            .read_bulk(self.bulk_in_endpoint, &mut response, DEFAULT_TIMEOUT)
            .map_err(|error| usb_error(UsbOperation::QueryControlBytes, error))?;
        let Some(bytes) = response.get(..7) else {
            return Err(ScannerError::IncompleteResponse {
                received,
                expected: 7,
            });
        };
        if received < bytes.len() {
            return Err(ScannerError::IncompleteResponse {
                received,
                expected: bytes.len(),
            });
        }
        let mut control_bytes = [0_u8; 7];
        control_bytes.copy_from_slice(bytes);
        Ok(control_bytes)
    }

    /// Reads the 64-byte descriptor queried by the vendor SDK during device open.
    pub fn read_descriptor_bytes(&mut self) -> Result<[u8; 64], ScannerError> {
        let command = [crate::protocol::GET_DESCRIPTOR];
        self.write_command(&command, UsbOperation::QueryControlBytes)?;
        let mut response = [0_u8; 512];
        let received = self
            .handle
            .read_bulk(self.bulk_in_endpoint, &mut response, DEFAULT_TIMEOUT)
            .map_err(|error| usb_error(UsbOperation::QueryControlBytes, error))?;
        if received < 64 {
            return Err(ScannerError::IncompleteResponse {
                received,
                expected: 64,
            });
        }
        let mut desc = [0_u8; 64];
        desc.copy_from_slice(&response[..64]);
        Ok(desc)
    }

    /// Requests one frame using the Android demo's recovered `GetFrame` setup.
    ///
    /// The Java demo enables `GetFrame` by default. This crate mirrors its
    /// frame-mode register sequence using the known `0x67` exposure from the
    /// alternate `GetImage2(4)` path. The returned frame remains an
    /// unprocessed sensor image; proprietary geometric calibration is not
    /// embedded in this crate.
    pub fn capture_frame(&mut self) -> Result<RawFrame, ScannerError> {
        self.capture_frame_with_frame_mode(DEFAULT_IMAGE_DOSE)
    }

    /// Requests and reads one frame using a sensor exposure/dose value.
    ///
    /// The value is passed directly to the FS80H variable-dose image command.
    /// Use this method when calibrating image exposure for a particular device.
    pub fn capture_frame_with_dose(&mut self, dose: u8) -> Result<RawFrame, ScannerError> {
        let stream = self.capture_raw_sensor_stream(dose)?;
        RawFrame::from_raw_sensor_stream(&stream)
    }

    /// Requests a frame with the register sequence used by the vendor SDK's
    /// `GetFrame` path.
    ///
    /// It programs the frame-mode analog gain for the selected dose, allows a
    /// settling interval for the CMOS sensor, captures the complete 384x800
    /// raw sensor stream in 4096-byte chunks, resamples it to the nominal 320x480
    /// biometric frame, and restores the gain register to its idle value even if
    /// capture fails.
    pub fn capture_frame_with_frame_mode(&mut self, dose: u8) -> Result<RawFrame, ScannerError> {
        let setup = frame_mode_setup_command(dose);
        self.write_command(&setup, UsbOperation::ConfigureFrameMode)?;

        // Settling time for analog optical integration and CMOS register stabilization
        std::thread::sleep(std::time::Duration::from_millis(60));

        let stream_result = (|| {
            self.flush_bulk_in();
            self.trigger_frame(dose)?;
            let endpoint = self.bulk_in_endpoint;
            read_complete_frame(RAW_SENSOR_STREAM_SIZE, |chunk| {
                self.handle
                    .read_bulk(endpoint, chunk, FRAME_CHUNK_TIMEOUT)
                    .map_err(|error| usb_error(UsbOperation::ReadFrame, error))
            })
        })();

        let restore = frame_mode_reset_command();
        let restore_result = self.write_command(&restore, UsbOperation::ConfigureFrameMode);

        match stream_result {
            Ok(stream) => {
                restore_result?;
                RawFrame::from_raw_sensor_stream(&stream)
            }
            Err(error) => {
                let _ = restore_result;
                Err(error)
            }
        }
    }

    /// Captures the complete unresampled CMOS sensor stream directly from the device.
    ///
    /// For the FS80H/FS81H, this returns 307,200 bytes (384 columns by 800 rows).
    pub fn capture_raw_sensor_stream(&mut self, dose: u8) -> Result<Vec<u8>, ScannerError> {
        self.flush_bulk_in();
        self.trigger_frame(dose)?;
        let endpoint = self.bulk_in_endpoint;
        read_complete_frame(RAW_SENSOR_STREAM_SIZE, |chunk| {
            self.handle
                .read_bulk(endpoint, chunk, FRAME_CHUNK_TIMEOUT)
                .map_err(|error| usb_error(UsbOperation::ReadFrame, error))
        })
    }

    /// Flushes any stale unread bytes from the Bulk IN endpoint FIFO.
    fn flush_bulk_in(&mut self) {
        let mut discard = [0_u8; BULK_CHUNK_SIZE];
        while let Ok(count) = self.handle.read_bulk(
            self.bulk_in_endpoint,
            &mut discard,
            std::time::Duration::from_millis(15),
        ) {
            if count == 0 {
                break;
            }
        }
    }

    /// Sets the scanner's green and red status diodes.
    ///
    /// This control is opt-in because the exact illumination behavior varies
    /// across Futronic hardware revisions. This scanner accepts one command
    /// sequence per claimed interface, so applications that need to change the
    /// diode state should reopen the device before their next command.
    pub fn set_diodes(&mut self, green: bool, red: bool) -> Result<(), ScannerError> {
        let command = diode_command(green, red);
        self.write_command(&command, UsbOperation::SetDiodes)
    }

    fn trigger_frame(&mut self, dose: u8) -> Result<(), ScannerError> {
        let command = capture_command(dose);
        self.write_command(&command, UsbOperation::TriggerFrame)
    }

    fn write_command(
        &mut self,
        command: &[u8],
        operation: UsbOperation,
    ) -> Result<(), ScannerError> {
        let written = self
            .handle
            .write_bulk(self.bulk_out_endpoint, command, DEFAULT_TIMEOUT)
            .map_err(|error| usb_error(operation, error))?;
        if written == command.len() {
            Ok(())
        } else {
            Err(ScannerError::IncompleteCommand {
                written,
                expected: command.len(),
            })
        }
    }
}

impl Drop for FutronicDevice {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.interface_number);
        #[cfg(target_os = "linux")]
        if self.detached_kernel_driver {
            let _ = self.handle.attach_kernel_driver(self.interface_number);
        }
    }
}

fn find_capture_interface(device: &Device<GlobalContext>) -> Result<(u8, u8, u8), ScannerError> {
    let config = device
        .active_config_descriptor()
        .map_err(|error| usb_error(UsbOperation::ReadDescriptor, error))?;
    let mut candidates = Vec::new();
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            let mut bulk_in = Vec::new();
            let mut bulk_out = Vec::new();
            for endpoint in descriptor.endpoint_descriptors() {
                if endpoint.transfer_type() != TransferType::Bulk {
                    continue;
                }
                match endpoint.direction() {
                    Direction::In => bulk_in.push(endpoint.address()),
                    Direction::Out => bulk_out.push(endpoint.address()),
                }
            }
            if bulk_in.len() == 1 && bulk_out.len() == 1 {
                candidates.push((descriptor.interface_number(), bulk_out[0], bulk_in[0]));
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();

    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }
    if let Some(candidate) = candidates.iter().copied().find(|(_, out, input)| {
        *out == EXPECTED_BULK_OUT_ENDPOINT && *input == EXPECTED_BULK_IN_ENDPOINT
    }) {
        return Ok(candidate);
    }
    Err(ScannerError::UnsupportedUsbLayout)
}

fn capture_command(dose: u8) -> [u8; 2] {
    [GET_IMAGE_VARIABLE_DOSE, dose]
}

fn diode_command(green: bool, red: bool) -> [u8; 3] {
    [SET_DIODES, u8::from(green), u8::from(red)]
}

fn control_bytes_command() -> [u8; 1] {
    [GET_CONTROL_BYTES]
}

fn frame_mode_setup_command(dose: u8) -> [u8; 2] {
    [SET_FRAME_MODE_GAIN, 0x80 | (dose >> 1)]
}

fn frame_mode_reset_command() -> [u8; 2] {
    [SET_FRAME_MODE_GAIN, FRAME_MODE_GAIN_RESET]
}

#[cfg(target_os = "linux")]
fn detach_kernel_driver(
    handle: &mut DeviceHandle<GlobalContext>,
    interface_number: u8,
) -> Result<bool, ScannerError> {
    let active = handle
        .kernel_driver_active(interface_number)
        .map_err(|error| usb_error(UsbOperation::DetachKernelDriver, error))?;
    if active {
        handle
            .detach_kernel_driver(interface_number)
            .map_err(|error| usb_error(UsbOperation::DetachKernelDriver, error))?;
    }
    Ok(active)
}

fn usb_error(operation: UsbOperation, error: rusb::Error) -> ScannerError {
    if error == rusb::Error::Timeout {
        ScannerError::Timeout
    } else {
        ScannerError::Usb {
            operation,
            message: error.to_string(),
        }
    }
}

/// Builds a frame from an abstract sequence of bulk reads. Keeping this logic
/// independent from `rusb` makes partial-transfer behavior testable without a
/// connected scanner.
///
/// Data is consumed in chunks up to `BULK_CHUNK_SIZE` (4096 bytes), matching the
/// vendor SDK's transfer granularity.
fn read_complete_frame<F>(expected_size: usize, mut read: F) -> Result<Vec<u8>, ScannerError>
where
    F: FnMut(&mut [u8]) -> Result<usize, ScannerError>,
{
    let mut buffer = vec![0_u8; expected_size];
    let mut total_read = 0;
    while total_read < buffer.len() {
        let chunk_size = (buffer.len() - total_read).min(BULK_CHUNK_SIZE);
        let count = read(&mut buffer[total_read..total_read + chunk_size])?;
        if count == 0 {
            break;
        }
        total_read += count;
    }
    if total_read == expected_size {
        Ok(buffer)
    } else {
        Err(ScannerError::IncompleteFrame {
            received: total_read,
            expected: expected_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        capture_command, control_bytes_command, diode_command, frame_mode_reset_command,
        frame_mode_setup_command, read_complete_frame, usb_error,
    };
    use crate::{ScannerError, UsbOperation, RAW_FRAME_SIZE};

    #[test]
    fn maps_usb_timeouts_to_the_public_timeout_variant() {
        assert_eq!(
            usb_error(UsbOperation::ReadFrame, rusb::Error::Timeout),
            ScannerError::Timeout
        );
    }

    #[test]
    fn assembles_partial_bulk_reads_from_a_mock_transport() {
        let mut remaining = RAW_FRAME_SIZE;
        let mut first = true;
        let frame = read_complete_frame(RAW_FRAME_SIZE, |buffer| {
            if remaining == 0 {
                return Ok(0);
            }
            let count = if first {
                first = false;
                64.min(buffer.len())
            } else {
                buffer.len().min(remaining)
            };
            buffer[..count].fill(7);
            remaining -= count;
            Ok(count)
        });
        assert_eq!(frame.as_ref().map(Vec::len), Ok(RAW_FRAME_SIZE));
    }

    #[test]
    fn reports_a_zero_length_mock_read_as_incomplete() {
        let result = read_complete_frame(RAW_FRAME_SIZE, |_| Ok(0));
        assert_eq!(
            result,
            Err(ScannerError::IncompleteFrame {
                received: 0,
                expected: RAW_FRAME_SIZE,
            })
        );
    }

    #[test]
    fn uses_the_recovered_variable_dose_bulk_command() {
        assert_eq!(capture_command(0x80), [0x6A, 0x80]);
    }

    #[test]
    fn uses_the_vendor_calibrated_dose_by_default() {
        assert_eq!(super::DEFAULT_IMAGE_DOSE, 166);
    }

    #[test]
    fn uses_binary_values_for_the_recovered_diode_command() {
        assert_eq!(diode_command(true, false), [0xE3, 1, 0]);
        assert_eq!(diode_command(false, true), [0xE3, 0, 1]);
    }

    #[test]
    fn uses_the_vendor_initialization_control_query() {
        assert_eq!(control_bytes_command(), [0x50]);
    }

    #[test]
    fn uses_the_recovered_get_frame_gain_sequence() {
        assert_eq!(frame_mode_setup_command(166), [0x49, 0xD3]);
        assert_eq!(frame_mode_reset_command(), [0x49, 0x04]);
    }
}
