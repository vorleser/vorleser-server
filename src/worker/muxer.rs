use ffmpeg::*;

use std::ffi::CString;
use std::ptr;
use std::path::Path;
use super::mediafile::MediaFile;
use super::util::*;
use worker::error::*;

pub struct NewMediaFile {
    ctx: *mut AVFormatContext
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
        let c_file_name = CString::new(file_name.to_str().unwrap()).unwrap();
        unsafe {
            let format = match ptr_to_opt_mut(av_guess_format(ptr::null(), c_file_name.as_ptr(), ptr::null())) {
                Some(f) => f,
                None => return Err(ErrorKind::Other("No Format could be guessed").into())
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

    pub fn write_header(&mut self) -> Result<()> {
        unsafe {
            try!(check_av_result(avformat_write_header(self.ctx, ptr::null_mut())));
        }
        Ok(())
    }

    fn write_frame(&mut self, pkt: &mut AVPacket) -> Result<()> {
        unsafe {
            pkt.stream_index = 0;
            try!(check_av_result(av_write_frame(self.ctx, pkt)));
        }
        Ok(())
    }

    fn write_trailer(&mut self) -> Result<()> {
        unsafe {
            try!(check_av_result(av_write_trailer(self.ctx)));
        }
        Ok(())
    }
}

pub fn merge_files(path: &AsRef<Path>, in_files: &[MediaFile]) -> Result<NewMediaFile> {
    // TODO: check in_files length
    let mut out = {
        let stream = try!(in_files.first().unwrap().get_best_stream(AVMEDIA_TYPE_AUDIO));
        try!(NewMediaFile::from_stream(path.as_ref(), stream))
    };
    info!("writing header");
    try!(out.write_header());
    info!("wrote header");

    let mut previous_files_duration: i64 = 0;
    for f in in_files {
        trace!("next file");

        let best = try!(f.get_best_stream(AVMEDIA_TYPE_AUDIO));

        let mut this_file_duration: i64 = 0;
        trace!("previous_files_duration: {}", previous_files_duration);
        loop {
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
    }
    info!("writing trailer");
    try!(out.write_trailer());
    Ok(out)
    // Self::new()
}



