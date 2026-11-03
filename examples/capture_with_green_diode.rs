use futronic_fs80h_rs::FutronicDevice;
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for a Futronic FS80H/FS81H scanner...");
    let mut scanner = FutronicDevice::open_first()?;
    println!("Connected to: {:?}", scanner.info());
    println!("Place and hold one finger steadily on the prism.");
    println!("Press Enter to enable the green diode and capture one frame.");
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;

    scanner.set_diodes(true, false)?;
    let frame = scanner.capture_frame_with_dose(0xA0)?;
    let output = "fingerprint_green_diode_A0.png";
    image::save_buffer(
        output,
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::L8,
    )?;
    println!("Wrote {output}");
    Ok(())
}
