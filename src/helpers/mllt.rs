use std::path::Path;
use std::time::Duration;
use std::mem::transmute;

use std::cmp::{min, max};

use failure::Error;
use id3::{Tag, Frame, Version};
use id3::frame::Content;
use id3;

use mp3_metadata;

#[derive(Debug, Fail)]
enum MlltError {
    #[fail(display = "Could not calculate mp3 frame duration")]
    IncalculableDuration
}

trait U8VecExt {
    fn push_i16(&mut self, int: i16);
    fn push_u16(&mut self, int: u16);
    fn push_u24(&mut self, int: u32);
    fn push_u32(&mut self, int: u32);
}

impl U8VecExt for Vec<u8> {
    fn push_i16(&mut self, int: i16) {
        let bytes: [u8; 2] = unsafe { transmute(int.to_be()) };
        self.extend_from_slice(&bytes);
    }

    fn push_u16(&mut self, int: u16) {
        let bytes: [u8; 2] = unsafe { transmute(int.to_be()) };
        self.extend_from_slice(&bytes);
    }

    fn push_u24(&mut self, int: u32) {
        let bytes: [u8; 4] = unsafe { transmute(int.to_be()) };
        assert!(bytes[0] == 0);
        self.extend_from_slice(&bytes[1..]);
    }

    fn push_u32(&mut self, int: u32) {
        let bytes: [u8; 4] = unsafe { transmute(int.to_be()) };
        self.extend_from_slice(&bytes);

    }
}

macro_rules! dump{
    ($($a:expr),*) => {
        debug!(concat!($(stringify!($a), " = {:?}, "),*), $($a),*);
    }
}

// millis
const DESIRED_ACCURACY: u64 = 1000;

fn build_mllt<P: AsRef<Path>>(file: P)-> Result<Vec<u8>, Error> {
    let meta = mp3_metadata::read_from_file(&file)?;

    let mut num_frames: u64 = 0;
    let foo: u32 = 0;
    let mut duration: Duration = Duration::from_secs(0);
    let mut size: u64 = 0;
    let mut smallest_frame = u32::max_value();
    let mut biggest_frame = 0;

    dump!(meta.frames.len());

    for frame in &meta.frames {
        num_frames += 1;
        duration += frame.duration.ok_or::<MlltError>(
            MlltError::IncalculableDuration.into())?;
        size += frame.size as u64;
        smallest_frame = min(smallest_frame, frame.size);
        biggest_frame = max(biggest_frame, frame.size);
    }

    dump!(num_frames, duration, size, smallest_frame, biggest_frame);

    // FFmpeg needs u64s and we just assume audiobooks to be smaller than this
    // This still allows for audiobooks that are billions of days in length
    let millis = duration.as_millis() as u64;

    let avg_frame_millis = millis / num_frames;
    let frames_per_ref = (DESIRED_ACCURACY / avg_frame_millis) as u16;
    dump!(millis, avg_frame_millis, frames_per_ref);
    let mut num_refs = num_frames / frames_per_ref as u64;
    if num_frames % frames_per_ref as u64 != 0 { num_refs += 1 }

    let avg_bytes_per_ref = (size / num_refs) as u32;
    let min_bytes_per_ref = (smallest_frame * frames_per_ref as u32);
    // this rounds down, so when building the refs we'll need to keep track
    // of time and add a millisecond every now and then
    let millis_per_ref = (millis / num_refs) as u32;

    dump!(num_refs, min_bytes_per_ref, millis_per_ref);

    let bits_for_size: u8 = 15;
    let bits_for_time: u8 = 1;

    let mut res = [].to_vec();

    // header
    res.push_u16(frames_per_ref);
    res.push_u24(min_bytes_per_ref);
    res.push_u24(millis_per_ref);
    res.push(bits_for_size);
    res.push(bits_for_time);

    // refs
    let mut count = 0;
    let mut running_count: u64 = 0;
    let mut running_bytes: u64 = 0;
    let mut running_duration = Duration::from_secs(0);
    let mut running_estimated_duration = Duration::from_secs(0);

    for chunk in meta.frames.chunks(frames_per_ref as usize) {
        // the ref describes the last frame in the chunk,
        // so if count < frames_per_ref we don't care about this
        // last chunk
        if chunk.len() < frames_per_ref as usize { break; }

        // bytes
        let chunk_bytes = chunk.iter().map(|frame| frame.size as u64).sum::<u64>();
        let bytes_offset = (chunk_bytes as u64 - min_bytes_per_ref as u64) as u16;
        // res.push_u16(bytes_offset << 0);

        // millis
        let chunk_duration = chunk.iter().map(|frame| frame.duration.unwrap()).sum::<Duration>();
        dump!(chunk_duration, millis_per_ref);
        running_estimated_duration += Duration::from_millis(millis_per_ref as u64);
        running_duration += chunk_duration;
        let millis_offset = (running_duration - running_estimated_duration).as_millis() as u64;
        if millis_offset > 0 {
            running_estimated_duration += Duration::from_millis(millis_offset);
        }

        let packed = bytes_offset << 1 | (millis_offset & 1) as u16;
        res.push_u16(packed);

        if (count % 500) == 0 {
            dump!(chunk_bytes, bytes_offset);
            dump!(chunk_duration, running_estimated_duration, running_duration, millis_offset);
        }
        count += 1;
    }

    dump!(running_duration, running_estimated_duration, count);

    return Ok(res);
}


pub fn mlltify<P: AsRef<Path>>(file: P) -> Result<(), Error> {
    let mut tag = Tag::read_from_path(&file)?;

    // don't do unnecessary work if there already is a mllt tag
    if tag.get("MLLT").is_some() { return Ok(()); }
    dump!(tag);

    let mut frame = Frame::with_content("MLLT", Content::Unknown(build_mllt(&file)?));
    frame.set_tag_alter_preservation(false);
    frame.set_file_alter_preservation(false);
    tag.add_frame(frame);

    tag.write_to_path(&file, Version::Id3v23)?;

    return Ok(());
}
