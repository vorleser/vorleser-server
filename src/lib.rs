#![allow(dead_code)]
#![feature(pub_restricted)]

#[macro_use] extern crate lazy_static;
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;
extern crate regex;
extern crate walkdir;

pub mod worker;
