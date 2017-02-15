#![feature(libc)]
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;

use std::mem;
use std::ffi::CString;
use std::ptr;

use ffmpeg::*;
use std::env;

fn main() {
    unsafe {
        let mut x: _ = avformat_alloc_context();
        let averror = avformat_open_input(&mut x, CString::new("/tmp/sample.mp3").unwrap().as_ptr(), ptr::null(), ptr::null_mut());
        let mut buf: [i8; 64] = [0; 64];
        let e = AVERROR_INVALIDDATA;
        av_strerror(averror, &mut buf[0] as *mut i8, 64);
        if averror == e {
            println!("AVERROR_INVALIDDATA")
        }
        // let lol = String::from_utf8_unchecked(mem::transmute::<[i8; 64], [u8; 64]>(buf).to_vec());
        // println!("{}", lol);
    }
}
