use ffmpeg::*;

use std::mem;
use std::ffi::CString;
use std::ptr;
use std::collections::HashMap;

#[derive(Debug)]
struct Media {
    length: f64,
    chapters: Vec<Chapter>,
    metadata: HashMap<String, String>
}

static mut FFMPEG_INITIALIZED: bool = false;

#[derive(Debug)]
struct Chapter {
    title: Option<String>,
    metadata: HashMap<String, String>,
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
                title: title,
                metadata: d
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


pub struct Context {
    pub ctx: *mut AVFormatContext
}

struct MediaError {
    code: i32
}

impl MediaError {
    fn describe(&self) -> String {
        if self.code > 0 {
            unsafe {
                let mut buf: [i8; 1024] = [0; 1024];
                av_strerror(self.code, &mut buf[0] as *mut i8, 64);
                return String::from_utf8_unchecked(mem::transmute::<[i8; 1024], [u8; 1024]>(buf).to_vec());
            }
        }
        "All seems good! This error should not exist.".to_string()
    }
}

impl Context {
    pub fn new() -> Self {
        unsafe {
            if !FFMPEG_INITIALIZED {
                unsafe { av_register_all(); }
                FFMPEG_INITIALIZED = true;
            }
            Self {ctx: avformat_alloc_context()}
        }
    }

    pub fn read_file(&mut self, file_name: &str) -> Result<(), MediaError>{
        unsafe {
            let averror = avformat_open_input(
                &mut self.ctx,
                CString::new(file_name).unwrap().as_ptr(),
                ptr::null(),
                ptr::null_mut()
            );
            if averror != 0 {
                return Err(MediaError{code: averror})
            }
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

    pub fn get_mediainfo(&self) -> Media {
        unsafe {
            Media {
                chapters: self.get_chapters(),
                length: apply_timebase((*self.ctx).duration, &AV_TIME_BASE_Q),
                metadata: dict_to_map(mem::transmute((*self.ctx).metadata))
            }
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
