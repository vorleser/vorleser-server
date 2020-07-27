extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use std::convert::TryInto;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use gst::prelude::*;
use gst::{GstBinExt, MessageView};
use libogg::{Packet, Page, Stream};

use crate::encoder::EncoderError;

static SINK_NAME: &'static str = "appsink-0";
static ENCODER_NAME: &'static str = "opusenc";

// At some point these should probably become runtime configurable
static FRAME_SIZE: u32 = 20;
static RATE: u32 = 48_000;

enum Offset {
    TemporalOffset(TemporalOffset),
    ByteOffset(u64),
}

#[derive(Debug)]
struct TemporalOffset {
    millis: u32,
    packet: u32,
    extra_bytes: u32,
}

struct OpusSpec {
    page_header_size: u32,
    page_body_size: u32,
    packet_size: u32,
    packet_length_ms: u32,
    rate: u32,
}

impl Default for OpusSpec {
    fn default() -> Self {
        OpusSpec {
            page_header_size: 53,
            page_body_size: 4160,
            packet_size: 160,
            packet_length_ms: FRAME_SIZE,
            rate: RATE,
        }
    }
}

impl OpusSpec {
    fn page_duration_ms(&self) -> u32 {
        self.frames_per_page() * self.packet_length_ms
    }

    fn frames_per_page(&self) -> u32 {
        self.page_body_size / self.packet_size
    }

    fn page_size(&self) -> u32 {
        self.page_body_size + self.page_header_size
    }
}

/// OggFile transparently encodes different file types into opus-oggs.
/// It needs to support both `Read` and `Seek` to enable access via RangeRequests
pub struct OpusFile {
    source: PathBuf,
    spec: OpusSpec,
    pipeline: gst::Pipeline,
    byte_offset: usize,
    header_data: Option<Vec<u8>>,
    stream: Stream,
    packet_num: u32,
    cached_page: Option<Page>,
    wrote_page_header: usize,
    wrote_page_body: usize,
    to_discard: usize,
    duration: gst::ClockTime,
}

impl OpusFile {
    pub fn create(source: impl AsRef<Path>) -> Result<Self, EncoderError> {
        let pipeline = Self::build_pipeline(source.as_ref().to_string_lossy().as_ref())?;
        pipeline.set_state(gst::State::Playing)?;
        let (res, _, _) = pipeline.get_state(gst::CLOCK_TIME_NONE);
        res?;
        let duration: gst::ClockTime = pipeline
            .query_duration()
            .ok_or(EncoderError::InvalidMediaFile)?;
        let out = Self {
            source: source.as_ref().to_owned(),
            spec: OpusSpec::default(),
            pipeline,
            byte_offset: 0,
            header_data: None,
            stream: Stream::new(0xf01353),
            packet_num: 0,
            cached_page: None,
            wrote_page_header: 0,
            wrote_page_body: 0,
            to_discard: 0,
            duration,
        };

        Ok(out)
    }

    fn get_sink(&self) -> Result<gst_app::AppSink, EncoderError> {
        self.pipeline
            .get_by_name(SINK_NAME)
            .ok_or(EncoderError::InvalidState("No AppSink (yet)"))
            .map(|element| {
                element
                    .dynamic_cast::<gst_app::AppSink>()
                    .expect("appsink was not an AppSink")
            })
    }

    fn get_encoder(&self) -> Result<gst::Element, EncoderError> {
        self.pipeline
            .get_by_name(ENCODER_NAME)
            .ok_or(EncoderError::InvalidState("No encoder (yet)"))
    }

    /// Get header page if it exsits, build it otheriwse
    fn get_header_page_data(&mut self) -> Result<&Vec<u8>, EncoderError> {
        if self.header_data.is_some() {
            Ok(self.header_data.as_ref().unwrap())
        } else {
            let header_data = self.build_header_data()?;
            self.header_data = Some(header_data);
            Ok(self.header_data.as_ref().unwrap())
        }
    }

    fn reset_stream(&mut self) {
        self.stream.reset();
        self.header_data = None;
    }

    fn build_header_data(&mut self) -> Result<Vec<u8>, EncoderError> {
        let mut data = Vec::new();
        for (i, packet_data) in self.get_opus_header_data()?.iter().enumerate() {
            let mut packet = Packet::new(&packet_data);
            if i == 0 {
                packet.set_bos(true);
            }
            self.stream.packetin(&mut packet);
            if i > 0 {
                loop {
                    let new_page = self.stream.flush();
                    if let Some(page) = new_page {
                        data.extend(page.header);
                        data.extend(page.body);
                    } else {
                        break;
                    }
                }
            } else {
                let new_page = self.stream.flush().ok_or(EncoderError::NoStreamHeader)?;
                data.extend(new_page.header);
                data.extend(new_page.body);
            }
        }
        if data.len() < 2 {
            return Err(EncoderError::NoStreamHeader);
        }
        Ok(data)
    }

    /// Returns the opus id header and comment header
    ///
    /// Each of the headers are not packed into ogg pages yet. Each header is represented as an
    /// individual Vec<u8>.
    fn get_opus_header_data(&self) -> Result<Vec<Vec<u8>>, EncoderError> {
        let sink = self.get_sink()?;
        let caps: Vec<gst::Caps> = sink
            .get_sink_pads()
            .iter()
            .filter_map(|pad| {
                let caps = pad.get_current_caps();
                if caps
                    .as_ref()
                    .and_then(|caps| {
                        caps.get_structure(0)
                            .map(|s| s.get_name().starts_with("audio/"))
                    })
                    .unwrap_or(false)
                {
                    caps
                } else {
                    None
                }
            })
            .collect();
        if caps.len() > 0 {
            log::warn!("More than one audio stream, taking the first one!");
        } else if caps.len() == 0 {
            log::error!("No Audio stream found.");
            return Err(EncoderError::InvalidState("No audio stream"));
        }
        let caps = &caps[0];
        let s = caps.get_structure(0).unwrap();
        let header = s
            .get::<gst::Array>("streamheader")?
            .ok_or(EncoderError::NoStreamHeader)?;
        let mut headers = Vec::new();
        for element in header.as_slice() {
            let buf = element
                .downcast_ref::<gst::Buffer>()
                .ok_or(EncoderError::NoStreamHeader)?
                .get()
                .ok_or(EncoderError::NoStreamHeader)?;
            let buf_map = buf.map_readable()?;
            // Headers aren't large and only exist once per file so just copy them
            headers.push(buf_map.to_owned());
        }
        Ok(headers)
    }

    fn get_next_page(&mut self) -> Result<Option<Page>, EncoderError> {
        let mut pkt = 0;
        while let Ok(sample) = self.get_sink()?.pull_sample() {
            println!("Sample info: {:?}", sample.get_info());
            println!("Buffer pts: {:?}", sample.get_buffer().unwrap().get_pts());
            println!("Buffer dts: {:?}", sample.get_buffer().unwrap().get_dts());
            println!("Buffer len: {:?}", sample.get_buffer().unwrap().get_size());
            let eos = self
                .get_sink()?
                .get_property("eos")?
                .get_some::<bool>()
                .unwrap_or(false);
            let buf = sample.get_buffer().unwrap();
            let buf_map = buf.map_readable().unwrap();
            let mut packet = Packet::new(&buf_map);
            packet.set_packetno(self.packet_num as i64);
            let is_eos = (self.byte_offset as u64
                + self.spec.page_header_size as u64
                + (self.spec.packet_size * (pkt + 1)) as u64)
                == self.size_bytes()? as u64;
            if is_eos {
                packet.set_eos(true);
            }
            self.packet_num += 1;
            packet.set_granulepos(
                (self.packet_num * (RATE / (1000 / FRAME_SIZE)))
                    .try_into()
                    .unwrap(),
            );
            self.stream.packetin(&mut packet);
            if pkt + 1 == self.spec.frames_per_page() || is_eos {
                if let Some(page) = self.stream.flush() {
                    return Ok(Some(page));
                }
            }
            pkt += 1;
        }
        Ok(None)
    }

    fn build_pipeline(file_name: &str) -> Result<gst::Pipeline, EncoderError> {
        gst::init().unwrap();

        let pipeline = gst::Pipeline::new(None);
        let src = gst::ElementFactory::make("filesrc", None)
            .map_err(|e| EncoderError::from(e).maybe_set_element("filesrc"))?;
        let decodebin = gst::ElementFactory::make("decodebin", None)
            .map_err(|e| EncoderError::from(e).maybe_set_element("decodebin"))?;

        let caps = gst::Caps::builder("audio/x-raw")
            .field("rate", &8000)
            .build();

        pipeline
            .add_many(&[&src, &decodebin])
            .expect("Failed to add");
        gst::Element::link_many(&[&src, &decodebin]).expect("Failed to link");
        let pipeline_weak = pipeline.downgrade();

        decodebin.connect_pad_added(move |_dbin, src_pad| {
            let result = (|| -> Result<(), EncoderError> {
                let pipeline = pipeline_weak
                    .upgrade()
                    .expect("Unable to upgrade pipeline reference.");

                let is_audio = src_pad
                    .get_current_caps()
                    .and_then(|caps| {
                        caps.get_structure(0)
                            .map(|s| s.get_name().starts_with("audio/"))
                    })
                    .unwrap_or(false);
                log::trace!(
                    "Pad of type {} discovered.",
                    if is_audio { "audio" } else { "non-audio" }
                );
                if is_audio {
                    let audioconvert = gst::ElementFactory::make("audioconvert", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("audioconvert"))?;
                    let audioresample = gst::ElementFactory::make("audioresample", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("audioresample"))?;
                    let rate_filter = gst::ElementFactory::make("capsfilter", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("capsfilter"))?;
                    let opusenc = gst::ElementFactory::make("opusenc", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("opusenc"))?;
                    opusenc.set_property_from_str("name", ENCODER_NAME);
                    opusenc.set_property_from_str("bandwidth", "narrowband");
                    opusenc.set_property("hard-resync", &true.to_value());
                    opusenc.set_property("perfect-timestamp", &true.to_value());
                    rate_filter.set_property("caps", &caps).unwrap();
                    let sink = gst::ElementFactory::make("appsink", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("appsink"))?;
                    sink.set_property_from_str("name", SINK_NAME);

                    let app_sink = sink.dynamic_cast::<gst_app::AppSink>().unwrap();
                    app_sink.set_property("sync", &false)?;
                    // We need some max buffer count to ensure that not reading from the OpusFile
                    // for a while doesn't fill up the system memory.
                    app_sink.set_property("max-buffers", &(128 as u32))?;
                    app_sink.set_wait_on_eos(true);
                    let sink = app_sink.dynamic_cast::<gst::Element>().unwrap();

                    let elements = &[&audioconvert, &audioresample, &rate_filter, &opusenc, &sink];
                    pipeline.add_many(elements)?;
                    gst::Element::link_many(elements)?;

                    for e in elements {
                        e.sync_state_with_parent()?;
                    }

                    let sink_pad = audioconvert.get_static_pad("sink").unwrap();
                    src_pad.link(&sink_pad)?;
                }
                Ok(())
            })();
            match result {
                Err(e) => {
                    log::error!("Failed to handle new pad {}", e);
                    // TODO: store error in instance to ensure that read calls can return it
                }
                Ok(()) => (),
            }
        });
        src.set_property_from_str("location", file_name);
        pipeline.set_state(gst::State::Ready)?;
        // pipeline.set_state(gst::State::Playing)?;
        Ok(pipeline)
    }

    /// Given a byte offset return milliseconds and a byte offset
    fn byte_to_offset(&mut self, position: usize) -> Result<Offset, EncoderError> {
        if self.get_header_page_data()?.len() >= position as usize {
            return Ok(Offset::ByteOffset(position as u64));
        }
        let offset_no_header = position - self.get_header_page_data()?.len();
        // After seeking the audio 'fades in' i.e. it's volume is slowly increased
        // This means we need to discard the first second or so of data after a seek
        let pages_to_prerender = 2;
        let mut pages =
            (offset_no_header / ((self.spec.page_header_size + self.spec.page_body_size) as usize));
        if pages < 3 {
            return Ok(Offset::ByteOffset(position as u64));
        }
        pages -= pages_to_prerender;
        let extra_bytes = offset_no_header
            - ((self.spec.page_header_size + self.spec.page_body_size) as usize * pages);
        let millis = pages as u32 * self.spec.page_duration_ms();
        Ok(Offset::TemporalOffset(TemporalOffset {
            millis,
            packet: (pages * (self.spec.page_body_size / self.spec.packet_size) as usize) as u32,
            extra_bytes: extra_bytes as u32,
        }))
    }
}

impl Drop for OpusFile {
    fn drop(&mut self) {
        self.pipeline.set_state(gst::State::Null);
    }
}

impl Read for OpusFile {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let mut wrote = 0;
        let header_data = self.get_header_page_data().unwrap().to_owned();
        let size = self
            .size_bytes()
            .map_err(|e| IoError::new(IoErrorKind::Other, "Unable to calculate size"))?;
        if self.byte_offset as u64 + buf.len() as u64 > size {
            buf = &mut buf[..(size - self.byte_offset as u64) as usize];
        }
        if self.byte_offset < header_data.len() {
            println!("Writing header");
            let wrote_header = buf.write(&header_data.as_slice()[self.byte_offset..])?;
            wrote += wrote_header;
            self.byte_offset += wrote_header;
        }
        if self.byte_offset >= header_data.len() {
            let wrote_data = self.read_from_pages(&mut buf[..])?;
            wrote += wrote_data;
        }
        Ok(wrote)
    }
}

impl OpusFile {
    fn read_from_pages(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let mut wrote = 0;
        let mut fail_count = 0;
        loop {
            if self.cached_page.is_none() {
                self.cached_page = self.get_next_page().map_err(|e| {
                    IoError::new(IoErrorKind::Other, format!("Encoder error: {}", e))
                })?;
                if self.cached_page.is_none() {
                    fail_count += 1;
                    if fail_count > 3 {
                        return Ok(wrote);
                    }
                } else {
                    fail_count = 0;
                }
                self.wrote_page_header = 0;
                self.wrote_page_body = 0;
                println!(
                    "header size: {:?}",
                    self.cached_page.as_ref().map(|p| p.header.len())
                );
                println!(
                    "body size: {:?}",
                    self.cached_page.as_ref().map(|p| p.body.len())
                );
            }
            loop {
                if wrote < buf.len() {
                    if self.cached_page.is_some() {
                        let (wrote_header, discarded_header) =
                            self.write_header(&mut buf[wrote..])?;
                        wrote += wrote_header;
                        self.byte_offset += wrote_header;
                        let (wrote_body, discarded_body) = self.write_body(&mut buf[wrote..])?;
                        wrote += wrote_body;
                        self.byte_offset += wrote_body;
                    } else {
                        break;
                    }
                } else {
                    return Ok(wrote);
                }
            }
        }
        return Ok(wrote);
    }

    fn write_header(&mut self, mut buf: &mut [u8]) -> std::io::Result<(usize, usize)> {
        if let Some(ref page) = self.cached_page {
            let mut discarded = 0;
            let mut to_write_header = &page.header[self.wrote_page_header..];
            if to_write_header.len() > 0 && self.to_discard > 0 {
                if to_write_header.len() > self.to_discard {
                    to_write_header = &to_write_header[self.to_discard..];
                    discarded += self.to_discard;
                    self.to_discard = 0;
                } else {
                    self.to_discard -= to_write_header.len();
                    discarded += to_write_header.len();
                    to_write_header = &[];
                }
            }
            let wrote = buf.write(to_write_header)?;
            to_write_header = &to_write_header[wrote..];
            self.wrote_page_header = page.header.len() - to_write_header.len();
            Ok((wrote, discarded))
        } else {
            Err(IoError::new(
                IoErrorKind::NotConnected,
                "Page not initialized".to_owned(),
            ))
        }
    }

    fn write_body(&mut self, mut buf: &mut [u8]) -> std::io::Result<(usize, usize)> {
        let mut discarded = 0;
        if let Some(ref page) = self.cached_page {
            let mut to_write_body = &page.body[self.wrote_page_body..];
            if to_write_body.len() > 0 && self.to_discard > 0 {
                if to_write_body.len() > self.to_discard {
                    to_write_body = &to_write_body[self.to_discard..];
                    discarded += self.to_discard;
                    self.to_discard = 0;
                } else {
                    self.to_discard -= to_write_body.len();
                    discarded += to_write_body.len();
                    to_write_body = &[];
                }
            }
            let wrote = buf.write(to_write_body)?;
            to_write_body = &to_write_body[wrote..];
            self.wrote_page_body = page.body.len() - to_write_body.len();
            if to_write_body.len() == 0 {
                self.cached_page = None;
            }
            Ok((wrote, discarded))
        } else {
            Err(IoError::new(
                IoErrorKind::NotConnected,
                "Page not initialized".to_owned(),
            ))
        }
    }

    fn size_bytes(&mut self) -> Result<u64, EncoderError> {
        let frame_len = self.spec.packet_length_ms as u64 * 1_000_000;
        let duration = self.duration.nanoseconds().unwrap_or(0);
        let num_packets = if duration % frame_len == 0 {
            duration / frame_len
        } else {
            duration / frame_len + 1
        };
        let num_pages = num_packets / self.spec.frames_per_page() as u64;
        let extra_packets = num_packets % self.spec.frames_per_page() as u64;
        let last_page_size = if extra_packets != 0 {
            self.spec.page_header_size + self.spec.packet_size * extra_packets as u32
        } else {
            0
        };
        Ok((self.get_header_page_data()?.len() as u64
            + (num_pages * self.spec.page_size() as u64)
            + last_page_size as u64))
    }
}

impl Seek for OpusFile {
    fn seek(&mut self, seek_from: SeekFrom) -> std::io::Result<u64> {
        let size = self
            .size_bytes()
            .map_err(|e| IoError::new(IoErrorKind::Other, "Unable to calculate size"))?;
        let mut pos = match seek_from {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(pos) => size - pos as u64,
            SeekFrom::Current(pos) => self.byte_offset as u64 + pos as u64,
        };
        if pos > size {
            pos = size;
        }
        self.byte_offset = pos as usize;
        let offset = self.byte_to_offset(pos as usize).map_err(|e| {
            IoError::new(
                IoErrorKind::Other,
                format!("Failed to calculate byte offset: {}", e),
            )
        })?;
        self.reset_stream();
        match offset {
            Offset::TemporalOffset(offset) => {
                println!(
                    "Seeking to ms {:?}, will discard an additional {:?} bytes",
                    offset.millis, offset.extra_bytes,
                );
                self.pipeline.set_state(gst::State::Paused).map_err(|e| {
                    IoError::new(
                        IoErrorKind::Other,
                        format!("Failed to pause underlying pipeline: {}", e),
                    )
                })?;
                let (res, _, _) = self.pipeline.get_state(gst::CLOCK_TIME_NONE);
                println!("--EEEEE");
                let seek_res = self.pipeline.seek(
                    1.0,
                    gst::SeekFlags::ACCURATE | gst::SeekFlags::KEY_UNIT | gst::SeekFlags::FLUSH,
                    gst::SeekType::Set,
                    gst::format::GenericFormattedValue::Time(gst::ClockTime::from_mseconds(
                        offset.millis as u64,
                    )),
                    gst::SeekType::None,
                    gst::format::GenericFormattedValue::Time(0.into()),
                );
                self.pipeline.set_state(gst::State::Playing).map_err(|e| {
                    IoError::new(
                        IoErrorKind::Other,
                        format!("Failed to unpause underlying pipeline: {}", e),
                    )
                })?;
                let (res, _, _) = self.pipeline.get_state(gst::CLOCK_TIME_NONE);
                self.to_discard = offset.extra_bytes as usize;
                self.packet_num = offset.packet;
                self.stream
                    .set_pageno((self.packet_num / self.spec.frames_per_page()) as i64)
            }
            Offset::ByteOffset(offset) => {
                self.pipeline.set_state(gst::State::Null).map_err(|e| {
                    IoError::new(
                        IoErrorKind::Other,
                        format!("Failed to terminate underlying pipeline: {}", e),
                    )
                })?;
                let (res, _, _) = self.pipeline.get_state(gst::CLOCK_TIME_NONE);
                self.pipeline = Self::build_pipeline(self.source.to_string_lossy().as_ref())
                    .map_err(|e| {
                        IoError::new(
                            IoErrorKind::Other,
                            format!("Failed to play underlying pipeline: {}", e),
                        )
                    })?;
                self.pipeline.set_state(gst::State::Playing).map_err(|e| {
                    IoError::new(
                        IoErrorKind::Other,
                        format!("Failed to play underlying pipeline: {}", e),
                    )
                })?;
                let (res, _, _) = self.pipeline.get_state(gst::CLOCK_TIME_NONE);
                let header_size = self.get_header_page_data().unwrap().len();
                if offset > header_size as u64 {
                    self.to_discard = (offset - header_size as u64) as usize;
                } else {
                    self.to_discard = 0;
                }
                self.packet_num = 0;
            }
        }
        self.cached_page = None;
        self.wrote_page_header = 0;
        self.wrote_page_body = 0;
        println!("--DDDDD");
        Ok(pos)
    }
}

#[cfg(test)]
mod test {
    use super::OpusFile;
    use env_logger;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom, Write};

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn read_header() {
        let mut opus_file = OpusFile::create("test-data/all.m4b").unwrap();
        let mut data = Vec::new();
        for _ in 0..2048 {
            data.push(0);
        }
        let read = opus_file.read(&mut data).unwrap();
        let header_target = [
            79, 103, 103, 83, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 83, 19, 240, 0, 0, 0, 0, 0, 171, 204,
            149, 10, 1, 19, 79, 112, 117, 115, 72, 101, 97, 100, 1, 2, 56, 1, 64, 31, 0, 0, 0, 0,
            0,
        ];
        for i in 0..header_target.len() {
            assert_eq!(header_target[i], data[i]);
        }
    }

    #[test]
    fn read_body() {
        let mut opus_file_a = OpusFile::create("test-data/all.m4b").unwrap();
        let mut out = File::create("/tmp/test.ogg").unwrap();
        let mut data_a = Vec::new();
        for _ in 0..1_000_000 {
            data_a.push(0);
        }

        let mut total_a = 0;
        loop {
            let read_a = opus_file_a.read(&mut data_a).unwrap();
            out.write_all(&data_a[..read_a]);
            total_a += read_a;
            if read_a == 0 {
                break;
            }
        }
        println!("Read a total of {}", total_a);
    }

    #[test]
    fn reproducible_encodes() {
        let mut opus_file_a = OpusFile::create("test-data/sine_silence_1_1_30_volume.mp3").unwrap();
        let mut opus_file_b = OpusFile::create("test-data/sine_silence_1_1_30_volume.mp3").unwrap();
        let mut data_a = Vec::new();
        let mut data_b = Vec::new();
        for _ in 0..1_000_000 {
            data_a.push(0);
            data_b.push(0);
        }

        loop {
            let read_a = opus_file_a.read(&mut data_a).unwrap();
            let read_b = opus_file_b.read(&mut data_b).unwrap();
            assert_eq!(read_a, read_b);
            for (i, (a, b)) in data_a.iter().zip(data_b.iter()).enumerate() {
                if !(a == b) {
                    println!("Position {:?}", i);
                }
                assert_eq!(a, b)
            }
            if read_a == 0 {
                break;
            }
        }
    }

    #[test]
    fn byte_offset() {
        let mut opus_file = OpusFile::create("test-data/sine_silence_1_1_30_volume.mp3").unwrap();
        let pos = 150_000;
        let offset = match opus_file.byte_to_offset(pos).unwrap() {
            super::Offset::TemporalOffset(o) => o,
            _ => panic!("Unexpected offset"),
        };
        println!("Offset: {:?}", offset);
        assert_eq!(offset.millis, 17160);
        assert_eq!(
            offset.extra_bytes,
            2423 + (opus_file.spec.page_body_size + opus_file.spec.page_header_size) * 2
        );
        let full_page_bytes = ((offset.packet / 26)
            * (opus_file.spec.page_body_size + opus_file.spec.page_header_size))
            as usize;
        assert_eq!(
            full_page_bytes
                + opus_file.get_header_page_data().unwrap().len()
                + offset.extra_bytes as usize,
            pos
        );
    }

    fn read_loop(mut reader: &mut dyn Read, buf: &mut [u8]) -> usize {
        let mut read = 0;
        loop {
            let new_read = reader.read(&mut buf[read..]).unwrap();
            read += new_read;
            println!("read: {}", read);
            if read == buf.len() || new_read == 0 {
                return read;
            }
        }
    }

    #[test]
    fn hit_page_boundary() {
        let mut opus = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let sector_size = 147_577;
        let mut data = Vec::with_capacity(sector_size);
        assert_eq!(
            (opus.spec.page_header_size + opus.spec.page_body_size) * 35
                + opus.get_header_page_data().unwrap().len() as u32,
            sector_size as u32
        );

        for _ in 0..sector_size {
            data.push(0);
        }
        let read = opus.read(&mut data).unwrap();
        assert_eq!(sector_size, read);
        let mut ogg_ident = vec![0, 0, 0, 0];
        let read = opus.read(&mut ogg_ident).unwrap();
        assert_eq!(std::str::from_utf8(&ogg_ident).unwrap(), "OggS");

        let mut opus_seek = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let seek = opus_seek.seek(SeekFrom::Start(sector_size as u64)).unwrap();
        assert_eq!(seek, sector_size as u64);

        let mut ogg_ident_seek = vec![0, 0, 0, 0];
        let read = opus_seek.read(&mut ogg_ident_seek).unwrap();
        assert_eq!(std::str::from_utf8(&ogg_ident_seek).unwrap(), "OggS");
    }

    #[test]
    fn just_before_page_boundary() {
        let mut opus = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let sector_size = 147_576;
        let mut data = Vec::with_capacity(sector_size);

        for _ in 0..sector_size {
            data.push(0);
        }
        let read = opus.read(&mut data).unwrap();
        assert_eq!(sector_size, read);
        let mut ogg_ident = vec![0, 0, 0, 0, 0];
        let read = opus.read(&mut ogg_ident).unwrap();
        assert_eq!(std::str::from_utf8(&ogg_ident[1..]).unwrap(), "OggS");

        let mut opus_seek = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let seek = opus_seek.seek(SeekFrom::Start(sector_size as u64)).unwrap();
        assert_eq!(seek, sector_size as u64);

        let mut ogg_ident_seek = vec![0, 0, 0, 0, 0];
        let read = opus_seek.read(&mut ogg_ident_seek).unwrap();
        assert_eq!(std::str::from_utf8(&ogg_ident_seek[1..]).unwrap(), "OggS");
    }

    #[test]
    fn seek_is_the_same() {
        init();
        let mut opus_file_seek =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut opus_file_read =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data_read = Vec::new();
        let mut data_seek = Vec::new();
        let sector_size = 150_000;

        for _ in 0..sector_size {
            data_read.push(0);
            data_seek.push(0);
        }

        let mut stitched = File::create("/tmp/stitched.ogg").unwrap();
        let mut complete = File::create("/tmp/complete.ogg").unwrap();

        // Discard sector_size bytes
        let read = read_loop(&mut opus_file_read, &mut data_read);
        complete.write_all(&data_read[..read]);
        stitched.write_all(&data_read[..read]);
        assert_eq!(read, sector_size);

        let read = read_loop(&mut opus_file_read, &mut data_read);
        complete.write_all(&data_read[..read]);

        let seek = opus_file_seek
            .seek(SeekFrom::Start(sector_size as u64))
            .unwrap();
        let read_seek = read_loop(&mut opus_file_seek, &mut data_seek);
        stitched.write_all(&data_seek[..read_seek]);

        assert_eq!(read, read_seek);

        for (i, (s, r)) in data_seek[..read_seek]
            .iter()
            .zip(data_read[..read].iter())
            .enumerate()
        {
            if (s != r) {
                println!("{}: {}, {}", i, s, r);
            }
            assert_eq!(s, r);
        }
    }

    #[test]
    fn seek_many() {
        init();
        let mut opus_file_seek =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data_seek = Vec::new();
        let sector_size = 15_000;

        for _ in 0..sector_size {
            data_seek.push(0);
        }

        let mut stitched = File::create("/tmp/many_seeks.ogg".to_owned()).unwrap();

        let mut i = 0;
        loop {
            let read = read_loop(&mut opus_file_seek, &mut data_seek);
            stitched.write_all(&data_seek[..read]);
            opus_file_seek = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
            i += 1;
            opus_file_seek.seek(SeekFrom::Start(sector_size as u64 * i));
            if read == 0 {
                break;
            }
        }
    }

    #[test]
    fn seek_in_header() {
        init();

        for offset in (0..20).step_by(1) {
            let mut opus_file_seek =
                OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
            let size = 243376;
            let mut data = Vec::with_capacity(size);

            println!("LEN: {:?}", data.len());
            for _ in 0..size {
                data.push(0);
            }

            let mut out = File::create(format!("/tmp/seek_in_header_{}.ogg", offset)).unwrap();

            opus_file_seek.read(&mut data[..200]).unwrap();
            let first_byte = data[0];
            opus_file_seek.seek(SeekFrom::Start(offset)).unwrap();
            let read = opus_file_seek.read(&mut data[offset as usize..]).unwrap();
            assert_eq!(first_byte, data[0]);

            out.write(&data[..read + offset as usize]).unwrap();
        }
    }

    #[test]
    fn seek_to_zero() {
        init();

        let mut opus_file_read =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut opus_file_seek =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let size = 3_000;
        let mut data_read = Vec::with_capacity(size);
        let mut data_seek = Vec::with_capacity(size);
        let mut data_discard = Vec::with_capacity(size);

        for _ in 0..size {
            data_read.push(0);
            data_seek.push(0);
            data_discard.push(0);
        }

        opus_file_seek.read(&mut data_discard).unwrap();
        opus_file_read.read(&mut data_read).unwrap();
        opus_file_seek.seek(SeekFrom::Start(0)).unwrap();
        let read = opus_file_seek.read(&mut data_seek).unwrap();

        let mut out = File::create("/tmp/seek_to_zero.ogg").unwrap();
        out.write_all(&data_seek[..read]).unwrap();

        for (i, (read, seek)) in data_read.iter().zip(data_seek.iter()).enumerate() {
            println!("Index: {}", i);
            assert_eq!(read, seek);
        }
    }

    #[test]
    fn fill_up_buffer() {
        init();
        let mut opus_file_read =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data = Vec::new();
        let size = 400;

        for _ in 0..size {
            data.push(0);
        }

        let read = opus_file_read.read(&mut data).unwrap();
        assert_eq!(size, read);
    }

    #[test]
    fn faster_than_real_time() {
        use std::time::SystemTime;
        init();
        let start = SystemTime::now();

        let mut file = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data = Vec::new();
        let sector_size = 10_000;

        for _ in 0..sector_size {
            data.push(0);
        }

        let mut out_file = File::create(format!("/tmp/out.ogg")).unwrap();

        loop {
            let read = file.read(&mut data).unwrap();
            out_file.write_all(&data[..read]);
            if read == 0 {
                break;
            }
        }
        // This should be a lot faster than real time!
        assert!(start.elapsed().unwrap().as_secs() < 5);
    }
}
