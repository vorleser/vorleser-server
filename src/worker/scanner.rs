extern crate diesel;
use walkdir::{WalkDir, WalkDirIterator};
use walkdir;
use humanesort::humane_order;
use regex::Regex;

use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use diesel::prelude::*;
use worker::mediafile::MediaFile;
use worker::error::*;
use ring::digest;
use ::helpers::db::{Pool, PooledConnection};
use ::models::library::*;
use ::models::audiobook::{Audiobook, NewAudiobook};
use ::models::chapter::NewChapter;
use ::schema::audiobooks;
use ::schema::chapters;
use ::schema::libraries;
use std::time::SystemTime;

pub struct Scanner {
    pub regex: Regex,
    pub library: Library,
    pub pool: Pool
}

quick_error! {
    #[derive(Debug)]
    pub enum ScannError {
        Io(err: io::Error) {}
        WalkDir(err: walkdir::Error) {}
        MediaError(err: MediaError) {}
    }
}

impl Scanner {
    pub fn new(conn_pool: Pool, library: Library) -> Self {
        Self {
            regex: Regex::new(library.is_audiobook_regex.as_str()).expect("Invalid Regex!"),
            library: library,
            pool: conn_pool
        }
    }

    // for all existing audiobooks
    // check hashes, if changed, remove book and create new with new data
    // if hashes have not changed: check symlinked/remuxed files still there? if not re-link/mux
    pub fn scan_library(&mut self) -> Result<(), ScannError> {
        //todo: it might be nice to check for file changed data and only check new files
        println!("Scanning library: {}", self.library.location);
        let last_scan = self.library.last_scan;
        self.library.last_scan = Some(SystemTime::now());
        let mut walker = WalkDir::new(&self.library.location).follow_links(true).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => return Err(ScannError::WalkDir(e)),
                Some(Ok(i)) => i,
            };
            let path = entry.path().strip_prefix(&self.library.location).unwrap();
            if path.components().count() == 0 { continue };
            if is_audiobook(path, &self.regex) {
                let should_scan = match most_recent_change(&self.library.location) {
                    Err(e) => return Err(e),
                    Ok(Some(time)) => if let Some(last_scan_time) = last_scan {
                        time >= last_scan_time
                    } else {
                        // if there was no scan before we should scan now
                        true
                    },
                    Ok(None) => {
                        println!("No change data for files available, will hash everything.");
                        true
                    }
                };
                if should_scan {
                    self.process_audiobook(&path);
                    println!("Processed: {:?}", path);
                }
                // If it is an audiobook we don't continue searching deeper in the dir tree here
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            }
        };
        diesel::update(libraries::dsl::libraries.filter(libraries::dsl::id.eq(self.library.id)))
               .set(&self.library)
               .execute(&*self.pool.get().unwrap());
        Ok(())
    }

    fn process_audiobook(&self, path: &AsRef<Path>) {
        unimplemented!();
        if path.as_ref().is_dir() {
            self.create_multifile_audiobook(self.pool.get().unwrap(), path.as_ref());
        } else {
            self.create_audiobook(self.pool.get().unwrap(), path);
        }
    }

    pub(super) fn create_audiobook(&self, conn: PooledConnection, path: &AsRef<Path>) -> Result<(), ScannError> {
        let file = match MediaFile::read_file(&path.as_ref()) {
            Ok(f) => f,
            Err(e) => return Err(ScannError::MediaError(e))
        };
        conn.transaction(|| -> Result<(), diesel::result::Error> {
            let md = file.get_mediainfo();
            let new_book = NewAudiobook {
                title: md.title,
                length: md.length,
                location: path.as_ref().to_str().unwrap().to_owned(),
                library_id: self.library.id
            };
            let books = diesel::insert(&new_book).into(audiobooks::table).get_results::<Audiobook>(&*conn).unwrap();
            let book = books.first().unwrap();
            let chapters = file.get_chapters();
            let new_chapters: Vec<NewChapter> = chapters.iter().enumerate().map(move |(i, chapter)| {
                NewChapter {
                    audiobook_id: book.id,
                    start_time: chapter.start,
                    title: chapter.title.clone().unwrap(),
                    number: i as i64
                }
            }).collect();
            let suc = diesel::insert(&new_chapters).into(chapters::table).execute(&*conn).unwrap();
            return Ok(())
        });
        Ok(())
    }

    pub(super) fn create_multifile_audiobook(&self, conn: PooledConnection, path: &Path) -> Result<(), MediaError> {
        println!("Stub for creating audiobook from dir");
        Ok(())
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}



pub fn checksum_file(path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut ctx = digest::Context::new(&digest::SHA256);
    update_hash_from_file(&mut ctx, path)?;
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}

fn update_hash_from_file(ctx: &mut digest::Context, path: &Path) -> Result<(), io::Error> {
    let file = File::open(path)?;
    for b in file.bytes() {
        ctx.update(&[b?]);
    }
    Ok(())
}

///
/// Returns the largest changed time stamp on any file in a given directory
///
fn most_recent_change(path: &AsRef<Path>) -> Result<Option<SystemTime>, ScannError> {
    // this is a suboptimal solution it doesn't really matter here but creating a vector is not
    // great.
    let times: Result<Vec<SystemTime>, _> = WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .map(|el| -> Result<SystemTime, ScannError> {
            match el {
                Ok(f) => {
                    match f.metadata().map(|el| el.modified()) {
                        Ok(Ok(modified)) => return Ok(modified),
                        Ok(Err(e)) => return Err(ScannError::Io(e)),
                        Err(e) => return Err(ScannError::WalkDir(e))
                    };
                },
                Err(e) => return Err(ScannError::WalkDir(e))
            }
        })
        .collect();
    Ok(times?.iter().max().map(|e| e.clone()))
}

pub fn checksum_dir(path: &Path) -> Result<Vec<u8>, io::Error> {
    let walker = WalkDir::new(path)
        .follow_links(true)
        .sort_by(
            |s, o| humane_order(s.to_string_lossy(), o.to_string_lossy())
            );
    let mut ctx = digest::Context::new(&digest::SHA256);
    for entry in walker {
        match entry {
            Ok(e) => {
                let p = e.path();
                if e.file_type().is_file() {
                    update_hash_from_file(&mut ctx, p)?;
                }
                ctx.update(p.to_string_lossy().as_bytes());
            }
            _ => ()
        }
    }
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}
