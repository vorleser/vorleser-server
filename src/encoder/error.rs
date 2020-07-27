extern crate gstreamer as gst;
use gst::glib::error::BoolError;
use gst::structure::GetError;
use gst::{PadLinkError, StateChangeError};

#[derive(Debug, Fail)]
pub enum EncoderError {
    #[fail(display = "Failed to create element {:?}", _0)]
    FailedToCreateElement(Option<&'static str>),
    #[fail(display = "GStreamer Error: {}", _0)]
    GstreamerError(#[fail(cause)] BoolError),
    #[fail(display = "GStreamer PadLinkError: {}", _0)]
    PadLinkError(#[fail(cause)] PadLinkError),
    #[fail(display = "GStreamer StateChangeError: {}", _0)]
    StateChangeError(#[fail(cause)] StateChangeError),
    #[fail(display = "Invalid State: {}", _0)]
    InvalidState(&'static str),
    #[fail(display = "Unable to get value {}", _0)]
    GetError(String),
    #[fail(display = "Stream header missing")]
    NoStreamHeader,
    #[fail(display = "Invalid media file")]
    InvalidMediaFile,
}

impl From<BoolError> for EncoderError {
    fn from(err: BoolError) -> Self {
        if err.message == "Failed to create element from factory name" {
            Self::FailedToCreateElement(None)
        } else {
            Self::GstreamerError(err)
        }
    }
}

impl From<StateChangeError> for EncoderError {
    fn from(err: StateChangeError) -> Self {
        Self::StateChangeError(err)
    }
}

impl From<GetError<'_>> for EncoderError {
    fn from(err: GetError) -> Self {
        Self::GetError(format!("{}", err))
    }
}

impl From<PadLinkError> for EncoderError {
    fn from(err: PadLinkError) -> Self {
        Self::PadLinkError(err)
    }
}

impl EncoderError {
    pub fn maybe_set_element(self, element: &'static str) -> Self {
        match self {
            Self::FailedToCreateElement(_) => Self::FailedToCreateElement(Some(element)),
            s => s,
        }
    }
}
