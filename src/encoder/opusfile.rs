extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use std::convert::TryInto;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use gst::prelude::*;
use gst::{GstBinExt, MessageView};
use ogger::{Packet, Page, Stream};

use crate::encoder::EncoderError;

static SINK_NAME: &'static str = "appsink-0";
static ENCODER_NAME: &'static str = "opusenc";

// At some point these should probably become runtime configurable
static FRAME_SIZE: u32 = 20;
static RATE: u32 = 48_000;

/// OggFile transparently encodes different file types into opus-oggs.
/// It needs to support both `Read` and `Seek` to enable access via RangeRequests
pub struct OpusFile {
    pipeline: gst::Pipeline,
    byte_offset: usize,
    header_data: Option<Vec<u8>>,
    stream: Stream,
    packet_num: u32,
    cached_page: Option<Page>,
    wrote_page_header: usize,
    wrote_page_body: usize,
    to_discard: usize,
}

impl OpusFile {
    pub fn create(source: impl AsRef<Path>) -> Result<Self, EncoderError> {
        let pipeline = Self::build_pipeline(source.as_ref().to_string_lossy().as_ref())?;
        let bus = pipeline.get_bus().unwrap();
        let out = Self {
            pipeline,
            byte_offset: 0,
            header_data: None,
            stream: Stream::new(0xf01353),
            packet_num: 0,
            cached_page: None,
            wrote_page_header: 0,
            wrote_page_body: 0,
            to_discard: 0,
        };
        out.pipeline.set_state(gst::State::Playing)?;
        // Wait for pipeline to be ready
        for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
            match msg.view() {
                MessageView::StateChanged(s) => {
                    let name = s
                        .get_src()
                        .unwrap()
                        .get_property("name")
                        .unwrap()
                        .get::<String>()
                        .unwrap();
                    if name.unwrap().starts_with("pipeline")
                        && s.get_current() == gst::State::Playing
                    {
                        break;
                    }
                }
                MessageView::Eos(..) => break,
                MessageView::Error(e) => log::error!("GStreamer Error: {:?}", e),
                e => (),
            }
        }

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
            packet.set_eos(eos);
            self.packet_num += 1;
            packet.set_granulepos(
                (self.packet_num * (RATE / (1000 / FRAME_SIZE)))
                    .try_into()
                    .unwrap(),
            );
            self.stream.packetin(&mut packet);
            if let Some(page) = self.stream.pageout() {
                return Ok(Some(page));
            }
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

    fn drain_sink(&self) -> Result<(), EncoderError> {
        loop {
            let sample = self
                .get_sink()?
                .try_pull_preroll(gst::ClockTime::from_mseconds(10));
            if sample.is_none() {
                break;
            }
        }
        loop {
            let sample = self
                .get_sink()?
                .try_pull_sample(gst::ClockTime::from_mseconds(10));
            if sample.is_none() {
                break;
            }
        }
        Ok(())
    }

    /// Returns a time offset in seconds + a small byte offset to seek to a specific byte
    fn byte_to_offset(&mut self, byte_index: u64) -> Result<(u64, u64), EncoderError> {
        // TODO: handle seeks that are shorter than the header
        if self.get_header_page_data()?.len() > byte_index as usize {
            panic!("Seeking that doesn't go beyond the header is not supported!")
        }
        let offset_no_header = byte_index as usize - self.get_header_page_data()?.len();
        println!("Header size: {}", self.get_header_page_data()?.len());
        let bitrate_type_enum = self.get_encoder()?.get_property("bitrate-type").unwrap();
        let bitrate_type = gst::glib::EnumValue::from_value(&bitrate_type_enum)
            .unwrap()
            .get_value();
        assert_eq!(bitrate_type, 0); // assert we are in CBR mode
        let frame_size = self.get_encoder()?.get_property("frame-size").unwrap();
        assert_eq!(
            20,
            gst::glib::EnumValue::from_value(&frame_size)
                .unwrap()
                .get_value()
        );
        let bitrate = self
            .get_encoder()?
            .get_property("bitrate")
            .unwrap()
            .get::<i32>()
            .unwrap()
            .unwrap() as usize;

        let byterate = bitrate / 8;
        let full_seconds = self.bytes_to_full_seconds(offset_no_header)? - 1;
        let mut num_headers_skipped = full_seconds * byterate / 4160;
        if (full_seconds * byterate) % 4160 != 0 {
            num_headers_skipped += 1;
        }
        Ok((
            full_seconds as u64,
            (offset_no_header as usize - (full_seconds * byterate) - (num_headers_skipped * 53))
                as u64,
        ))
    }

    fn bytes_to_full_seconds(&self, content_bytes: usize) -> Result<usize, EncoderError> {
        let bitrate = self
            .get_encoder()?
            .get_property("bitrate")
            .unwrap()
            .get::<i32>()
            .unwrap()
            .unwrap() as usize;
        let mut num_pages = content_bytes / (4160 + 53);
        if content_bytes % (4160 + 53) != 0 {
            num_pages += 1
        }
        let opus_bytes = content_bytes - (num_pages * 53);
        Ok(opus_bytes / (bitrate / 8))
    }

    /// Bytes in a second, excluding the file header but including ogg page headers.
    fn second_to_content_bytes(&self, seconds: u64) -> Result<u64, EncoderError> {
        // TODO: explore 53 and 4160 magic numbers
        let bitrate = self
            .get_encoder()?
            .get_property("bitrate")
            .unwrap()
            .get::<i32>()
            .unwrap()
            .unwrap() as u64;
        let opus_bytes_per_sec = bitrate / 8;
        let total_opus_bytes = seconds * opus_bytes_per_sec;
        let mut ogg_pages = total_opus_bytes / 4160;
        if total_opus_bytes % 4160 != 0 {
            ogg_pages += 1;
        };
        let total_ogg_header_bytes = ogg_pages * 53;
        let total_content_bytes = total_opus_bytes + total_ogg_header_bytes;
        Ok(total_content_bytes)
    }
}

impl Drop for OpusFile {
    fn drop(&mut self) {
        self.pipeline.set_state(gst::State::Null);
    }
}

impl Read for OpusFile {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        println!("BUFFER SIZE: {:?}", buf.len());
        let mut wrote = 0;
        let header_data = self.get_header_page_data().unwrap().to_owned();
        if self.byte_offset < header_data.len() {
            println!("Writing header");
            let wrote_header = buf.write(&header_data.as_slice()[self.byte_offset..])?;
            wrote += wrote_header;
            println!("WROTE HEADER: {:?}, {:?}", wrote_header, wrote);
            println!(
                "AFTER HEADER: {:?}, {:?}",
                buf[wrote_header - 1],
                buf[wrote_header]
            );
            self.byte_offset += wrote_header;
        }
        if self.byte_offset >= header_data.len() && wrote < buf.len() {
            let wrote_data = self.read_from_pages(&mut buf)?;
            wrote += wrote_data;
            println!(
                "Last elements: {:?}, {:?}",
                buf[buf.len() - 2],
                buf[buf.len() - 1]
            );
            println!(
                "Wrote page data: {} wrote total data: {}",
                wrote_data, wrote
            );
            self.byte_offset += wrote_data;
        }
        Ok(wrote)
    }
}

impl OpusFile {
    fn read_from_pages(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let mut wrote_total = 0;
        loop {
            if self.cached_page.is_none() {
                self.cached_page = self.get_next_page().map_err(|e| {
                    IoError::new(IoErrorKind::Other, format!("Encoder error: {}", e))
                })?;
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
            if let Some(ref page) = self.cached_page {
                'inner_header: loop {
                    let mut wrote = 0;
                    println!("self.wrote_page_header {}", self.wrote_page_header);
                    if self.to_discard > 0 {
                        assert_eq!(self.wrote_page_header, 0);
                        if page.header.len() > self.to_discard {
                            wrote = buf.write(&page.header[self.to_discard..])?;
                            wrote_total += wrote;
                            self.wrote_page_header = wrote + self.to_discard;
                            self.to_discard = 0;
                            println!("self.wrote_page_header {}", self.wrote_page_header);
                        } else {
                            self.to_discard -= page.header.len();
                            println!(
                                "Discarded {} bytes, still {} bytes to discard",
                                page.header.len(),
                                self.to_discard
                            );
                            println!("self.wrote_page_header {}", self.wrote_page_header);
                            break 'inner_header;
                        }
                    } else if self.wrote_page_header < page.header.len() {
                        println!("Writing from offset {}", self.wrote_page_header);
                        wrote = buf.write(&page.header[self.wrote_page_header..])?;
                        wrote_total += wrote;
                        self.wrote_page_header += wrote;
                    }
                    if wrote == 0 && self.wrote_page_header == page.header.len() {
                        break;
                    } else if wrote == 0 {
                        return Ok(wrote_total);
                    }
                }
                'inner: loop {
                    let wrote;
                    if self.to_discard > 0 {
                        assert_eq!(self.wrote_page_body, 0);
                        if page.body.len() > self.to_discard {
                            wrote = buf.write(&page.body[self.to_discard..])?;
                            self.wrote_page_header = self.to_discard + wrote;
                            wrote_total += wrote;
                            self.to_discard = 0;
                        } else {
                            self.to_discard -= page.body.len();
                            println!("Discarded a page");
                            println!(
                                "Discarded {} bytes, still {} bytes to discard",
                                page.body.len(),
                                self.to_discard
                            );
                            self.cached_page = None;
                            break 'inner;
                        }
                    } else {
                        println!(
                            "Already wrote {:?} of the total length: {:?}",
                            self.wrote_page_body,
                            page.body.len()
                        );
                        wrote = buf.write(&page.body[self.wrote_page_body..])?;
                        wrote_total += wrote;
                        self.wrote_page_body += wrote;
                    }
                    if wrote == 0 && self.wrote_page_body == page.body.len() {
                        // the entire page was written
                        self.cached_page = None;
                        break 'inner;
                    } else if wrote == 0 {
                        return Ok(wrote_total);
                    }
                }
            } else {
                return Ok(wrote_total);
            }
        }
    }
}

impl Seek for OpusFile {
    fn seek(&mut self, seek_from: SeekFrom) -> std::io::Result<u64> {
        let pos = match seek_from {
            SeekFrom::Start(pos) => pos,
            _ => unimplemented!(),
        };
        self.byte_offset = pos as usize;
        println!("SEEKING to {}", pos);
        println!("--BBBBB");
        let (target_sec, trim_bytes) = self.byte_to_offset(pos).map_err(|e| {
            IoError::new(
                IoErrorKind::Other,
                format!("Failed to calculate byte offset: {}", e),
            )
        })?;
        println!(
            "Seeking to second {:?}, will discard an additional {:?} bytes",
            target_sec, trim_bytes
        );
        self.pipeline.set_state(gst::State::Paused).map_err(|e| {
            IoError::new(
                IoErrorKind::Other,
                format!("Failed to pause underlying pipeline: {}", e),
            )
        })?;
        let (res, _, _) = self.pipeline.get_state(gst::CLOCK_TIME_NONE);
        self.drain_sink().map_err(|e| {
            IoError::new(
                IoErrorKind::Other,
                format!("Failed to drain underlying pipeline: {}", e),
            )
        })?;
        println!("--EEEEE");
        let seek_res = self.pipeline.seek(
            std::f64::INFINITY,
            gst::SeekFlags::ACCURATE | gst::SeekFlags::KEY_UNIT | gst::SeekFlags::FLUSH,
            gst::SeekType::Set,
            gst::format::GenericFormattedValue::Time(gst::ClockTime::from_seconds(target_sec)),
            gst::SeekType::None,
            gst::format::GenericFormattedValue::Time(0.into()),
        );
        self.to_discard = trim_bytes as usize;
        self.packet_num = (target_sec * 1000 / 20) as u32;
        self.cached_page = None;
        self.wrote_page_header = 0;
        self.wrote_page_body = 0;
        self.pipeline.set_state(gst::State::Playing).map_err(|e| {
            IoError::new(
                IoErrorKind::Other,
                format!("Failed to pause underlying pipeline: {}", e),
            )
        })?;
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
        let mut opus_file = OpusFile::create("test-data/1.mp3").unwrap();
        let offset = opus_file.byte_to_offset(200_000).unwrap();
        println!("Offset: {:?}", offset);
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
    fn byte_calculations() {
        let mut opus = OpusFile::create("test-data/sine_silence_1_1_30_volume.mp3").unwrap();
        let bytes = opus.second_to_content_bytes(6).unwrap();
        println!("Bytes: {:?}", bytes);
        assert_eq!(bytes, 8000 * 6 + 12 * 53);
        assert_eq!(opus.bytes_to_full_seconds(bytes as usize).unwrap(), 6);

        let header_bytes = opus.get_header_page_data().unwrap().len();
        let (secs, extra_bytes) = opus
            .byte_to_offset((header_bytes + bytes as usize + 1234) as u64)
            .unwrap();
        assert_eq!(secs, 5);
        let bytes_5 = opus.second_to_content_bytes(5).unwrap();
        let bytes_6 = opus.second_to_content_bytes(6).unwrap();
        assert_eq!(extra_bytes, 1234 + (bytes_6 - bytes_5));
    }

    #[test]
    fn seek_is_the_same() {
        init();
        let page = 0;

        let mut opus_file_seek =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut opus_file_read =
            OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data_read = Vec::new();
        let mut data_seek = Vec::new();
        let sector_size = 100_000;

        for _ in 0..sector_size {
            data_read.push(0);
            data_seek.push(0);
        }

        let mut stitched = File::create(format!("/tmp/stitched_{}.ogg", page)).unwrap();
        let mut complete = File::create(format!("/tmp/complete_{}.ogg", page)).unwrap();

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
            println!("{}: {}, {}", i, s, r);
            assert_eq!(s, r);
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

        let mut out = File::create(format!("/tmp/fill_up_buffer.ogg")).unwrap();
        let read = opus_file_read.read(&mut data).unwrap();
        assert_eq!(size, read);
    }

    #[test]
    fn faster_than_real_time() {
        init();

        let mut file = OpusFile::create("test-data/sine_silence_1_1_30_volume.wav").unwrap();
        let mut data = Vec::new();
        let sector_size = 100_000;

        for _ in 0..sector_size {
            data.push(0);
        }

        let mut out_file = File::create(format!("/tmp/out.ogg")).unwrap();

        loop {
            let read = read_loop(&mut file, &mut data);
            out_file.write_all(&data[..read]);
            if read == 0 {
                break;
            }
        }
    }
}
