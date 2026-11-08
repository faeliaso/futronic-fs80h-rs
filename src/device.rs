use crate::error::{ScannerError, UsbOperation};
use crate::protocol::{
    is_supported_product, BULK_CHUNK_SIZE, DEFAULT_IMAGE_DOSE, DEFAULT_TIMEOUT,
    EXPECTED_BULK_IN_ENDPOINT, EXPECTED_BULK_OUT_ENDPOINT, FRAME_CHUNK_TIMEOUT,
    FRAME_MODE_GAIN_RESET, FUTRONIC_VID, GET_CONTROL_BYTES,
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
        let stream = restore_frame_mode_after_capture(stream_result, || {
            self.write_command(
                &[SET_FRAME_MODE_GAIN, FRAME_MODE_GAIN_RESET],
                UsbOperation::ConfigureFrameMode,
            )
        })?;
        RawFrame::from_raw_sensor_stream(&stream)
    }

    /// Captures the unresampled CMOS stream for diagnostics.
    pub fn capture_raw_sensor_stream(&mut self, dose: u8) -> Result<Vec<u8>, ScannerError> {
        self.flush_bulk_in();
        self.write_command(&[GET_IMAGE_VARIABLE_DOSE, dose], UsbOperation::TriggerFrame)?;
        let endpoint = self.bulk_in_endpoint;
        let expected_size = RAW_SENSOR_STREAM_SIZE;
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
    select_capture_interface(candidates)
}

/// Chooses the capture interface from the bulk endpoint candidates reported by
/// a USB configuration descriptor. Kept separate from descriptor traversal so
/// the selection policy can be tested without a physical scanner.
fn select_capture_interface(
    mut candidates: Vec<(u8, u8, u8)>,
) -> Result<(u8, u8, u8), ScannerError> {
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

/// Restores the frame-mode register after an attempted capture. A restore
/// failure is returned only when capture succeeded; otherwise the capture
/// error is the actionable failure and is preserved for the caller.
fn restore_frame_mode_after_capture<T>(
    capture_result: Result<T, ScannerError>,
    restore: impl FnOnce() -> Result<(), ScannerError>,
) -> Result<T, ScannerError> {
    match capture_result {
        Ok(value) => {
            restore()?;
            Ok(value)
        }
        Err(capture_error) => {
            if let Err(restore_error) = restore() {
                tracing::warn!(
                    ?capture_error,
                    ?restore_error,
                    "failed to restore scanner frame mode after capture failure"
                );
            }
            Err(capture_error)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{read_complete_frame, restore_frame_mode_after_capture, select_capture_interface};
    use crate::error::ScannerError;
    use crate::protocol::{EXPECTED_BULK_IN_ENDPOINT, EXPECTED_BULK_OUT_ENDPOINT};
    use std::collections::VecDeque;

    fn read_chunks(
        expected: usize,
        chunks: impl IntoIterator<Item = Result<Vec<u8>, ScannerError>>,
    ) -> Result<Vec<u8>, ScannerError> {
        let mut chunks = chunks.into_iter().collect::<VecDeque<_>>();
        read_complete_frame(expected, |destination| match chunks.pop_front() {
            Some(Ok(chunk)) => {
                assert!(chunk.len() <= destination.len());
                destination[..chunk.len()].copy_from_slice(&chunk);
                Ok(chunk.len())
            }
            Some(Err(error)) => Err(error),
            None => Ok(0),
        })
    }

    #[test]
    fn frame_pipeline_reads_normal_chunks() {
        let frame = read_chunks(8, [Ok(vec![1; 4]), Ok(vec![2; 4])]).unwrap();
        assert_eq!(frame, vec![1, 1, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn frame_pipeline_reads_short_chunks() {
        let frame = read_chunks(5, [Ok(vec![1, 2]), Ok(vec![3]), Ok(vec![4, 5])]).unwrap();
        assert_eq!(frame, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn frame_pipeline_rejects_empty_chunk_as_incomplete() {
        let error = read_chunks(5, [Ok(vec![1, 2]), Ok(vec![])]).unwrap_err();
        assert_eq!(
            error,
            ScannerError::IncompleteFrame {
                received: 2,
                expected: 5
            }
        );
    }

    #[test]
    fn frame_pipeline_reports_bytes_received_before_timeout() {
        let error = read_chunks(5, [Ok(vec![1, 2, 3]), Err(ScannerError::Timeout)]).unwrap_err();
        assert_eq!(
            error,
            ScannerError::FrameReadTimeout {
                received: 3,
                expected: 5
            }
        );
    }

    #[test]
    fn frame_pipeline_propagates_non_timeout_error() {
        let error = read_chunks(
            5,
            [Err(ScannerError::IncompleteCommand {
                written: 1,
                expected: 2,
            })],
        )
        .unwrap_err();
        assert_eq!(
            error,
            ScannerError::IncompleteCommand {
                written: 1,
                expected: 2
            }
        );
    }

    #[test]
    fn capture_interface_accepts_a_single_candidate() {
        assert_eq!(
            select_capture_interface(vec![(3, 0x04, 0x86)]),
            Ok((3, 0x04, 0x86))
        );
    }

    #[test]
    fn capture_interface_prefers_expected_endpoints_among_multiple_candidates() {
        assert_eq!(
            select_capture_interface(vec![
                (3, 0x04, 0x86),
                (1, EXPECTED_BULK_OUT_ENDPOINT, EXPECTED_BULK_IN_ENDPOINT),
            ]),
            Ok((1, EXPECTED_BULK_OUT_ENDPOINT, EXPECTED_BULK_IN_ENDPOINT))
        );
    }

    #[test]
    fn capture_interface_deduplicates_equivalent_descriptors() {
        assert_eq!(
            select_capture_interface(vec![(1, 0x01, 0x82), (1, 0x01, 0x82)]),
            Ok((1, 0x01, 0x82))
        );
    }

    #[test]
    fn capture_interface_rejects_ambiguous_or_empty_layouts() {
        assert_eq!(
            select_capture_interface(vec![(1, 0x01, 0x82), (2, 0x03, 0x84)]),
            Ok((1, 0x01, 0x82))
        );
        assert_eq!(
            select_capture_interface(vec![(1, 0x03, 0x84), (2, 0x04, 0x86)]),
            Err(ScannerError::UnsupportedUsbLayout)
        );
        assert_eq!(
            select_capture_interface(vec![]),
            Err(ScannerError::UnsupportedUsbLayout)
        );
    }

    struct SimulatedTransport {
        capture_result: Result<Vec<u8>, ScannerError>,
        restore_result: Result<(), ScannerError>,
        restore_calls: usize,
    }

    impl SimulatedTransport {
        fn restore_frame_mode(&mut self) -> Result<(), ScannerError> {
            self.restore_calls += 1;
            self.restore_result.clone()
        }
    }

    #[test]
    fn capture_failure_is_preserved_when_restore_also_fails() {
        let capture_error = ScannerError::FrameReadTimeout {
            received: 1024,
            expected: 307_200,
        };
        let mut transport = SimulatedTransport {
            capture_result: Err(capture_error.clone()),
            restore_result: Err(ScannerError::Usb {
                operation: crate::UsbOperation::ConfigureFrameMode,
                message: "simulated restore failure".to_string(),
            }),
            restore_calls: 0,
        };

        let capture_result = transport.capture_result.clone();
        let result =
            restore_frame_mode_after_capture(capture_result, || transport.restore_frame_mode());

        assert_eq!(result, Err(capture_error));
        assert_eq!(transport.restore_calls, 1);
    }

    #[test]
    fn restore_failure_is_returned_after_successful_capture() {
        let restore_error = ScannerError::Usb {
            operation: crate::UsbOperation::ConfigureFrameMode,
            message: "simulated restore failure".to_string(),
        };
        let mut transport = SimulatedTransport {
            capture_result: Ok(vec![1, 2, 3]),
            restore_result: Err(restore_error.clone()),
            restore_calls: 0,
        };

        let capture_result = transport.capture_result.clone();
        let result =
            restore_frame_mode_after_capture(capture_result, || transport.restore_frame_mode());

        assert_eq!(result, Err(restore_error));
        assert_eq!(transport.restore_calls, 1);
    }
}
