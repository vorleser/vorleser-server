use ffmpeg::*;

use std::mem;
use std::ffi::CString;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use std::collections::HashMap;
use std::slice;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::iter::FromIterator;

lazy_static! {
    static ref FFMPEG_INITIALIZED: Mutex<bool> = Mutex::new(false);
}

#[derive(Debug)]
pub struct Media {
    length: f64,
    chapters: Vec<Chapter>,
    metadata: HashMap<String, String>
}

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
            let mut buf: [c_char; 1024] = [0; 1024];
            av_strerror(code, &mut buf[0], 1024);
            CStr::from_ptr(&buf[0]).to_string_lossy().into_owned()
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

fn check_av_result(num: i32) -> Result<i32, MediaError> {
    if num < 0 {
        Err(MediaError::new(num))
    }
    else {
        Ok(num)
    }
}

fn ensure_av_register_all() {
    unsafe {
        let mut initialized_guard = FFMPEG_INITIALIZED.lock().unwrap();
        if !*initialized_guard {
            av_register_all();
            *initialized_guard = true;
        }
    }
}

fn ptr_to_opt_mut<T>(ptr: *mut T) -> Option<*mut T> {
    if ptr == ptr::null_mut() {
        None
    } else {
        Some(ptr)
    }
}

fn ptr_to_opt<T>(ptr: *const T) -> Option<*const T> {
    if ptr == ptr::null() {
        None
    } else {
        Some(ptr)
    }
}

impl MediaFile {
    pub fn read_file(file_name: &Path) -> Result<Self, MediaError> {
        let file_name_str = match file_name.to_str() {
            Some(s) => s,
            None => return Err(MediaError{
                code: 0,
                description: "Non UTF8 Path provided".to_string()
            })
        };
        unsafe {
            ensure_av_register_all();
            let c_file_name = CString::new(file_name_str).unwrap();
            let mut new = Self {
                ctx: avformat_alloc_context(),
                averror: 0,
                av_packet: None
            };
            new.averror = try!(check_av_result(avformat_open_input(
                &mut new.ctx,
                c_file_name.as_ptr(),
                ptr::null(),
                ptr::null_mut()
            )));
            try!(check_av_result(avformat_find_stream_info(new.ctx, ptr::null_mut())));
            Ok(new)
        }
    }

    pub fn read_packet(&self) -> Result<Option<AVPacket>, MediaError> {
        unsafe {
            let mut pkt = mem::uninitialized::<AVPacket>();
            match check_av_result(av_read_frame(self.ctx, &mut pkt)) {
                Err(MediaError{code: AVERROR_EOF, .. } ) => Ok(None),
                Err(e) => Err(e),
                _ => Ok(Some(pkt))
            }
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

    pub fn get_codec(&self) -> &AVCodec {
        unsafe {
            unimplemented!()
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

impl MediaFile {
    pub fn get_streams(&self) -> &[&AVStream] {
        unsafe {
            slice::from_raw_parts(mem::transmute((*self.ctx).streams), (*self.ctx).nb_streams as usize)
        }
    }

    pub fn get_best_stream(&self, media_type: AVMediaType) -> Result<&AVStream, MediaError> {
        unsafe {
            // for s in self.get_streams() {
            //     if (*(*s).codecpar).codec_type == AVMEDIA_TYPE_AUDIO {
            //         println!("{:?}", (*s).index);
            //         return Some(s)
            //     }
            // }
            let stream_index = try!(check_av_result(av_find_best_stream(self.ctx, media_type, -1, -1, ptr::null_mut(), 0)));
            Ok(self.get_streams()[stream_index as usize])
        }
    }
}

pub struct NewMediaFile {
    ctx: *mut AVFormatContext
}

impl NewMediaFile {
    pub fn from_stream(file_name: &Path, stream: &AVStream) -> Result<Self, MediaError> {
        unsafe {
            let time_base = (*stream).time_base;
            Self::new(
                file_name,
                &mut *(*stream).codecpar,
                time_base
            )
        }
    }

    pub fn new(file_name: &Path, codec: &mut AVCodecParameters, time_base: AVRational) -> Result<Self, MediaError> {
        ensure_av_register_all();
        let c_file_name = CString::new(file_name.to_str().unwrap()).unwrap();
        unsafe {
            let format = match ptr_to_opt_mut(av_guess_format(ptr::null(), c_file_name.as_ptr(), ptr::null())) {
                Some(f) => f,
                None => return Err(MediaError{
                    description: "Not format could be guessed!".to_string(),
                    code: 1337
                })
            };
            let mut ctx = ptr::null_mut();
            try!(check_av_result(avformat_alloc_output_context2(&mut ctx, ptr::null(), ptr::null(), c_file_name.as_ptr())));
            // (*ctx).oformat = format;
            let mut io_ctx = ptr::null_mut();
            try!(check_av_result(avio_open2(&mut io_ctx, c_file_name.as_ptr(), AVIO_FLAG_WRITE, ptr::null(), ptr::null_mut())));
            (*ctx).pb = io_ctx;
            let stream = ptr_to_opt_mut(avformat_new_stream(ctx, ptr::null())).unwrap();
            (*stream).time_base = time_base;
            avcodec_parameters_copy((*stream).codecpar, codec);
            Ok(Self{ ctx: ctx })
        }
        // avformat_new_stream(ctx, );
    }

    pub fn write_header(&mut self) -> Result<(), MediaError> {
        unsafe {
            try!(check_av_result(avformat_write_header(self.ctx, ptr::null_mut())));
        }
        Ok(())
    }

    fn write_frame(&mut self, pkt: &mut AVPacket) -> Result<(), MediaError> {
        unsafe {
            pkt.stream_index = 0;
            try!(check_av_result(av_write_frame(self.ctx, pkt)));
        }
        Ok(())
    }

    fn write_trailer(&mut self) -> Result<(), MediaError> {
        unsafe {
            try!(check_av_result(av_write_trailer(self.ctx)));
        }
        Ok(())
    }
}

// struct Stream {
//     data: *mut AVStream
// }

// impl Stream {
//     fn new(&AVStream) -> Self {
//         Stream {data: AVStream}
//     }
// }

pub fn merge_files(path: &Path, in_files: Vec<MediaFile>) -> Result<NewMediaFile, MediaError> {
    // todo: check in_files length
    let mut out = {
        let stream = try!(in_files.first().unwrap().get_best_stream(AVMEDIA_TYPE_AUDIO));
        try!(NewMediaFile::from_stream(path, stream))
    };
    println!("writing header");
    try!(out.write_header());
    println!("wrote header");

    let mut previous_files_duration: i64 = 0;
    for f in in_files {
        println!("next file");

        let best = try!(f.get_best_stream(AVMEDIA_TYPE_AUDIO));

        let mut this_file_duration: i64 = 0;
        println!("previous_files_duration: {}", previous_files_duration);
        loop {
            let mut last_pts = 0;
            let mut last_dts = 0;
            match try!(f.read_packet()) {
                Some(mut pkt) => {
                    if pkt.stream_index != best.index {
                        continue;
                    }
                    // Todo: I am not sure if this is the proper way to do this
                    // maybe we need to keep a running value instead of letting ffmpeg guess
                    //println!("kek: pkt: {}, file: {} :kek", pkt.duration, this_file_duration);
                    this_file_duration = this_file_duration + pkt.duration;
                    pkt.dts += previous_files_duration;
                    pkt.pts += previous_files_duration;

                    if pkt.pts < 0 || pkt.dts < 0 {
                        println!("foo");
                    }
                    try!(out.write_frame(&mut pkt))
                },
                None => break
            }
        }
        previous_files_duration = previous_files_duration + this_file_duration;
        this_file_duration = 0;
    }
    println!("writing trailer");
    try!(out.write_trailer());
    Ok(out)
    // Self::new()
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
