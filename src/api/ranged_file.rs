use std::fs::File;
use std::path::{Path, PathBuf};
use std::io;
use std::ops::{Deref, DerefMut};

use rocket::request::Request;
use rocket::response::{Response, Responder, Body};
use rocket::http::{Status, ContentType};
use rocket::http::hyper::header::{Range, ByteRangeSpec, AcceptRanges, RangeUnit, ContentLength, ContentRange, ContentRangeSpec};
use rocket::http::hyper::header::Range::Bytes;
use rocket::http::hyper::header::ByteRangeSpec::*;
use std::fs::metadata;
use std::io::{Seek, SeekFrom, Read};

/// A file with an associated name; responds with the Content-Type based on the
/// file extension.
#[derive(Debug)]
pub struct RangedFile(PathBuf, File);

impl RangedFile {
    /// Attempts to open a file in read-only mode.
    ///
    /// # Errors
    ///
    /// This function will return an error if path does not already exist. Other
    /// errors may also be returned according to
    /// [OpenOptions::open](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html#method.open).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rocket::response::RangedFile;
    ///
    /// # #[allow(unused_variables)]
    /// let file = RangedFile::open("foo.txt");
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<RangedFile> {
        let file = File::open(path.as_ref())?;
        Ok(RangedFile(path.as_ref().to_path_buf(), file))
    }

    /// Retrieve the underlying `File`.
    #[inline(always)]
    pub fn file(&self) -> &File {
        &self.1
    }

    /// Take the underlying `File`.
    #[inline(always)]
    pub fn take_file(self) -> File {
        self.1
    }

    /// Retrieve a mutable borrow to the underlying `File`.
    #[inline(always)]
    pub fn file_mut(&mut self) -> &mut File {
        &mut self.1
    }

    /// Retrieve the path of this file.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::io;
    /// use rocket::response::RangedFile;
    ///
    /// # #[allow(dead_code)]
    /// # fn demo_path() -> io::Result<()> {
    /// let file = RangedFile::open("foo.txt")?;
    /// assert_eq!(file.path().as_os_str(), "foo.txt");
    /// # Ok(())
    /// # }
    /// ```
    #[inline(always)]
    pub fn path(&self) -> &Path {
        self.0.as_path()
    }
}

/// Streams the named file to the client. Sets or overrides the Content-Type in
/// the response according to the file's extension if the extension is
/// recognized. See
/// [ContentType::from_extension](/rocket/http/struct.ContentType.html#method.from_extension)
/// for more information. If you would like to stream a file with a different
/// Content-Type than that implied by its extension, use a `File` directly.
impl Responder<'static> for RangedFile {
    fn respond_to(self, req: &Request) -> Result<Response<'static>, Status> {
        let mut response = Response::new();
        // if let Some(ext) = self.path().extension() {
        //     // TODO: Use Cow for lowercase.
        //     let ext_string = ext.to_string_lossy().to_lowercase();
        //     if let Some(content_type) = ContentType::from_extension(&ext_string) {
        //         response.set_header(content_type);
        //     }
        // }
        // hard-coded for testing, rocket doesn't have an mp3 content-type
        response.set_header(ContentType::new("audio", "mpeg"));

        let meta = metadata(self.path()).unwrap();
        let size = meta.len();
        response.set_header(AcceptRanges(vec![RangeUnit::Bytes]));

        if let Some(range) = req.headers().get_one("Range") {
            let r: Range = range.parse().unwrap();
            println!("{:?}", r);
            match r {
                Bytes(vec) => {
                    let spec = &vec[0];
                    let mut f = self.take_file();
                    match *spec {
                        FromTo(from, to) => {
                            let first_byte_not_sent = to + 1;
                            f.seek(SeekFrom::Start(from));
                            let body = Body::Sized(f.take(first_byte_not_sent - from), first_byte_not_sent - from);
                            let result_spec = ContentRangeSpec::Bytes{
                                range: Some((from, to)),
                                instance_length: Some(size)
                            };
                            response.set_header(ContentRange(result_spec));
                            response.set_raw_body(body);
                        }
                        AllFrom(from) => {
                            f.seek(SeekFrom::Start(from));
                            let body = Body::Sized(f, meta.len() - from);
                            let result_spec = ContentRangeSpec::Bytes{
                                range: Some((from, size - 1)),
                                instance_length: Some(size)
                            };
                            response.set_header(ContentRange(result_spec));
                            response.set_raw_body(body);
                        }
                        Last(n) => {
                            f.seek(SeekFrom::End(-(n as i64)));
                            let body = Body::Sized(f, n);
                            let result_spec = ContentRangeSpec::Bytes{
                                range: Some((size - n, size - 1)),
                                instance_length: Some(size)
                            };
                            response.set_header(ContentRange(result_spec));
                            response.set_raw_body(body);
                        }
                    };
                    response.set_status(Status::PartialContent);
                }
                _ => unreachable!("can't deal with non-byte ranges")
            }
        } else {
            response.set_sized_body(self.take_file());
        }

        Ok(response)
    }
}
