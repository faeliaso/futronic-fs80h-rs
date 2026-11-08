use crate::device::{find_capture_interface, usb_error, FutronicDevice};
use crate::error::{ScannerError, UsbOperation};
use crate::protocol::{is_supported_product, FS80_PID, FS81_PID, FUTRONIC_VID};
use std::fmt;
use std::str::FromStr;

/// Error returned when parsing a [`DeviceId`] from a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDeviceIdError {
    details: String,
}

impl ParseDeviceIdError {
    #[must_use]
    pub fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }
}

impl fmt::Display for ParseDeviceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid device ID: {}", self.details)
    }
}

impl std::error::Error for ParseDeviceIdError {}

/// Unique USB topology identifier identifying a scanner on the host bus.
///
/// In USB subsystems (Linux, macOS, Windows via libusb), devices are addressable
/// by their host USB bus number and device address.
///
/// Standard string representations such as `"1:4"` or `"001:004"` can be parsed
/// using [`FromStr`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceId {
    /// Host USB bus number.
    pub bus_number: u8,
    /// Host USB device address.
    pub address: u8,
}

impl DeviceId {
    /// Creates a new device identifier from its USB bus number and device address.
    #[must_use]
    pub const fn new(bus_number: u8, address: u8) -> Self {
        Self {
            bus_number,
            address,
        }
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.bus_number, self.address)
    }
}

impl From<(u8, u8)> for DeviceId {
    fn from((bus, address): (u8, u8)) -> Self {
        Self::new(bus, address)
    }
}

impl FromStr for DeviceId {
    type Err = ParseDeviceIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let cleaned = input.trim();
        if cleaned.is_empty() {
            return Err(ParseDeviceIdError::new("empty device ID string"));
        }

        // Handle forms like:
        // "1:4", "001:004", "1/4", "Bus 001 Device 004", "bus 1 dev 4"
        let parts: Vec<&str> = if cleaned.contains(':') {
            cleaned.split(':').collect()
        } else if cleaned.contains('/') {
            cleaned.split('/').collect()
        } else {
            // Try extracting numeric tokens from e.g. "Bus 001 Device 004"
            let numbers: Vec<&str> = cleaned
                .split_whitespace()
                .filter(|token| token.chars().all(|c| c.is_ascii_digit()))
                .collect();
            if numbers.len() == 2 {
                numbers
            } else {
                return Err(ParseDeviceIdError::new(format!(
                    "unrecognized format '{cleaned}'; expected 'bus:address' (e.g. '1:4')"
                )));
            }
        };

        if parts.len() != 2 {
            return Err(ParseDeviceIdError::new(format!(
                "expected exactly 2 components (bus:address), got {}",
                parts.len()
            )));
        }

        let bus_str = parts[0].trim();
        let addr_str = parts[1].trim();

        let bus_number = bus_str.parse::<u8>().map_err(|_| {
            ParseDeviceIdError::new(format!("invalid bus number '{bus_str}'; must be 0-255"))
        })?;

        let address = addr_str.parse::<u8>().map_err(|_| {
            ParseDeviceIdError::new(format!(
                "invalid device address '{addr_str}'; must be 0-255"
            ))
        })?;

        Ok(Self::new(bus_number, address))
    }
}

/// Discovered Futronic scanner found on the USB bus without claiming exclusive access.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoveredDevice {
    /// Topology identifier (`bus_number:address`).
    pub id: DeviceId,
    /// USB vendor ID (0x1491 for Futronic).
    pub vendor_id: u16,
    /// USB product ID (0x0020 for FS80H, 0x0025 for FS81H).
    pub product_id: u16,
    /// Human-readable model designation (e.g. "FS80H", "FS81H").
    pub model: String,
    /// Physical USB hub port chain if reported by the operating system.
    pub port_numbers: Option<Vec<u8>>,
    /// Product string descriptor reported by the scanner, if readable without claiming.
    pub product_name: Option<String>,
}

impl fmt::Display for DiscoveredDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] at USB {}",
            self.model,
            self.product_name.as_deref().unwrap_or("Futronic Scanner"),
            self.id
        )
    }
}

/// Enumerates and inspects all compatible Futronic scanners connected to the USB bus.
///
/// This function queries the USB bus without claiming scanner interfaces, allowing safe
/// inspection of connected hardware before deciding which device to open.
pub fn list_devices() -> Result<Vec<DiscoveredDevice>, ScannerError> {
    let devices = rusb::devices().map_err(|error| usb_error(UsbOperation::Enumerate, error))?;
    let mut discovered = Vec::new();

    for device in devices.iter() {
        let descriptor = match device.device_descriptor() {
            Ok(desc) => desc,
            Err(_) => continue,
        };

        if descriptor.vendor_id() != FUTRONIC_VID || !is_supported_product(descriptor.product_id())
        {
            continue;
        }

        let model = match descriptor.product_id() {
            FS80_PID => "FS80H",
            FS81_PID => "FS81H",
            _ => "Unknown Futronic",
        }
        .to_string();

        let port_numbers = device.port_numbers().ok();
        let product_name = device
            .open()
            .ok()
            .and_then(|handle| handle.read_product_string_ascii(&descriptor).ok());

        discovered.push(DiscoveredDevice {
            id: DeviceId::new(device.bus_number(), device.address()),
            vendor_id: descriptor.vendor_id(),
            product_id: descriptor.product_id(),
            model,
            port_numbers,
            product_name,
        });
    }

    discovered.sort_by_key(|d| (d.id.bus_number, d.id.address));
    Ok(discovered)
}

/// Opens a specific Futronic scanner identified by its [`DeviceId`].
pub fn open_device_by_id(id: &DeviceId) -> Result<FutronicDevice, ScannerError> {
    let devices = rusb::devices().map_err(|error| usb_error(UsbOperation::Enumerate, error))?;

    for device in devices.iter() {
        if device.bus_number() != id.bus_number || device.address() != id.address {
            continue;
        }

        let descriptor = device
            .device_descriptor()
            .map_err(|error| usb_error(UsbOperation::ReadDescriptor, error))?;

        if descriptor.vendor_id() != FUTRONIC_VID || !is_supported_product(descriptor.product_id())
        {
            return Err(ScannerError::DeviceWithIdNotFound(*id));
        }

        let (interface_number, bulk_out_endpoint, bulk_in_endpoint) =
            find_capture_interface(&device)?;

        return FutronicDevice::open_device(
            device,
            descriptor,
            interface_number,
            bulk_out_endpoint,
            bulk_in_endpoint,
        );
    }

    Err(ScannerError::DeviceWithIdNotFound(*id))
}

/// Opens the connected Futronic scanner if exactly one is present.
///
/// - If 0 scanners are found, returns `Err(ScannerError::DeviceNotFound)`.
/// - If 1 scanner is found, opens it and returns `Ok(FutronicDevice)`.
/// - If more than 1 scanner is found, returns `Err(ScannerError::MultipleDevicesFound(devices))`
///   containing all discovered devices, allowing the caller to select and call [`open_device_by_id`].
pub fn open_single_device() -> Result<FutronicDevice, ScannerError> {
    let devices = list_devices()?;

    match devices.len() {
        0 => Err(ScannerError::DeviceNotFound),
        1 => open_device_by_id(&devices[0].id),
        _ => Err(ScannerError::MultipleDevicesFound(devices)),
    }
}
