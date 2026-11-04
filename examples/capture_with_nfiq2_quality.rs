//! Reject a low-quality live capture before creating or storing a fingerprint template.
//!
//! Set `NFIQ2_DIR` to an NFIQ2 2.3 installation and pass its model-info file:
//! `NFIQ2_DIR=/usr/local/nfiq2 cargo run --features nfiq2 --example capture_with_nfiq2_quality -- /path/to/model.info`

use futronic_fs80h_rs::{FutronicDevice, Nfiq2, Nfiq2Error, Nfiq2Options};
use std::{
    env,
    io::{self, Write},
};

fn wait_for_enter(prompt: &str) -> io::Result<()> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = env::args()
        .nth(1)
        .ok_or("usage: capture_with_nfiq2_quality <model-info-file>")?;
    let assessor = Nfiq2::new(model, Nfiq2Options::default())?;
    let mut scanner = FutronicDevice::open_first()?;

    loop {
        wait_for_enter("Place a finger on the scanner, then press Enter: ")?;
        let frame = scanner.capture_frame()?;
        let assessment = match frame.assess_nfiq2(&assessor) {
            Ok(assessment) => assessment,
            Err(Nfiq2Error::CaptureRejected { feedback, .. }) => {
                println!(
                    "Capture rejected; reposition the finger and try again. feedback={feedback:?}"
                );
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        println!(
            "NFIQ2 score={}; accepted={}; feedback={:?}",
            assessment.score, assessment.accepted, assessment.feedback
        );
        if assessment.accepted {
            println!("Capture accepted. It can now be converted to a template or stored.");
            break;
        }
        println!("Capture rejected; reposition the finger and try again.");
    }
    Ok(())
}
