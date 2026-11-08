//! Stable, allocation-safe C ABI. Rust types never cross this boundary.

use crate::{
    list_devices, FingerPresenceMetrics, FutronicDevice, Matcher, MatcherOptions, RawFrame,
    TemplateOptions,
};
use std::{
    cell::RefCell,
    ffi::{c_char, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};

pub const ABI_VERSION: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Kept for forward-compatible feature-gated API entries.
pub enum Fs80hStatus {
    Ok = 0,
    InvalidArgument = 1,
    BufferTooSmall = 2,
    NotFound = 3,
    Timeout = 4,
    Unsupported = 5,
    Error = 255,
}

#[repr(C)]
pub struct Fs80hDevice {
    inner: FutronicDevice,
}

#[repr(C)]
pub struct Fs80hFrame {
    inner: RawFrame,
}

#[repr(C)]
pub struct Fs80hMatcher {
    inner: Matcher,
}

#[cfg(feature = "mcc")]
#[repr(C)]
pub struct Fs80hMccTemplate {
    inner: crate::mcc::MccTemplate,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fs80hBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Fs80hDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus_number: u8,
    pub address: u8,
    /// 80 for FS80H, 81 for FS81H, zero when unknown.
    pub model: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Fs80hPresenceMetrics {
    pub contact_ratio: f32,
    pub active_blocks: u32,
    pub total_blocks: u32,
    pub is_present: u8,
}

/// Stable, bounded controls for a non-interactive automatic capture.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fs80hAutoCaptureOptions {
    pub timeout_ms: u64,
    pub min_contact_ratio: f32,
    pub poll_dose: u8,
    pub capture_dose: u8,
    pub poll_interval_ms: u32,
    pub consecutive_stable_frames: u32,
    pub stability_tolerance: f32,
    pub green_led: u8,
    pub red_led: u8,
    pub wait_for_removal: u8,
    pub removal_timeout_ms: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Fs80hAutoCaptureReport {
    pub final_contact: Fs80hPresenceMetrics,
    pub removal_waited: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Fs80hMatchResult {
    pub score: f32,
    pub is_match: u8,
    pub matched_minutiae: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Fs80hMinutia {
    pub x: u16,
    pub y: u16,
    /// 1 = ending, 2 = bifurcation.
    pub kind: u8,
    pub angle: f32,
    pub quality: f32,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("no error").unwrap());
}

fn set_error(message: impl AsRef<str>) {
    let message = message.as_ref().replace('\0', "?");
    LAST_ERROR.with(|slot| *slot.borrow_mut() = CString::new(message).unwrap());
}

fn with_guard(f: impl FnOnce() -> Result<(), (Fs80hStatus, String)>) -> Fs80hStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => Fs80hStatus::Ok,
        Ok(Err((status, message))) => {
            set_error(message);
            status
        }
        Err(_) => {
            set_error("internal panic was contained at the native ABI boundary");
            Fs80hStatus::Error
        }
    }
}

fn map_error(error: impl std::fmt::Display) -> (Fs80hStatus, String) {
    let message = error.to_string();
    let status = if message.contains("not found") {
        Fs80hStatus::NotFound
    } else if message.contains("timed out") {
        Fs80hStatus::Timeout
    } else {
        Fs80hStatus::Error
    };
    (status, message)
}

unsafe fn required_out<'a, T>(value: *mut T) -> Result<&'a mut T, (Fs80hStatus, String)> {
    if value.is_null() {
        Err((
            Fs80hStatus::InvalidArgument,
            "required output pointer is null".into(),
        ))
    } else {
        Ok(&mut *value)
    }
}

unsafe fn required_ref<'a, T>(value: *const T, name: &str) -> Result<&'a T, (Fs80hStatus, String)> {
    if value.is_null() {
        Err((Fs80hStatus::InvalidArgument, format!("{name} is null")))
    } else {
        Ok(&*value)
    }
}

unsafe fn required_mut<'a, T>(
    value: *mut T,
    name: &str,
) -> Result<&'a mut T, (Fs80hStatus, String)> {
    if value.is_null() {
        Err((Fs80hStatus::InvalidArgument, format!("{name} is null")))
    } else {
        Ok(&mut *value)
    }
}

unsafe fn input_bytes<'a>(data: *const u8, len: usize) -> Result<&'a [u8], (Fs80hStatus, String)> {
    if data.is_null() && len != 0 {
        Err((
            Fs80hStatus::InvalidArgument,
            "data is null with non-zero length".into(),
        ))
    } else if len == 0 {
        Ok(&[])
    } else {
        Ok(std::slice::from_raw_parts(data, len))
    }
}

fn buffer_from_vec(data: Vec<u8>) -> Fs80hBuffer {
    let mut boxed = data.into_boxed_slice();
    let result = Fs80hBuffer {
        data: boxed.as_mut_ptr(),
        len: boxed.len(),
    };
    std::mem::forget(boxed);
    result
}

fn presence(metrics: FingerPresenceMetrics) -> Fs80hPresenceMetrics {
    Fs80hPresenceMetrics {
        contact_ratio: metrics.contact_ratio,
        active_blocks: metrics.contact_blocks as u32,
        total_blocks: metrics.total_blocks as u32,
        is_present: u8::from(metrics.is_present),
    }
}

fn auto_options(
    value: &Fs80hAutoCaptureOptions,
) -> Result<crate::AutoCaptureOptions, (Fs80hStatus, String)> {
    if !value.min_contact_ratio.is_finite()
        || !(0.0..=1.0).contains(&value.min_contact_ratio)
        || !value.stability_tolerance.is_finite()
        || !(0.0..=1.0).contains(&value.stability_tolerance)
        || value.consecutive_stable_frames == 0
        || value.timeout_ms == 0
        || value.removal_timeout_ms == 0
        || value.green_led > 1
        || value.red_led > 1
        || value.wait_for_removal > 1
    {
        return Err((
            Fs80hStatus::InvalidArgument,
            "invalid automatic-capture options".into(),
        ));
    }
    Ok(crate::AutoCaptureOptions {
        timeout: std::time::Duration::from_millis(value.timeout_ms),
        min_contact_ratio: value.min_contact_ratio,
        poll_dose: value.poll_dose,
        capture_dose: value.capture_dose,
        poll_interval: std::time::Duration::from_millis(value.poll_interval_ms.into()),
        consecutive_stable_frames: value.consecutive_stable_frames as usize,
        stability_tolerance: value.stability_tolerance,
        visual_feedback: value.green_led != 0 || value.red_led != 0,
        wait_for_removal: value.wait_for_removal != 0,
        removal_timeout: std::time::Duration::from_millis(value.removal_timeout_ms),
    })
}

/// Returns the ABI revision, independent from the crate semver version.
#[no_mangle]
pub extern "C" fn fs80h_abi_version() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn fs80h_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Pointer is thread-local and valid until the next ABI call on this thread.
#[no_mangle]
pub extern "C" fn fs80h_last_error_message() -> *const c_char {
    LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fs80h_buffer_free(buffer: *mut Fs80hBuffer) {
    if buffer.is_null() {
        return;
    }
    unsafe {
        let buffer = &mut *buffer;
        if !buffer.data.is_null() {
            drop(Box::from_raw(std::slice::from_raw_parts_mut(
                buffer.data,
                buffer.len,
            )));
        }
        *buffer = Fs80hBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
    }
}

#[no_mangle]
pub extern "C" fn fs80h_list_devices(
    devices: *mut Fs80hDeviceInfo,
    capacity: usize,
    written: *mut usize,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let written = required_out(written)?;
        let found = list_devices().map_err(map_error)?;
        *written = found.len();
        if devices.is_null() && capacity == 0 {
            return Ok(());
        }
        if devices.is_null() {
            return Err((Fs80hStatus::InvalidArgument, "devices is null".into()));
        }
        if capacity < found.len() {
            return Err((
                Fs80hStatus::BufferTooSmall,
                "device buffer is too small".into(),
            ));
        }
        let out = std::slice::from_raw_parts_mut(devices, found.len());
        for (target, item) in out.iter_mut().zip(found) {
            *target = Fs80hDeviceInfo {
                vendor_id: item.vendor_id,
                product_id: item.product_id,
                bus_number: item.id.bus_number,
                address: item.id.address,
                model: if item.product_id == 0x0020 {
                    80
                } else if item.product_id == 0x0025 {
                    81
                } else {
                    0
                },
            };
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_device_open_first(out: *mut *mut Fs80hDevice) -> Fs80hStatus {
    with_guard(|| unsafe {
        let out = required_out(out)?;
        *out = Box::into_raw(Box::new(Fs80hDevice {
            inner: FutronicDevice::open_first().map_err(map_error)?,
        }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_device_open(
    bus: u8,
    address: u8,
    out: *mut *mut Fs80hDevice,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let out = required_out(out)?;
        let id = crate::DeviceId::new(bus, address);
        *out = Box::into_raw(Box::new(Fs80hDevice {
            inner: FutronicDevice::open_by_id(&id).map_err(map_error)?,
        }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_device_close(device: *mut Fs80hDevice) {
    if !device.is_null() {
        unsafe {
            drop(Box::from_raw(device));
        }
    }
}

#[no_mangle]
pub extern "C" fn fs80h_device_set_diodes(
    device: *mut Fs80hDevice,
    green: u8,
    red: u8,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        required_mut(device, "device")?
            .inner
            .set_diodes(green != 0, red != 0)
            .map_err(map_error)
    })
}

#[no_mangle]
pub extern "C" fn fs80h_device_capture(
    device: *mut Fs80hDevice,
    dose: u8,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let device = required_mut(device, "device")?;
        let out = required_out(out)?;
        let frame = if dose == 0 {
            device.inner.capture_frame()
        } else {
            device.inner.capture_frame_with_dose(dose)
        }
        .map_err(map_error)?;
        *out = Box::into_raw(Box::new(Fs80hFrame { inner: frame }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_device_auto_capture(
    device: *mut Fs80hDevice,
    options: *const Fs80hAutoCaptureOptions,
    out: *mut *mut Fs80hFrame,
    report: *mut Fs80hAutoCaptureReport,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let device = required_mut(device, "device")?;
        let out = required_out(out)?;
        let options = auto_options(required_ref(options, "options")?)?;
        let frame = device
            .inner
            .wait_for_finger_present(&options, |_| true)
            .map_err(map_error)?;
        if !report.is_null() {
            *report = Fs80hAutoCaptureReport {
                final_contact: presence(
                    frame.presence_metrics_with_threshold(options.min_contact_ratio),
                ),
                removal_waited: u8::from(options.wait_for_removal),
            };
        }
        *out = Box::into_raw(Box::new(Fs80hFrame { inner: frame }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_free(frame: *mut Fs80hFrame) {
    if !frame.is_null() {
        unsafe {
            drop(Box::from_raw(frame));
        }
    }
}

#[no_mangle]
pub extern "C" fn fs80h_frame_dimensions(
    frame: *const Fs80hFrame,
    width: *mut u32,
    height: *mut u32,
    dpi: *mut u16,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let frame = required_ref(frame, "frame")?;
        *required_out(width)? = frame.inner.width();
        *required_out(height)? = frame.inner.height();
        *required_out(dpi)? = frame.inner.dpi();
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_copy_pixels(
    frame: *const Fs80hFrame,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? =
            buffer_from_vec(required_ref(frame, "frame")?.inner.as_bytes().to_vec());
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_inverted(
    frame: *const Fs80hFrame,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: required_ref(frame, "frame")?.inner.inverted(),
        }));
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_frame_contrast_percentiles(
    frame: *const Fs80hFrame,
    low: u8,
    high: u8,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        if low >= high || high > 100 {
            return Err((
                Fs80hStatus::InvalidArgument,
                "percentiles must be ordered in 0..=100".into(),
            ));
        }
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: required_ref(frame, "frame")?
                .inner
                .contrast_stretched(low, high),
        }));
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_frame_background_normalized(
    frame: *const Fs80hFrame,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: required_ref(frame, "frame")?.inner.background_normalized(),
        }));
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_frame_ridge_enhanced(
    frame: *const Fs80hFrame,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: required_ref(frame, "frame")?.inner.ridge_enhanced(),
        }));
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_frame_encode_image(
    frame: *const Fs80hFrame,
    format: u32,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let format = match format {
            1 => crate::FingerprintImageFormat::Png,
            2 => crate::FingerprintImageFormat::Tiff,
            3 => crate::FingerprintImageFormat::Pgm,
            _ => return Err((Fs80hStatus::InvalidArgument, "unknown image format".into())),
        };
        *required_out(out)? = buffer_from_vec(
            required_ref(frame, "frame")?
                .inner
                .encode_image(format)
                .map_err(map_error)?,
        );
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_device_read_descriptor(
    device: *mut Fs80hDevice,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = buffer_from_vec(
            required_mut(device, "device")?
                .inner
                .read_descriptor_bytes()
                .map_err(map_error)?
                .to_vec(),
        );
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn fs80h_device_read_control_bytes(
    device: *mut Fs80hDevice,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = buffer_from_vec(
            required_mut(device, "device")?
                .inner
                .read_control_bytes()
                .map_err(map_error)?
                .to_vec(),
        );
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_from_pixels(
    data: *const u8,
    len: usize,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: RawFrame::new(input_bytes(data, len)?.to_vec()).map_err(map_error)?,
        }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_presence(
    frame: *const Fs80hFrame,
    out: *mut Fs80hPresenceMetrics,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = presence(required_ref(frame, "frame")?.inner.presence_metrics());
        Ok(())
    })
}

/// Set `minutiae` to null with capacity zero to obtain the required element count.
#[no_mangle]
pub extern "C" fn fs80h_frame_extract_minutiae(
    frame: *const Fs80hFrame,
    minutiae: *mut Fs80hMinutia,
    capacity: usize,
    written: *mut usize,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let extracted =
            crate::extract_minutiae(&required_ref(frame, "frame")?.inner).map_err(map_error)?;
        *required_out(written)? = extracted.len();
        if minutiae.is_null() && capacity == 0 {
            return Ok(());
        }
        if minutiae.is_null() {
            return Err((Fs80hStatus::InvalidArgument, "minutiae is null".into()));
        }
        if capacity < extracted.len() {
            return Err((
                Fs80hStatus::BufferTooSmall,
                "minutiae buffer is too small".into(),
            ));
        }
        for (out, item) in std::slice::from_raw_parts_mut(minutiae, extracted.len())
            .iter_mut()
            .zip(extracted)
        {
            *out = Fs80hMinutia {
                x: item.x as u16,
                y: item.y as u16,
                kind: match item.kind {
                    crate::MinutiaType::Ending => 1,
                    crate::MinutiaType::Bifurcation => 2,
                },
                angle: item.theta,
                quality: item.quality,
            };
        }
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_to_wsq(
    frame: *const Fs80hFrame,
    bitrate: f32,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = buffer_from_vec(
            required_ref(frame, "frame")?
                .inner
                .to_wsq(Some(bitrate))
                .map_err(map_error)?,
        );
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_from_wsq(
    data: *const u8,
    len: usize,
    out: *mut *mut Fs80hFrame,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hFrame {
            inner: RawFrame::from_wsq(input_bytes(data, len)?).map_err(map_error)?,
        }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_extract_iso(
    frame: *const Fs80hFrame,
    format: u32,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let format = match format {
            2005 => crate::IsoFormat::Iso2005,
            2011 => crate::IsoFormat::Iso2011,
            _ => {
                return Err((
                    Fs80hStatus::InvalidArgument,
                    "ISO format must be 2005 or 2011".into(),
                ))
            }
        };
        *required_out(out)? = buffer_from_vec(
            required_ref(frame, "frame")?
                .inner
                .extract_iso_template_as(format, &TemplateOptions::default())
                .map_err(map_error)?
                .into_bytes(),
        );
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_frame_extract_ansi(
    frame: *const Fs80hFrame,
    format: u32,
    out: *mut Fs80hBuffer,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let format = match format {
            2004 => crate::AnsiFormat::Ansi3782004,
            2009 => crate::AnsiFormat::Ansi3782009,
            2010 => crate::AnsiFormat::Ansi3782009Am1,
            _ => {
                return Err((
                    Fs80hStatus::InvalidArgument,
                    "ANSI format must be 2004, 2009 or 2010".into(),
                ))
            }
        };
        *required_out(out)? = buffer_from_vec(
            required_ref(frame, "frame")?
                .inner
                .extract_ansi_template_as(format, &TemplateOptions::default())
                .map_err(map_error)?
                .into_bytes(),
        );
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_matcher_create(threshold: f32, out: *mut *mut Fs80hMatcher) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(out)? = Box::into_raw(Box::new(Fs80hMatcher {
            inner: Matcher::new(MatcherOptions { threshold }).map_err(map_error)?,
        }));
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn fs80h_matcher_free(matcher: *mut Fs80hMatcher) {
    if !matcher.is_null() {
        unsafe {
            drop(Box::from_raw(matcher));
        }
    }
}

fn write_match(
    result: crate::MatchResult,
    out: *mut Fs80hMatchResult,
) -> Result<(), (Fs80hStatus, String)> {
    unsafe {
        *required_out(out)? = Fs80hMatchResult {
            score: result.score,
            is_match: u8::from(result.is_match),
            matched_minutiae: result.matched_minutiae,
        };
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn fs80h_matcher_verify_iso(
    matcher: *const Fs80hMatcher,
    enrolled: *const u8,
    enrolled_len: usize,
    probe: *const u8,
    probe_len: usize,
    out: *mut Fs80hMatchResult,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let matcher = required_ref(matcher, "matcher")?;
        let enrolled = crate::IsoTemplate::from_bytes(input_bytes(enrolled, enrolled_len)?)
            .map_err(map_error)?;
        let probe =
            crate::IsoTemplate::from_bytes(input_bytes(probe, probe_len)?).map_err(map_error)?;
        write_match(
            matcher.inner.verify(&enrolled, &probe).map_err(map_error)?,
            out,
        )
    })
}

#[no_mangle]
pub extern "C" fn fs80h_matcher_verify_ansi(
    matcher: *const Fs80hMatcher,
    enrolled: *const u8,
    enrolled_len: usize,
    probe: *const u8,
    probe_len: usize,
    out: *mut Fs80hMatchResult,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let matcher = required_ref(matcher, "matcher")?;
        let enrolled = crate::AnsiTemplate::from_bytes(input_bytes(enrolled, enrolled_len)?)
            .map_err(map_error)?;
        let probe =
            crate::AnsiTemplate::from_bytes(input_bytes(probe, probe_len)?).map_err(map_error)?;
        write_match(
            matcher.inner.verify(&enrolled, &probe).map_err(map_error)?,
            out,
        )
    })
}

#[cfg(feature = "mcc")]
#[no_mangle]
pub extern "C" fn fs80h_mcc_from_frame(
    frame: *const Fs80hFrame,
    out: *mut *mut Fs80hMccTemplate,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        let frame = required_ref(frame, "frame")?;
        let minutiae = crate::extract_minutiae(&frame.inner).map_err(map_error)?;
        *required_out(out)? = Box::into_raw(Box::new(Fs80hMccTemplate {
            inner: crate::mcc::encode(&minutiae, frame.inner.width(), frame.inner.height()),
        }));
        Ok(())
    })
}

#[cfg(feature = "mcc")]
#[no_mangle]
pub extern "C" fn fs80h_mcc_free(template: *mut Fs80hMccTemplate) {
    if !template.is_null() {
        unsafe {
            drop(Box::from_raw(template));
        }
    }
}

#[cfg(feature = "mcc")]
#[no_mangle]
pub extern "C" fn fs80h_mcc_match(
    probe: *const Fs80hMccTemplate,
    gallery: *const Fs80hMccTemplate,
    score: *mut f32,
) -> Fs80hStatus {
    with_guard(|| unsafe {
        *required_out(score)? = crate::mcc::match_templates(
            &required_ref(probe, "probe")?.inner,
            &required_ref(gallery, "gallery")?.inner,
        );
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_pixel_length_is_reported_without_panicking() {
        let mut frame = ptr::null_mut();
        assert_eq!(
            fs80h_frame_from_pixels(ptr::null(), 1, &mut frame),
            Fs80hStatus::InvalidArgument
        );
        assert!(frame.is_null());
    }

    #[test]
    fn buffer_free_accepts_null_and_resets_owned_data() {
        fs80h_buffer_free(ptr::null_mut());
        let mut buffer = buffer_from_vec(vec![1, 2, 3]);
        fs80h_buffer_free(&mut buffer);
        assert!(buffer.data.is_null());
        assert_eq!(buffer.len, 0);
    }
}
