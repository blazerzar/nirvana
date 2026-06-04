use std::ffi::c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: u32, event_type: u32) -> f64;
    fn CGSessionCopyCurrentDictionary() -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> *const c_void;
    fn CFBooleanGetValue(boolean: *const c_void) -> bool;
    fn CFRelease(cf: *const c_void);
}

const HID_SYSTEM_STATE: u32 = 1;
const ANY_INPUT_EVENT: u32 = u32::MAX;
const K_CFSTRING_ENCODING_UTF8: u32 = 0x8000100;

pub(super) fn get_idle_seconds() -> u64 {
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT_EVENT) };
    secs as u64
}

pub(super) fn is_locked() -> bool {
    unsafe {
        let session = CGSessionCopyCurrentDictionary();
        if session.is_null() {
            // No session means locked computer
            return true;
        }

        let key_str = std::ffi::CString::new("CGSSessionScreenIsLocked").unwrap();
        let key =
            CFStringCreateWithCString(std::ptr::null(), key_str.as_ptr(), K_CFSTRING_ENCODING_UTF8);

        let value = CFDictionaryGetValue(session, key);
        CFRelease(key);
        CFRelease(session);

        if value.is_null() {
            return false;
        }

        CFBooleanGetValue(value)
    }
}
