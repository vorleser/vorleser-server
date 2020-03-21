use crate::ffmpeg::*;
use crate::ffmpeg::AVMediaType::AVMEDIA_TYPE_AUDIO;

use std::ffi::{CString, CStr, OsStr};
use std::mem;
use std::ptr;
use std::path::{Path, PathBuf};
use super::mediafile::MediaFile;
use super::util::*;
use crate::helpers::mllt;
use crate::worker::error::*;
use std::error::Error;

pub struct NewMediaFile {
    ctx: *mut AVFormatContext,
    is_mp3: bool,
    path: PathBuf
}

impl NewMediaFile {
    pub fn from_stream(file_name: &Path, stream: &AVStream) -> Result<Self> {
        unsafe {
            let time_base = (*stream).time_base;
            Self::new(
                file_name,
                &mut *(*stream).codecpar,
                time_base
            )
        }
    }

    pub fn new(file_name: &Path, codec: &mut AVCodecParameters, time_base: AVRational) -> Result<Self> {
        ensure_av_register_all();
        let c_file_name = CString::new(
                match file_name.to_str() {
                    Some(s) => s,
                    None => return Err(WorkerError::InvalidUtf8.into())
                }).unwrap();
        let ipod_short_name = CString::new("ipod").unwrap();
        unsafe {
            let extension = file_name.extension().and_then(OsStr::to_str);
            debug!("finding format for {:?}", extension);
            let short_name = match extension {
                Some("m4b") => {
                    debug!("short_name ipod");
                    ipod_short_name.as_ptr()
                },
                _ => ptr::null()
            };
            debug!("decided on short_name {:?}", short_name);
            let format = match ptr_to_opt_mut(av_guess_format(short_name, c_file_name.as_ptr(), ptr::null())) {
                Some(f) => f,
                None => return Err(WorkerError::UnkownFormat.into())
            };
            let mut ctx = ptr::null_mut();
            check_av_result(avformat_alloc_output_context2(&mut ctx, format, ptr::null(), c_file_name.as_ptr()))?;

            let mut is_mp3 = false;
            match CStr::from_ptr((*format).name).to_str().expect("ffmpeg format name not valid utf8 (╯°□°）╯︵ ┻━┻") {
                "ipod" => {
                    let faststart = CString::new("movflags").unwrap();
                    let opt_true = CString::new("faststart").unwrap();
                    check_av_result(av_opt_set((*ctx).priv_data, faststart.as_ptr(), opt_true.as_ptr(), AV_OPT_SEARCH_CHILDREN))?;
                }
                "mp3" => {
                    // let write_xing = CString::new("write_xing").unwrap();
                    // let f = CString::new("0").unwrap();
                    // try!(check_av_result(av_opt_set((*ctx).priv_data, write_xing.as_ptr(), f.as_ptr(), AV_OPT_SEARCH_CHILDREN)));
                    is_mp3 = true;
                }
                _ => {}
            }

            let mut io_ctx = ptr::null_mut();
            check_av_result(avio_open2(&mut io_ctx, c_file_name.as_ptr(), AVIO_FLAG_WRITE, ptr::null(), ptr::null_mut()))?;
            (*ctx).pb = io_ctx;
            let stream = ptr_to_opt_mut(avformat_new_stream(ctx, ptr::null())).unwrap();
            (*stream).time_base = time_base;
            avcodec_parameters_copy((*stream).codecpar, codec);
            Ok(Self{ ctx, is_mp3, path: file_name.to_owned() })
        }
    }

    pub fn write_header(&mut self) -> Result<()> {
        unsafe {
            check_av_result(avformat_write_header(self.ctx, ptr::null_mut()))?;
        }
        Ok(())
    }

    fn write_frame(&mut self, pkt: &mut AVPacket) -> Result<()> {
        unsafe {
            pkt.stream_index = 0;
            check_av_result(av_write_frame(self.ctx, pkt))?;
        }
        Ok(())
    }

    fn write_trailer(&mut self) -> Result<()> {
        unsafe {
            check_av_result(av_write_trailer(self.ctx))?;
        }
        if self.is_mp3 {
            mllt::mlltify(&self.path)?;
        }
        Ok(())
    }
}

pub fn merge_files(path: &dyn AsRef<Path>, in_files: &[MediaFile]) -> Result<NewMediaFile> {
    // TODO: check in_files length
    // TODO: check that formats are actually compatible
    if in_files.is_empty() {
        return Err(WorkerError::Other{
            description: "No Mediafiles".to_owned()
        }.into());
    }
    let mut out = {
        let stream = in_files.first().unwrap().get_best_stream(AVMEDIA_TYPE_AUDIO)?;
        NewMediaFile::from_stream(path.as_ref(), stream)?
    };
    info!("writing header");
    out.write_header()?;
    info!("wrote header");

    let mut previous_files_duration: i64 = 0;
    for f in in_files {
        trace!("next file");

        let best = f.get_best_stream(AVMEDIA_TYPE_AUDIO)?;

        let mut this_file_duration: i64 = 0;
        trace!("previous_files_duration: {}", previous_files_duration);
        loop {
            match f.read_packet()? {
                Some(mut pkt) => {
                    if pkt.stream_index != best.index {
                        continue;
                    }
                    // Todo: I am not sure if this is the proper way to do this
                    // maybe we need to keep a running value instead of letting ffmpeg guess
                    //println!("kek: pkt: {}, file: {} :kek", pkt.duration, this_file_duration);
                    this_file_duration += pkt.duration;
                    pkt.dts += previous_files_duration;
                    pkt.pts += previous_files_duration;

                    if pkt.pts < 0 || pkt.dts < 0 {
                        println!("foo");
                    }
                    out.write_frame(&mut pkt)?;
                    unsafe {
                        av_free_packet(&mut pkt);
                    }
                    ()
                },
                None => break
            }
        }
        previous_files_duration += this_file_duration;
    }
    info!("writing trailer");
    out.write_trailer()?;
    Ok(out)
    // Self::new()
}
