use crate::error::{ScannerError, UsbOperation};
use crate::protocol::{
    is_supported_product, BULK_CHUNK_SIZE, DEFAULT_IMAGE_DOSE, DEFAULT_TIMEOUT,
    EXPECTED_BULK_IN_ENDPOINT, EXPECTED_BULK_OUT_ENDPOINT, FRAME_CHUNK_TIMEOUT,
    FRAME_MODE_GAIN_RESET, FS80_PID, FS80_RAW_SENSOR_STREAM_SIZE, FUTRONIC_VID, GET_CONTROL_BYTES,
    GET_IMAGE_VARIABLE_DOSE, RAW_SENSOR_STREAM_SIZE, SET_DIODES, SET_FRAME_MODE_GAIN,
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

impl DeviceInfo {
    #[must_use]
    pub const fn id(&self) -> crate::discovery::DeviceId {
        crate::discovery::DeviceId::new(self.bus_number, self.address)
    }
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
    pub fn open_first() -> Result<Self, ScannerError> {
        let devices = rusb::devices().map_err(|e| usb_error(UsbOperation::Enumerate, e))?;
        for device in devices.iter() {
            let descriptor = device
                .device_descriptor()
                .map_err(|e| usb_error(UsbOperation::ReadDescriptor, e))?;
            if descriptor.vendor_id() == FUTRONIC_VID
                && is_supported_product(descriptor.product_id())
            {
                let (interface, out, input) = find_capture_interface(&device)?;
                return Self::open_device(device, descriptor, interface, out, input);
            }
        }
        Err(ScannerError::DeviceNotFound)
    }

    pub fn list_devices() -> Result<Vec<crate::discovery::DiscoveredDevice>, ScannerError> {
        crate::discovery::list_devices()
    }

    pub fn open_by_id(id: &crate::discovery::DeviceId) -> Result<Self, ScannerError> {
        crate::discovery::open_device_by_id(id)
    }

    pub fn open_single() -> Result<Self, ScannerError> {
        crate::discovery::open_single_device()
    }

    pub(crate) fn open_device(
        device: Device<GlobalContext>,
        descriptor: rusb::DeviceDescriptor,
        interface_number: u8,
        bulk_out_endpoint: u8,
        bulk_in_endpoint: u8,
    ) -> Result<Self, ScannerError> {
        #[allow(unused_mut)]
        let mut handle = device
            .open()
            .map_err(|e| usb_error(UsbOperation::OpenDevice, e))?;
        #[cfg(target_os = "linux")]
        let detached_kernel_driver = detach_kernel_driver(&mut handle, interface_number)?;
        handle
            .claim_interface(interface_number)
            .map_err(|e| match e {
                rusb::Error::Busy => ScannerError::InterfaceBusy,
                other => usb_error(UsbOperation::ClaimInterface, other),
            })?;
        let info = DeviceInfo {
            vendor_id: descriptor.vendor_id(),
            product_id: descriptor.product_id(),
            bus_number: device.bus_number(),
            address: device.address(),
            product_name: handle.read_product_string_ascii(&descriptor).ok(),
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

    #[must_use]
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn read_control_bytes(&mut self) -> Result<[u8; 7], ScannerError> {
        self.write_command(&[GET_CONTROL_BYTES], UsbOperation::QueryControlBytes)?;
        let mut response = [0; 512];
        let received = self
            .handle
            .read_bulk(self.bulk_in_endpoint, &mut response, DEFAULT_TIMEOUT)
            .map_err(|e| usb_error(UsbOperation::QueryControlBytes, e))?;
        if received < 7 {
            return Err(ScannerError::IncompleteResponse {
                received,
                expected: 7,
            });
        }
        let mut bytes = [0; 7];
        bytes.copy_from_slice(&response[..7]);
        Ok(bytes)
    }

    pub fn read_descriptor_bytes(&mut self) -> Result<[u8; 64], ScannerError> {
        self.write_command(
            &[crate::protocol::GET_DESCRIPTOR],
            UsbOperation::QueryControlBytes,
        )?;
        let mut response = [0; 512];
        let received = self
            .handle
            .read_bulk(self.bulk_in_endpoint, &mut response, DEFAULT_TIMEOUT)
            .map_err(|e| usb_error(UsbOperation::QueryControlBytes, e))?;
        if received < 64 {
            return Err(ScannerError::IncompleteResponse {
                received,
                expected: 64,
            });
        }
        let mut bytes = [0; 64];
        bytes.copy_from_slice(&response[..64]);
        Ok(bytes)
    }

    pub fn capture_frame(&mut self) -> Result<RawFrame, ScannerError> {
        self.capture_frame_with_frame_mode(DEFAULT_IMAGE_DOSE)
    }

    pub fn capture_frame_with_dose(&mut self, dose: u8) -> Result<RawFrame, ScannerError> {
        RawFrame::from_raw_sensor_stream(&self.capture_raw_sensor_stream(dose)?)
    }

    pub fn capture_frame_with_frame_mode(&mut self, dose: u8) -> Result<RawFrame, ScannerError> {
        self.write_command(
            &[SET_FRAME_MODE_GAIN, 0x80 | (dose >> 1)],
            UsbOperation::ConfigureFrameMode,
        )?;
        std::thread::sleep(std::time::Duration::from_millis(60));
        let stream_result = self.capture_raw_sensor_stream(dose);
        let restore = self.write_command(
            &[SET_FRAME_MODE_GAIN, FRAME_MODE_GAIN_RESET],
            UsbOperation::ConfigureFrameMode,
        );
        match stream_result {
            Ok(stream) => {
                restore?;
                RawFrame::from_raw_sensor_stream(&stream)
            }
            Err(error) => {
                let _ = restore;
                Err(error)
            }
        }
    }

    /// Captures the unresampled CMOS stream for diagnostics.
    pub fn capture_raw_sensor_stream(&mut self, dose: u8) -> Result<Vec<u8>, ScannerError> {
        self.flush_bulk_in();
        self.write_command(&[GET_IMAGE_VARIABLE_DOSE, dose], UsbOperation::TriggerFrame)?;
        let endpoint = self.bulk_in_endpoint;
        let expected_size = if self.info.product_id == FS80_PID {
            FS80_RAW_SENSOR_STREAM_SIZE
        } else {
            RAW_SENSOR_STREAM_SIZE
        };
        read_complete_frame(expected_size, |chunk| {
            self.handle
                .read_bulk(endpoint, chunk, FRAME_CHUNK_TIMEOUT)
                .map_err(|e| usb_error(UsbOperation::ReadFrame, e))
        })
    }

    pub fn set_diodes(&mut self, green: bool, red: bool) -> Result<(), ScannerError> {
        self.write_command(
            &[SET_DIODES, u8::from(green), u8::from(red)],
            UsbOperation::SetDiodes,
        )
    }

    fn flush_bulk_in(&mut self) {
        let mut discard = [0; BULK_CHUNK_SIZE];
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

    fn write_command(
        &mut self,
        command: &[u8],
        operation: UsbOperation,
    ) -> Result<(), ScannerError> {
        let written = self
            .handle
            .write_bulk(self.bulk_out_endpoint, command, DEFAULT_TIMEOUT)
            .map_err(|e| usb_error(operation, e))?;
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

pub(crate) fn find_capture_interface(
    device: &Device<GlobalContext>,
) -> Result<(u8, u8, u8), ScannerError> {
    let config = device
        .active_config_descriptor()
        .map_err(|e| usb_error(UsbOperation::ReadDescriptor, e))?;
    let mut candidates = Vec::new();
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            let mut input = Vec::new();
            let mut output = Vec::new();
            for endpoint in descriptor.endpoint_descriptors() {
                if endpoint.transfer_type() == TransferType::Bulk {
                    match endpoint.direction() {
                        Direction::In => input.push(endpoint.address()),
                        Direction::Out => output.push(endpoint.address()),
                    }
                }
            }
            if input.len() == 1 && output.len() == 1 {
                candidates.push((descriptor.interface_number(), output[0], input[0]));
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }
    candidates
        .into_iter()
        .find(|(_, out, input)| {
            *out == EXPECTED_BULK_OUT_ENDPOINT && *input == EXPECTED_BULK_IN_ENDPOINT
        })
        .ok_or(ScannerError::UnsupportedUsbLayout)
}

#[cfg(target_os = "linux")]
fn detach_kernel_driver(
    handle: &mut DeviceHandle<GlobalContext>,
    interface: u8,
) -> Result<bool, ScannerError> {
    let active = handle
        .kernel_driver_active(interface)
        .map_err(|e| usb_error(UsbOperation::DetachKernelDriver, e))?;
    if active {
        handle
            .detach_kernel_driver(interface)
            .map_err(|e| usb_error(UsbOperation::DetachKernelDriver, e))?;
    }
    Ok(active)
}

pub(crate) fn usb_error(operation: UsbOperation, error: rusb::Error) -> ScannerError {
    if error == rusb::Error::Timeout {
        ScannerError::Timeout
    } else {
        ScannerError::Usb {
            operation,
            message: error.to_string(),
        }
    }
}

fn read_complete_frame<F>(expected: usize, mut read: F) -> Result<Vec<u8>, ScannerError>
where
    F: FnMut(&mut [u8]) -> Result<usize, ScannerError>,
{
    let mut buffer = vec![0; expected];
    let mut received = 0;
    while received < expected {
        let size = (expected - received).min(BULK_CHUNK_SIZE);
        let count = match read(&mut buffer[received..received + size]) {
            Ok(count) => count,
            Err(ScannerError::Timeout) => {
                return Err(ScannerError::FrameReadTimeout { received, expected });
            }
            Err(error) => return Err(error),
        };
        if count == 0 {
            break;
        }
        received += count;
    }
    if received == expected {
        Ok(buffer)
    } else {
        Err(ScannerError::IncompleteFrame { received, expected })
    }
}
