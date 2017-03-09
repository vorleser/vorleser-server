use ffmpeg::*;

use std::mem;
use std::ffi::CString;
use std::ffi::CStr;
use std::ptr;
use std::path::Path;
use std::slice;
use super::error::MediaError;
use super::util::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Media {
    pub length: f64,
    pub chapters: Vec<Chapter>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Chapter {
    pub title: Option<String>,
    pub metadata: HashMap<String, String>,
    pub start: f64,
}

impl Chapter {
    fn from_av_chapter(av: &AVChapter) -> Chapter {
        let start = apply_timebase(av.start, &av.time_base);
        let end = apply_timebase(av.end, &av.time_base);
        let d = dict_to_map(av.metadata as *mut Dictionary);
        let title = d.get("title").cloned();
        Chapter {
            start: start.clone(),
            title: title,
            metadata: d,
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

impl MediaFile {
    pub fn read_file(file_name: &Path) -> Result<Self, MediaError> {
        let file_name_str = match file_name.to_str() {
            Some(s) => s,
            None => return Err(MediaError::from_description(
                0,
                "Non UTF8 Path provided".to_string()
            ))
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

    pub fn get_cover_art(self) -> Result<Option<Vec<u8>>, MediaError> {
        unsafe {
            let best_image = try!(self.get_best_stream(AVMEDIA_TYPE_VIDEO));
            loop {
                match try!(self.read_packet()) {
                    Some(ref pkt) => {
                        if pkt.stream_index == best_image.index {
                            return Ok(Some(slice::from_raw_parts(
                                pkt.data, pkt.size as usize
                                ).to_owned()));
                        } else {
                            continue;
                        }
                    },
                    None => break
                }
            };
            Ok(None)
        }
    }

    pub fn get_chapters(&self) -> Vec<Chapter> {
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
                metadata: dict_to_map((*self.ctx).metadata as *mut Dictionary)
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
