extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use std::convert::TryInto;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Write};
use std::path::PathBuf;

use gst::prelude::*;
use gst::{GstBinExt, MessageView};
use ogger::{Packet, Page, Stream};

use crate::encoder::EncoderError;

static SINK_NAME: &'static str = "appsink-0";

// At some point these should probably become runtime configurable
static FRAME_SIZE: u32 = 20;
static RATE: u32 = 48_000;

/// OggFile transparently encodes different file types into opus-oggs.
/// It needs to support both `Read` and `Seek` to enable access via RangeRequests
struct OpusFile {
    underlying_file: PathBuf,
    pipeline: gst::Pipeline,
    byte_offset: usize,
    header_data: Option<Vec<u8>>,
    stream: Stream,
    packet_num: u32,
    cached_page: Option<Page>,
    wrote_page_header: usize,
    wrote_page_body: usize,
}

impl OpusFile {
    pub fn create(source: PathBuf) -> Result<Self, EncoderError> {
        let pipeline = Self::build_pipeline(source.to_string_lossy().as_ref())?;
        let bus = pipeline.get_bus().unwrap();
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
                        // Set maximum rate to speed things up
                        let seek_res = pipeline.seek(
                            std::f64::INFINITY,
                            gst::SeekFlags::ACCURATE,
                            gst::SeekType::Set,
                            gst::format::GenericFormattedValue::Time(0.into()),
                            gst::SeekType::None,
                            gst::format::GenericFormattedValue::Time(0.into()),
                        );
                        break;
                    }
                }
                MessageView::Eos(..) => break,
                MessageView::Error(e) => log::error!("GStreamer Error: {:?}", e),
                e => (),
            }
        }

        Ok(Self {
            underlying_file: source,
            pipeline,
            byte_offset: 0,
            header_data: None,
            stream: Stream::new(0xf01353),
            packet_num: 1,
            cached_page: None,
            wrote_page_header: 0,
            wrote_page_body: 0,
        })
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

    fn build_header_page(&mut self) -> Result<Page, EncoderError> {
        for packet_data in self.get_header_data()? {
            let mut packet = Packet::new(&packet_data);
            self.stream.packetin(&mut packet);
        }
        self.stream.flush().ok_or(EncoderError::NoStreamHeader)
    }

    fn get_header_data(&self) -> Result<Vec<Vec<u8>>, EncoderError> {
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
            headers.push(buf_map.to_owned())
        }
        Ok(headers)
    }

    fn get_next_page(&mut self) -> Result<Option<Page>, EncoderError> {
        while let Ok(sample) = self.get_sink()?.pull_sample() {
            let buf = sample.get_buffer().unwrap();
            let buf_map = buf.map_readable().unwrap();
            let mut packet = Packet::new(&buf_map);
            packet.set_packetno(self.packet_num as i64);
            self.packet_num += 1;
            if self.packet_num == 0 {
                packet.set_bos(1);
            }
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
                    opusenc.set_property_from_str("bandwidth", "narrowband");
                    rate_filter.set_property("caps", &caps).unwrap();
                    let sink = gst::ElementFactory::make("appsink", None)
                        .map_err(|e| EncoderError::from(e).maybe_set_element("appsink"))?;
                    sink.set_property_from_str("name", SINK_NAME);
                    println!(
                        "Sink: {:?}",
                        sink.get_property("name").unwrap().get::<String>().unwrap()
                    );
                    // println!(sink.get_prope)

                    let app_sink = sink.dynamic_cast::<gst_app::AppSink>().unwrap();
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

                    pipeline.set_state(gst::State::Playing)?;
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
        pipeline.set_state(gst::State::Playing)?;
        Ok(pipeline)
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
        if self.header_data.as_ref().is_none() {
            match self.build_header_page() {
                Ok(page) => {
                    let mut data = Vec::new();
                    data.write(&page.get_header()).unwrap();
                    data.write(&page.get_body()).unwrap();
                    self.header_data = Some(data);
                    println!("{:?}", self.header_data);
                }
                Err(e) => {
                    log::warn!("Error while reading: {}", e);
                    return Err(IoError::new(
                        IoErrorKind::Other,
                        format!("EncoderError: {}", e),
                    ));
                }
            }
        }
        let header_data = self.header_data.as_ref().unwrap();
        if self.byte_offset < header_data.len() {
            let wrote_header = buf.write(&header_data[self.byte_offset..])?;
            wrote += wrote_header;
            self.byte_offset += wrote_header;
        }
        if self.byte_offset >= header_data.len() {
            wrote += self.read_from_pages(&mut buf[wrote..])?;
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
            }
            if let Some(ref page) = self.cached_page {
                loop {
                    let wrote = buf.write(&page.get_header()[self.wrote_page_header..])?;
                    wrote_total += wrote;
                    self.wrote_page_header += wrote;
                    self.byte_offset += wrote;
                    if wrote == 0 && self.wrote_page_header == page.get_header().len() {
                        break;
                    } else if wrote == 0 {
                        return Ok(wrote_total);
                    }
                }
                loop {
                    let wrote = buf.write(&page.get_body()[self.wrote_page_body..])?;
                    wrote_total += wrote;
                    self.wrote_page_body += wrote;
                    self.byte_offset += wrote;
                    if wrote == 0 && self.wrote_page_body == page.get_body().len() {
                        // the entire page was written
                        self.cached_page = None;
                        break;
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

mod test {
    use super::OpusFile;
    use std::fs::File;
    use std::io::{Read, Write};

    #[test]
    fn read_header() {
        let mut opus_file = OpusFile::create("test-data/all.m4b".into()).unwrap();
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
        let mut opus_file_a = OpusFile::create("test-data/1.mp3".into()).unwrap();
        let mut out = File::create("/tmp/test.ogg").unwrap();
        let mut data_a = Vec::new();
        for _ in 0..100_000 {
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
        let mut opus_file_a = OpusFile::create("test-data/1.mp3".into()).unwrap();
        let mut opus_file_b = OpusFile::create("test-data/1.mp3".into()).unwrap();
        let mut data_a = Vec::new();
        let mut data_b = Vec::new();
        for _ in 0..100_000 {
            data_a.push(0);
            data_b.push(0);
        }

        loop {
            let read_a = opus_file_a.read(&mut data_a).unwrap();
            let read_b = opus_file_b.read(&mut data_b).unwrap();
            assert_eq!(read_a, read_b);
            for (i, (a, b)) in data_a.iter().zip(data_b.iter()).enumerate() {
                assert_eq!(a, b)
            }
            if read_a == 0 {
                break;
            }
        }
    }
}
