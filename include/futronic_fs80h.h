#ifndef FUTRONIC_FS80H_H
#define FUTRONIC_FS80H_H

/* Stable C ABI v1 for futronic-fs80h-rs. */
#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#  if defined(FS80H_BUILDING_DLL)
#    define FS80H_API __declspec(dllexport)
#  else
#    define FS80H_API __declspec(dllimport)
#  endif
#else
#  define FS80H_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define FS80H_ABI_VERSION 1u
/* Defined as 1 only in an artifact built locally with --features nfiq2. */
#ifndef FS80H_HAS_NFIQ2
#  define FS80H_HAS_NFIQ2 0
#endif

typedef enum fs80h_status {
  FS80H_OK = 0, FS80H_INVALID_ARGUMENT = 1, FS80H_BUFFER_TOO_SMALL = 2,
  FS80H_NOT_FOUND = 3, FS80H_TIMEOUT = 4, FS80H_UNSUPPORTED = 5,
  FS80H_ERROR = 255
} fs80h_status;

typedef struct fs80h_device fs80h_device;
typedef struct fs80h_frame fs80h_frame;
typedef struct fs80h_matcher fs80h_matcher;
typedef struct fs80h_mcc_template fs80h_mcc_template;

typedef struct { uint8_t *data; size_t len; } fs80h_buffer;
typedef struct { uint16_t vendor_id, product_id; uint8_t bus_number, address, model; } fs80h_device_info;
typedef struct { float contact_ratio; uint32_t active_blocks, total_blocks; uint8_t is_present; } fs80h_presence_metrics;
typedef struct { float score; uint8_t is_match; size_t matched_minutiae; } fs80h_match_result;
typedef struct { uint16_t x, y; uint8_t kind; float angle, quality; } fs80h_minutia;

FS80H_API uint32_t fs80h_abi_version(void);
FS80H_API const char *fs80h_version(void);
/* Thread-local; valid only until the caller's next library call. */
FS80H_API const char *fs80h_last_error_message(void);
FS80H_API void fs80h_buffer_free(fs80h_buffer *buffer);

/* Set devices=NULL and capacity=0 to obtain the required element count. */
FS80H_API fs80h_status fs80h_list_devices(fs80h_device_info *devices, size_t capacity, size_t *written);
FS80H_API fs80h_status fs80h_device_open_first(fs80h_device **out);
FS80H_API fs80h_status fs80h_device_open(uint8_t bus, uint8_t address, fs80h_device **out);
FS80H_API void fs80h_device_close(fs80h_device *device);
FS80H_API fs80h_status fs80h_device_set_diodes(fs80h_device *device, uint8_t green, uint8_t red);
/* dose=0 selects the calibrated default dose (166). */
FS80H_API fs80h_status fs80h_device_capture(fs80h_device *device, uint8_t dose, fs80h_frame **out);
/* Returns the unresampled 384x800 stream (307,200 bytes). */
FS80H_API fs80h_status fs80h_device_capture_raw_sensor_stream(fs80h_device *device, uint8_t dose, fs80h_buffer *out);
/* Experimental FS80H research APIs; not liveness or anti-spoof controls. */
FS80H_API fs80h_status fs80h_device_probe_lfd_auxiliary(fs80h_device *device, uint8_t selector, uint8_t profile, fs80h_buffer *out);
FS80H_API fs80h_status fs80h_device_lfd_firmware_config(fs80h_device *device, fs80h_buffer *out);
FS80H_API fs80h_status fs80h_device_auto_capture(fs80h_device *device, uint64_t timeout_ms, fs80h_frame **out);

FS80H_API void fs80h_frame_free(fs80h_frame *frame);
FS80H_API fs80h_status fs80h_frame_dimensions(const fs80h_frame *frame, uint32_t *width, uint32_t *height, uint16_t *dpi);
FS80H_API fs80h_status fs80h_frame_copy_pixels(const fs80h_frame *frame, fs80h_buffer *out);
FS80H_API fs80h_status fs80h_frame_from_pixels(const uint8_t *data, size_t len, fs80h_frame **out);
FS80H_API fs80h_status fs80h_frame_presence(const fs80h_frame *frame, fs80h_presence_metrics *out);
/* Set minutiae=NULL and capacity=0 to obtain the required element count. */
FS80H_API fs80h_status fs80h_frame_extract_minutiae(const fs80h_frame *frame, fs80h_minutia *minutiae, size_t capacity, size_t *written);
FS80H_API fs80h_status fs80h_frame_to_wsq(const fs80h_frame *frame, float bitrate, fs80h_buffer *out);
FS80H_API fs80h_status fs80h_frame_from_wsq(const uint8_t *data, size_t len, fs80h_frame **out);
/* format is 2005 or 2011. */
FS80H_API fs80h_status fs80h_frame_extract_iso(const fs80h_frame *frame, uint32_t format, fs80h_buffer *out);
/* format is 2004, 2009 or 2010 (2009/AM1). */
FS80H_API fs80h_status fs80h_frame_extract_ansi(const fs80h_frame *frame, uint32_t format, fs80h_buffer *out);

FS80H_API fs80h_status fs80h_matcher_create(float threshold, fs80h_matcher **out);
FS80H_API void fs80h_matcher_free(fs80h_matcher *matcher);
FS80H_API fs80h_status fs80h_matcher_verify_iso(const fs80h_matcher *matcher, const uint8_t *enrolled, size_t enrolled_len, const uint8_t *probe, size_t probe_len, fs80h_match_result *out);
FS80H_API fs80h_status fs80h_matcher_verify_ansi(const fs80h_matcher *matcher, const uint8_t *enrolled, size_t enrolled_len, const uint8_t *probe, size_t probe_len, fs80h_match_result *out);

FS80H_API fs80h_status fs80h_mcc_from_frame(const fs80h_frame *frame, fs80h_mcc_template **out);
FS80H_API void fs80h_mcc_free(fs80h_mcc_template *template_);
FS80H_API fs80h_status fs80h_mcc_match(const fs80h_mcc_template *probe, const fs80h_mcc_template *gallery, float *score);

#ifdef __cplusplus
}
#endif
#endif
