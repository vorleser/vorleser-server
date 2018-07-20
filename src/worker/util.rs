use std::ffi::{CStr, CString};
use std::slice;
use std::collections::HashMap;
use std::sync::Mutex;
use std::os::raw::c_char;
use ffmpeg::{AVDictionaryEntry, AVRational, av_register_all, av_log_set_level, AV_LOG_QUIET};
use worker::error::*;
use std::fs::File;
use std::io::Read;
use std::path::Path;

lazy_static! {
    static ref FFMPEG_INITIALIZED: Mutex<bool> = Mutex::new(false);
}


#[no_mangle]
pub struct Dictionary {
    pub count: usize,
    pub elems: *mut AVDictionaryEntry
}

pub(super) fn string_from_ptr(ptr: *const c_char) -> Result<Option<String>> {
    if ptr.is_null() {
        Ok(None)
    } else {
        unsafe {
            Ok(Some(CStr::from_ptr(ptr).to_str()?.to_owned()))
        }
    }
}

pub(super) fn dict_to_map(dict_pointer: *mut Dictionary) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if dict_pointer.is_null() {
        return map
    }
    unsafe {
        let dict: &Dictionary = &mut *dict_pointer;
        let v = av_dict_vec(dict);
        for i in v.iter() {
            let key = CStr::from_ptr((*i).key).to_str().unwrap().to_owned();
            let value = CStr::from_ptr((*i).value).to_str().unwrap().to_owned();
            map.insert(
                key,
                value
                );
        }
        map
    }
}

pub(super) fn av_dict_vec(dict: &Dictionary) -> &[AVDictionaryEntry] {
    unsafe {
        slice::from_raw_parts(dict.elems, dict.count)
    }
}

pub(super) fn apply_timebase(time: i64, timebase: AVRational) -> f64 {
    time as f64 * (f64::from(timebase.num) / f64::from(timebase.den))
}

pub(super) fn check_av_result(num: i32) -> Result<i32> {
    if num < 0 {
        Err(new_media_error(num).into())
    }
    else {
        Ok(num)
    }
}

pub(super) fn ensure_av_register_all() {
    unsafe {
        let mut initialized_guard = FFMPEG_INITIALIZED.lock().unwrap();
        if !*initialized_guard {
            av_register_all();
            *initialized_guard = true;
        }
    }
}

pub(super) fn ptr_to_opt_mut<T>(ptr: *mut T) -> Option<*mut T> {
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

pub(super) fn ptr_to_opt<T>(ptr: *const T) -> Option<*const T> {
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

pub fn shut_up_ffmpeg() {
    unsafe {
        av_log_set_level(AV_LOG_QUIET);
    }
}
