use futronic_fs80h_rs::{DeviceId, DeviceInfo, DiscoveredDevice, ParseDeviceIdError, ScannerError};

#[test]
fn device_id_constructs_and_formats() {
    let id = DeviceId::new(1, 4);
    assert_eq!(id.bus_number, 1);
    assert_eq!(id.address, 4);
    assert_eq!(id.to_string(), "1:4");

    let from_tuple: DeviceId = (2, 15).into();
    assert_eq!(from_tuple, DeviceId::new(2, 15));
    assert_eq!(from_tuple.to_string(), "2:15");
}

#[test]
fn device_id_parses_valid_formats() {
    // "bus:address"
    assert_eq!("1:4".parse::<DeviceId>().unwrap(), DeviceId::new(1, 4));
    assert_eq!("001:004".parse::<DeviceId>().unwrap(), DeviceId::new(1, 4));

    // "bus/address"
    assert_eq!("2/15".parse::<DeviceId>().unwrap(), DeviceId::new(2, 15));

    // Spaces around delimiters
    assert_eq!(" 3 : 9 ".parse::<DeviceId>().unwrap(), DeviceId::new(3, 9));

    // "Bus 001 Device 004" (lsusb format)
    assert_eq!(
        "Bus 001 Device 004".parse::<DeviceId>().unwrap(),
        DeviceId::new(1, 4)
    );
    assert_eq!(
        "bus 2 dev 15".parse::<DeviceId>().unwrap(),
        DeviceId::new(2, 15)
    );
}

#[test]
fn device_id_rejects_invalid_formats() {
    assert!("".parse::<DeviceId>().is_err());
    assert!("   ".parse::<DeviceId>().is_err());
    assert!("foo".parse::<DeviceId>().is_err());
    assert!("1:".parse::<DeviceId>().is_err());
    assert!(":4".parse::<DeviceId>().is_err());
    assert!("1:2:3".parse::<DeviceId>().is_err());
    assert!("1:999".parse::<DeviceId>().is_err()); // Out of u8 range
    assert!("-1:4".parse::<DeviceId>().is_err());
}

#[test]
fn parse_device_id_error_implements_display_and_error() {
    let err = ParseDeviceIdError::new("test failure");
    assert_eq!(err.to_string(), "invalid device ID: test failure");
    let dyn_err: &dyn std::error::Error = &err;
    assert_eq!(dyn_err.to_string(), "invalid device ID: test failure");
}

#[test]
fn scanner_error_formats_discovery_variants() {
    let err_id = ScannerError::DeviceWithIdNotFound(DeviceId::new(1, 4));
    assert_eq!(
        err_id.to_string(),
        "scanner with ID '1:4' was not found on the USB bus"
    );

    let err_multi = ScannerError::MultipleDevicesFound(vec![
        DiscoveredDevice {
            id: DeviceId::new(1, 4),
            vendor_id: 0x1491,
            product_id: 0x0020,
            model: "FS80H".to_string(),
            port_numbers: Some(vec![1, 2]),
            product_name: Some("Futronic FS80H".to_string()),
        },
        DiscoveredDevice {
            id: DeviceId::new(1, 5),
            vendor_id: 0x1491,
            product_id: 0x0025,
            model: "FS81H".to_string(),
            port_numbers: None,
            product_name: None,
        },
    ]);
    assert_eq!(
        err_multi.to_string(),
        "multiple Futronic scanners found (2); specify a device ID using open_by_id"
    );
}

#[test]
fn device_info_id_converts_correctly() {
    let info = DeviceInfo {
        vendor_id: 0x1491,
        product_id: 0x0020,
        bus_number: 1,
        address: 4,
        product_name: Some("Futronic FS80H".to_string()),
    };
    assert_eq!(info.id(), DeviceId::new(1, 4));
}

#[test]
fn discovered_device_formats_display() {
    let dev = DiscoveredDevice {
        id: DeviceId::new(1, 4),
        vendor_id: 0x1491,
        product_id: 0x0020,
        model: "FS80H".to_string(),
        port_numbers: Some(vec![1, 2]),
        product_name: Some("Futronic USB Fingerprint Scanner".to_string()),
    };

    assert_eq!(
        dev.to_string(),
        "FS80H [Futronic USB Fingerprint Scanner] at USB 1:4"
    );
}
