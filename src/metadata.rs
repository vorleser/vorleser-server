use ffmpeg::*;

use std::mem;
use std::ffi::CString;
use std::ffi::CStr;
use std::ptr;
use std::collections::HashMap;
use std::slice;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct Media {
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

pub struct AVDictionary {
    pub count: usize,
    pub elems: *mut AVDictionaryEntry
}

impl Chapter {
    fn from_av_chapter(av: &AVChapter) -> Chapter {
        let start = apply_timebase(av.start, &av.time_base);
        let end = apply_timebase(av.end, &av.time_base);
        let d = dict_to_map(av.metadata as *mut AVDictionary);
        let title = d.get("title").cloned();
        Chapter {
            start: start.clone(),
            end: end.clone(),
            title: title,
            metadata: d
        }
    }

    fn from_av_chapters(avs: &[&AVChapter]) -> Vec<Chapter> {
        let mut res = Vec::new();
        for av in avs.iter() {
            res.push(Self::from_av_chapter(av))
        }
        res
    }
}


pub struct MediaFile {
    ctx: *mut AVFormatContext,
    averror: i32,
    av_packet: Option<AVPacket>
}

#[derive(Debug)]
pub struct MediaError {
    code: i32,
    description: String
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl MediaError {
    fn new(code: i32) -> MediaError {
        let description = unsafe {
            let mut buf: [i8; 1024] = [0; 1024];
            av_strerror(code, &mut buf[0] as *mut i8, 1024);
            String::from_utf8_unchecked(mem::transmute::<[i8; 1024], [u8; 1024]>(buf).to_vec())
        };
        MediaError {code: code, description: description}
    }
}

impl Error for MediaError {
    fn description(&self) -> &str {
        return &self.description;
    }

    fn cause(&self) -> Option<&Error> {
        return None
    }
}

impl MediaFile {
    pub fn read_file(file_name: &str) -> Result<Self, MediaError>{
        unsafe {
            if !FFMPEG_INITIALIZED {
                av_register_all();
                FFMPEG_INITIALIZED = true;
            }
            let c_file_name = CString::new(file_name).unwrap();
            let mut new = Self {
                ctx: avformat_alloc_context(),
                averror: 0,
                av_packet: None
            };
            new.averror = avformat_open_input(
                &mut new.ctx,
                c_file_name.as_ptr(),
                ptr::null(),
                ptr::null_mut()
                );
            if new.averror != 0 {
                return Err(MediaError::new(new.averror))
            } else {
                avformat_find_stream_info(new.ctx, ptr::null_mut());
            }
            Ok(new)
        }
    }

    pub fn get_cover_art(&mut self) -> &[u8] {
        unsafe {
            self.av_packet = Some(mem::uninitialized());
            av_init_packet(self.av_packet.as_mut().unwrap() as *mut _);
            av_read_frame(self.ctx, self.av_packet.as_mut().unwrap() as *mut _);
            slice::from_raw_parts(self.av_packet.as_ref().unwrap().data, self.av_packet.as_ref().unwrap().size as usize)
        }
    }

    fn get_chapters(&self) -> Vec<Chapter> {
        Chapter::from_av_chapters(self.av_chapter_slice())
    }

    fn av_chapter_slice(&self) -> &[&AVChapter] {
        unsafe {
            slice::from_raw_parts(
                mem::transmute((*self.ctx).chapters),
                (*self.ctx).nb_chapters as usize
                )
        }
    }

    pub fn get_mediainfo(&self) -> Media {
        unsafe {
            Media {
                chapters: self.get_chapters(),
                length: apply_timebase((*self.ctx).duration, &AV_TIME_BASE_Q),
                metadata: dict_to_map((*self.ctx).metadata as *mut AVDictionary)
            }
        }
    }
}

impl Drop for MediaFile {
    fn drop(&mut self) {
        if self.averror == 0 {
            unsafe {
                avformat_close_input(&mut self.ctx);
                if let Some(ref mut pkt) = self.av_packet {
                    av_free_packet(pkt);
                }
                avformat_free_context(self.ctx);
            }
        }
    }
}

fn dict_to_map(dict_pointer: *mut AVDictionary) -> HashMap<String, String> {
    let mut map = HashMap::new();
    unsafe {
        let dict: &AVDictionary = &mut *dict_pointer;
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


fn av_dict_vec(dict: &AVDictionary) -> &[AVDictionaryEntry] {
    unsafe {
        slice::from_raw_parts(dict.elems, dict.count)
    }
}


fn apply_timebase(time: i64, timebase: &AVRational) -> f64 {
    time as f64 * (timebase.num as f64 / timebase.den as f64)
}
