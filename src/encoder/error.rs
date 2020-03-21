extern crate gstreamer as gst;
use gst::glib::error::BoolError;
use gst::PadLinkError;

#[derive(Debug, Fail)]
pub enum EncoderError {
    #[fail(display = "Failed to create element {:?}", _0)]
    FailedToCreateElement(Option<&'static str>),
    #[fail(display = "GstreamerError: {}", _0)]
    GstreamerError(#[fail(cause)] BoolError),
    #[fail(display = "PadLinkError: {}", _0)]
    PadLinkError(#[fail(cause)] PadLinkError),
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
