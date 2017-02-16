#![feature(libc)]
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;

use std::mem;
use std::ffi::CString;
use std::ptr;
use std::collections::HashMap;

use ffmpeg::*;
use std::env;

fn main() {
    unsafe { av_register_all(); }
    get_metadata("/tmp/lol.m4a");
}

struct Chapter {
    name: String,
    start: f64
}

struct Audiobook {
    length: f64,
    chapters: Vec<Chapter>,
    name: String
}

fn get_metadata(file_name: &str) -> Audiobook {
    unsafe {
    let mut ctx: _ = avformat_alloc_context();
    let averror = avformat_open_input(
        &mut ctx,
        CString::new(file_name).unwrap().as_ptr(),
        ptr::null(), ptr::null_mut()
    );
    if averror > 0 {
        let mut buf: [i8; 64] = [0; 64];
        av_strerror(averror, &mut buf[0] as *mut i8, 64);
        let lol = String::from_utf8_unchecked(mem::transmute::<[i8; 64], [u8; 64]>(buf).to_vec());
        panic!(lol)
    }

    let a = Audiobook {
        chapters: Vec::new(),
        name: "lul".to_string(),
        length: apply_timebase((*ctx).duration, &AV_TIME_BASE_Q)
    };

    println!("Map: {:?}", dict_to_map(&mut *((*ctx).metadata)));

    avformat_free_context(ctx);
    return a;
    }
}


fn dict_to_map(dict: &mut AVDictionary) -> HashMap<String, String> {
    let v = av_dict_vec(dict);
    let mut map = HashMap::new();
    for i in v.iter() {
        unsafe {
        let key = CString::from_raw((*i).key).into_string().unwrap(); 
        let value = CString::from_raw((*i).value).into_string().unwrap();
        map.insert(
            key,
            value
        );
        }
    }
    map
}


fn av_dict_vec(dict: &mut AVDictionary) -> Vec<AVDictionaryEntry> {
    unsafe {
        Vec::from_raw_parts(dict.elems, dict.count, dict.count)
    }
}

fn chapter_vec(dict: &AVDictionary) {
    unsafe {}
}

fn apply_timebase(time: i64, timebase: &AVRational) -> f64 {
    time as f64 * (timebase.num as f64 / timebase.den as f64)
}
