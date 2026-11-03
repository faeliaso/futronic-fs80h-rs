use crate::error::WsqError;
use core::ffi::{c_char, c_float, c_int, c_uchar, c_void};
use std::ptr;

extern "C" {
    fn wsq_encode_mem(
        odata: *mut *mut c_uchar,
        olen: *mut c_int,
        r_bitrate: c_float,
        idata: *mut c_uchar,
        width: c_int,
        height: c_int,
        depth: c_int,
        ppi: c_int,
        comment_text: *mut c_char,
    ) -> c_int;

    fn wsq_decode_mem(
        odata: *mut *mut c_uchar,
        owidth: *mut c_int,
        oheight: *mut c_int,
        odepth: *mut c_int,
        oppi: *mut c_int,
        lossyflag: *mut c_int,
        idata: *const c_uchar,
        ilen: c_int,
    ) -> c_int;

    fn free(ptr: *mut c_void);
}

/// Compresses raw 8-bit grayscale pixels to FBI WSQ format.
pub fn encode_to_wsq(
    pixels: &[u8],
    width: usize,
    height: usize,
    ppi: usize,
    bitrate: f32,
) -> Result<Vec<u8>, WsqError> {
    if !(0.1..=2.5).contains(&bitrate) {
        return Err(WsqError::InvalidBitrate(format!("{bitrate:.2}")));
    }
    if pixels.is_empty() || pixels.len() != width * height {
        return Err(WsqError::EmptyBuffer);
    }

    let mut out_ptr: *mut c_uchar = ptr::null_mut();
    let mut out_len: c_int = 0;

    let mut input_copy = pixels.to_vec();

    let ret = unsafe {
        wsq_encode_mem(
            &mut out_ptr,
            &mut out_len,
            bitrate as c_float,
            input_copy.as_mut_ptr(),
            width as c_int,
            height as c_int,
            8,
            ppi as c_int,
            ptr::null_mut(),
        )
    };

    if ret != 0 || out_ptr.is_null() || out_len <= 0 {
        if !out_ptr.is_null() {
            unsafe { free(out_ptr.cast()) };
        }
        return Err(WsqError::EncodeFailed(ret));
    }

    let result = unsafe {
        let slice = core::slice::from_raw_parts(out_ptr, out_len as usize);
        let vec = slice.to_vec();
        free(out_ptr.cast());
        vec
    };

    Ok(result)
}

/// Decompresses an FBI WSQ stream back to raw 8-bit grayscale pixels.
pub fn decode_from_wsq(wsq_bytes: &[u8]) -> Result<(Vec<u8>, usize, usize, usize), WsqError> {
    if wsq_bytes.is_empty() {
        return Err(WsqError::EmptyBuffer);
    }

    let mut out_ptr: *mut c_uchar = ptr::null_mut();
    let mut width: c_int = 0;
    let mut height: c_int = 0;
    let mut depth: c_int = 0;
    let mut ppi: c_int = 0;
    let mut lossy: c_int = 0;

    let ret = unsafe {
        wsq_decode_mem(
            &mut out_ptr,
            &mut width,
            &mut height,
            &mut depth,
            &mut ppi,
            &mut lossy,
            wsq_bytes.as_ptr(),
            wsq_bytes.len() as c_int,
        )
    };

    if ret != 0 || out_ptr.is_null() || width <= 0 || height <= 0 {
        if !out_ptr.is_null() {
            unsafe { free(out_ptr.cast()) };
        }
        return Err(WsqError::DecodeFailed(ret));
    }

    if depth != 8 {
        unsafe { free(out_ptr.cast()) };
        return Err(WsqError::UnsupportedDepth(depth));
    }

    let total_pixels = (width as usize) * (height as usize);
    let result = unsafe {
        let slice = core::slice::from_raw_parts(out_ptr, total_pixels);
        let vec = slice.to_vec();
        free(out_ptr.cast());
        vec
    };

    Ok((result, width as usize, height as usize, ppi as usize))
}
