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

#[derive(Debug)]
struct Chapter {
    title: Option<String>,
    start: f64,
    end: f64
}

impl Chapter {
    fn from_av_chapter(av: &AVChapter) -> Chapter {
        let start = apply_timebase(av.start, &av.time_base);
        let end = apply_timebase(av.end, &av.time_base);
        unsafe {
            let d = dict_to_map(mem::transmute(av.metadata));
            let title = d.get("title").cloned();
            Chapter {
                start: start.clone(),
                end: end.clone(),
                title: title
            }
        }
    }

    fn from_av_chapters(mut avs: Vec<&AVChapter>) -> Vec<Chapter> {
        let mut res = Vec::new();
        for av in avs.iter_mut() {
            res.push(Self::from_av_chapter(av))
        }
        res
    }
}

struct Audiobook {
    length: f64,
    chapters: Vec<Chapter>,
    name: String
}

struct Context {
    pub ctx: *mut AVFormatContext
}

enum LULError {
}

impl Context {
    pub fn new() -> Self {
        unsafe {
            Self {ctx: avformat_alloc_context()}
        }
    }

    pub fn read_file(&mut self, file_name: &str) -> Result<(), LULError>{
        unsafe {
            let averror = avformat_open_input(
                &mut self.ctx,
                CString::new(file_name).unwrap().as_ptr(),
                ptr::null(),
                ptr::null_mut()
            );
        }
        Ok(())
    }

    fn get_chapters(&self) -> Vec<Chapter> {
        Chapter::from_av_chapters(self.av_chapter_vec())
    }

    fn av_chapter_vec<'a>(&self) -> Vec<&'a AVChapter> {
        unsafe {
            Vec::from_raw_parts(
                mem::transmute((*self.ctx).chapters),
                (*self.ctx).nb_chapters as usize,
                (*self.ctx).nb_chapters as usize)
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            avformat_free_context(self.ctx);
        }
    }
}

fn get_metadata(file_name: &str) {
    let mut c = Context::new();
    c.read_file(file_name);
    println!("{:?}", c.get_chapters());
    // unsafe {
    // let mut ctx: *mut AVFormatContext = avformat_alloc_context();
    // let averror = avformat_open_input(
    //     &mut ctx,
    //     CString::new(file_name).unwrap().as_ptr(),
    //     ptr::null(), ptr::null_mut()
    // );
    // if averror > 0 {
    //     let mut buf: [i8; 64] = [0; 64];
    //     av_strerror(averror, &mut buf[0] as *mut i8, 64);
    //     let lol = String::from_utf8_unchecked(mem::transmute::<[i8; 64], [u8; 64]>(buf).to_vec());
    //     panic!(lol)
    // }


    // let a = Audiobook {
    //     chapters: Vec::new(),
    //     name: "lul".to_string(),
    //     length: apply_timebase((*ctx).duration, &AV_TIME_BASE_Q)
    // };
    // let ref mut chaps = av_chapter_vec(ctx)[0];
    // let c = Chapter::from_av_chapter(chap);
    // unsafe {
    //     let mut chaps = av_chapter_vec(c.ctx);
    //     let c = Chapter::from_av_chapters(chaps);
    //     println!("{:?}", c);
    // }
}

fn dict_to_map(dict: &AVDictionary) -> HashMap<String, String> {
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


fn av_dict_vec(dict: &AVDictionary) -> Vec<AVDictionaryEntry> {
    unsafe {
        Vec::from_raw_parts(dict.elems, dict.count, dict.count)
    }
}


fn av_chapter_vec<'a>(ctx: *const AVFormatContext) -> Vec<&'a AVChapter> {
    unsafe {
        Vec::from_raw_parts(mem::transmute((*ctx).chapters), (*ctx).nb_chapters as usize, (*ctx).nb_chapters as usize)
    }
}

fn apply_timebase(time: i64, timebase: &AVRational) -> f64 {
    time as f64 * (timebase.num as f64 / timebase.den as f64)
}
