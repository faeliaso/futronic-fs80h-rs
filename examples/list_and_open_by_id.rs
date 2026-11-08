//! Demonstrates listing Futronic scanners on the USB bus, smart single-device
//! opening, and explicit device selection by DeviceId (`open_by_id`).

use futronic_fs80h_rs::{DeviceId, FutronicDevice, ScannerError};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Futronic FS80H/FS81H Device Discovery ===");

    // 1. List all scanners connected to the USB bus
    let devices = FutronicDevice::list_devices()?;
    println!("Found {} connected Futronic scanner(s):", devices.len());
    for (index, dev) in devices.iter().enumerate() {
        println!(
            "  [{}] Model: {} | ID: {} | Product: {} | Ports: {:?}",
            index,
            dev.model,
            dev.id,
            dev.product_name.as_deref().unwrap_or("N/A"),
            dev.port_numbers
        );
    }

    if devices.is_empty() {
        println!("\nNo Futronic scanners found. Please plug in a device.");
        return Ok(());
    }

    // 2. Check if a specific ID was requested via command line arguments
    // e.g. `cargo run --example list_and_open_by_id -- 1:4`
    let target_id: Option<DeviceId> = env::args().nth(1).map(|arg| {
        arg.parse::<DeviceId>()
            .expect("Invalid device ID passed in command line arguments (expected 'bus:address')")
    });

    let mut scanner = if let Some(id) = target_id {
        println!("\nOpening explicitly requested scanner by ID '{}'...", id);
        FutronicDevice::open_by_id(&id)?
    } else {
        println!("\nAttempting single-device automatic open (FutronicDevice::open_single)...");
        match FutronicDevice::open_single() {
            Ok(device) => {
                println!(
                    "Single scanner detected and opened successfully! Scanner ID: {}",
                    device.info().id()
                );
                device
            }
            Err(ScannerError::MultipleDevicesFound(multiple)) => {
                println!(
                    "\nMultiple scanners detected ({} devices found).",
                    multiple.len()
                );
                println!(
                    "To avoid ambiguity, picking the first device '{}' using open_by_id...",
                    multiple[0].id
                );
                FutronicDevice::open_by_id(&multiple[0].id)?
            }
            Err(err) => return Err(err.into()),
        }
    };

    println!("\nOpened scanner info:");
    println!("  Model/Product : {:?}", scanner.info().product_name);
    println!(
        "  USB VID:PID   : {:04X}:{:04X}",
        scanner.info().vendor_id,
        scanner.info().product_id
    );
    println!(
        "  USB Bus:Addr  : {}:{}",
        scanner.info().bus_number,
        scanner.info().address
    );
    println!("  Device ID     : {}", scanner.info().id());

    // 3. Test capturing one frame from the opened scanner
    println!(
        "\nCapturing a test frame from device {}...",
        scanner.info().id()
    );
    let frame = scanner.capture_frame()?;
    println!(
        "Frame captured: {}x{} @ {} DPI ({} bytes)",
        frame.width(),
        frame.height(),
        frame.dpi(),
        frame.as_bytes().len()
    );

    println!("\nScanner operation completed successfully!");
    Ok(())
}
