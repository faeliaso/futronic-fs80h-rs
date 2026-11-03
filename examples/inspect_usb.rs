//! Read-only USB descriptor inspection for a connected Futronic scanner.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut found = false;
    for device in rusb::devices()?.iter() {
        let descriptor = device.device_descriptor()?;
        if descriptor.vendor_id() != 0x1491 || !matches!(descriptor.product_id(), 0x0020 | 0x0025) {
            continue;
        }
        found = true;
        let configuration = device.active_config_descriptor()?;
        println!(
            "device {:04X}:{:04X}; configuration {}",
            descriptor.vendor_id(),
            descriptor.product_id(),
            configuration.number(),
        );
        for interface in configuration.interfaces() {
            for alternate in interface.descriptors() {
                println!(
                    "interface {} alt {} class {:02X}/{:02X}/{:02X}",
                    alternate.interface_number(),
                    alternate.setting_number(),
                    alternate.class_code(),
                    alternate.sub_class_code(),
                    alternate.protocol_code(),
                );
                for endpoint in alternate.endpoint_descriptors() {
                    println!(
                        "  endpoint 0x{:02X}: {:?} {:?}, max packet {}",
                        endpoint.address(),
                        endpoint.direction(),
                        endpoint.transfer_type(),
                        endpoint.max_packet_size(),
                    );
                }
            }
        }
    }

    if !found {
        eprintln!("No supported Futronic scanner was visible through libusb.");
    }
    Ok(())
}
