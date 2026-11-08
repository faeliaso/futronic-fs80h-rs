use futronic_fs80h_rs::list_devices;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let devices = list_devices()?;

    if devices.is_empty() {
        println!("No Futronic FS80H/FS81H scanners found.");
        return Ok(());
    }

    for device in devices {
        println!(
            "model={} vid={:04x} pid={:04x} usb={}",
            device.model, device.vendor_id, device.product_id, device.id
        );
    }
    Ok(())
}
