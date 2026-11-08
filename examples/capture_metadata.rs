use futronic_fs80h_rs::FutronicDevice;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let variable_dose = std::env::args().skip(1).any(|arg| arg == "--variable-dose");
    let mut scanner = FutronicDevice::open_single()?;
    let info = scanner.info().clone();
    let frame = if variable_dose {
        scanner.capture_frame_with_dose(166)?
    } else {
        scanner.capture_frame()?
    };

    println!(
        "capture succeeded: model={} pid={:04x} geometry={}x{} dpi={} mode={}",
        if info.product_id == 0x0025 {
            "FS81H"
        } else {
            "FS80H"
        },
        info.product_id,
        frame.width(),
        frame.height(),
        frame.dpi(),
        if variable_dose {
            "variable-dose"
        } else {
            "frame-mode"
        }
    );
    Ok(())
}
