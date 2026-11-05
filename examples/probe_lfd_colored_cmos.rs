//! Inspect the recovered FS80H colored-CMOS LFD sequence or firmware profile.
//!
//! `cargo run --features lfd-research --example probe_lfd_colored_cmos -- --dry-run`

use futronic_fs80h_rs::{
    FutronicDevice, LfdColoredCmosDryRunPlan, LfdColoredCmosDryRunStep, FIRMWARE_COLOR_CONFIG_QUERY,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("--dry-run") => print_dry_run(),
        Some("--read-firmware-config") => read_firmware_config()?,
        _ => return Err(usage().into()),
    }
    Ok(())
}

fn print_dry_run() {
    let plan = LfdColoredCmosDryRunPlan::inferred_fs80h();
    println!("FS80H colored-CMOS LFD dry run; no scanner is opened and no USB is used.");
    println!(
        "Recovered firmware config query (not sent): {:02X} {:02X} {:02X} {:02X}",
        FIRMWARE_COLOR_CONFIG_QUERY[0],
        FIRMWARE_COLOR_CONFIG_QUERY[1],
        FIRMWARE_COLOR_CONFIG_QUERY[2],
        FIRMWARE_COLOR_CONFIG_QUERY[3]
    );
    for (index, step) in plan.steps().iter().enumerate() {
        match step {
            LfdColoredCmosDryRunStep::UnknownActivationRegister { register, source } => {
                println!(
                    "{:02}: register {register:#04X} = <unknown>; source: {source}",
                    index + 1
                );
            }
            LfdColoredCmosDryRunStep::AuxiliaryFrame { selector, dose } => {
                println!(
                    "{:02}: auxiliary frame selector={selector:#04X}, dose={dose}",
                    index + 1
                );
            }
            LfdColoredCmosDryRunStep::KnownRestore { command } => {
                println!(
                    "{:02}: restore command = {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} (register {:#04X} = {:#04X})",
                    index + 1,
                    command[0],
                    command[1],
                    command[2],
                    command[3],
                    command[4],
                    command[5],
                    command[6],
                    command[7],
                    command[1],
                    command[2],
                );
            }
        }
    }
    println!("safe_to_apply={}", plan.is_safe_to_apply());
}

fn read_firmware_config() -> Result<(), Box<dyn std::error::Error>> {
    let mut scanner = FutronicDevice::open_first()?;
    let descriptor = scanner.read_descriptor_bytes()?;
    let rev_byte = descriptor[0x17];
    println!("Read 64-byte descriptor. Device revision (descriptor[0x17]): {rev_byte:#04X} ({rev_byte})");

    let config = scanner.read_lfd_firmware_color_config()?;
    let bytes = config.bytes();
    println!("Read 64-byte FS80H firmware color configuration:");
    for chunk in bytes.chunks(16) {
        let hex_str: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        println!("  {}", hex_str.join(" "));
    }
    let byte_19 = bytes[0x19];
    let eligible_mask = (byte_19 & 0x81) == 0x01;
    let rev_supported = rev_byte != 0x13;
    let eligible = eligible_mask && rev_supported;
    println!();
    println!(
        "Vendor WriteBL2CMOS gate check:\n  - revision != 0x13: {rev_supported} (revision = {rev_byte:#04X})\n  - config[0x19] & 0x81 == 0x01: {eligible_mask} (config[0x19] = {byte_19:#04X})\n  => Overall WriteBL2CMOS eligible: {eligible}"
    );
    if !eligible {
        println!("\nWARNING: According to vendor SDK static analysis, this device does NOT execute WriteBL2CMOS! The SDK skips colored-CMOS register writes on this hardware.");
    }
    println!();
    println!("activation_register_commands (8-byte InternalSave7ControlBytes framing):");
    for (command, (reg, val)) in config
        .activation_register_commands()
        .iter()
        .zip(config.activation_register_pairs().iter())
    {
        println!(
            "  {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} (register {reg:#04X} = {val:#04X})",
            command[0], command[1], command[2], command[3], command[4], command[5], command[6], command[7]
        );
    }
    println!("restore_command: 40 4A 03 00 00 00 00 00 (register 0x4A = 0x03)");
    println!("No colored-CMOS register writes were sent.");
    Ok(())
}

fn usage() -> &'static str {
    "usage: probe_lfd_colored_cmos --dry-run | --read-firmware-config"
}
