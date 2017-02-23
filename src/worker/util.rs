use std::slice;
use std::ffi::CStr;
use std::collections::HashMap;
use ffmpeg::{AVDictionaryEntry, AVDictionary, AVRational};

#[no_mangle]
pub struct Dictionary {
    pub count: usize,
    pub elems: *mut AVDictionaryEntry
}

pub(super) fn dict_to_map(dict_pointer: *mut Dictionary) -> HashMap<String, String> {
    let mut map = HashMap::new();
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


pub(super) fn apply_timebase(time: i64, timebase: &AVRational) -> f64 {
    time as f64 * (timebase.num as f64 / timebase.den as f64)
}

