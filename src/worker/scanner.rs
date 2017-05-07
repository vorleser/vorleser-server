extern crate diesel;
use walkdir::{WalkDir, WalkDirIterator};
use walkdir;
use humanesort::humane_order;
use regex::Regex;

use std::io;
use std::io::Read;
use std::fs::File;
use std::error::Error;
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
        Io(err: io::Error) {
            description(err.description())
        }
        Db(err: diesel::result::Error) {
            from()
                description(err.description())
        }
        WalkDir(err: walkdir::Error) {
            description(err.description())
        }
        MediaError(err: MediaError) {
            description(err.description())
        }
        InvalidUnicode(err: ()) {
        }
        Other(descr: &'static str) {
            description(descr)
        }
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
        println!("Scanning library: {}", self.library.location);
        let last_scan = self.library.last_scan;
        self.library.last_scan = Some(SystemTime::now());
        let conn = &*self.pool.get().unwrap();
        let mut walker = WalkDir::new(&self.library.location).follow_links(true).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => return Err(ScannError::WalkDir(e)),
                Some(Ok(i)) => i,
            };
            let path = entry.path();
            let relative_path = entry.path().strip_prefix(&self.library.location).unwrap();
            if relative_path.components().count() == 0 { continue };
            if is_audiobook(relative_path, &self.regex) {
                let should_scan = match most_recent_change(&path) {
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
                    self.process_audiobook(&path, conn);
                }
                // If it is an audiobook we don't continue searching deeper in the dir tree here
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            }
        };
        match diesel::update(libraries::dsl::libraries.filter(libraries::dsl::id.eq(self.library.id)))
            .set(&self.library)
            .execute(conn) {
                Ok(_) => Ok(()),
                Err(e) => Err(ScannError::Db(e))
            }
    }

    fn process_audiobook(&self, path: &AsRef<Path>, conn: &diesel::pg::PgConnection) {
        if path.as_ref().is_dir() {
            self.create_multifile_audiobook(conn, path);
        } else {
            match self.create_audiobook(conn, path) {
                Ok(_) => (),
                Err(e) => println!("Error: {}", e.description())
            };
        }
    }

    pub(super) fn create_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<(), ScannError> {
        let file = match MediaFile::read_file(&path.as_ref()) {
            Ok(f) => f,
            Err(e) => return Err(ScannError::MediaError(e))
        };
        let relative_path = self.relative_path_str(path)?;
        let metadata = file.get_mediainfo();
        let new_book = NewAudiobook {
            title: metadata.title,
            length: metadata.length,
            location: relative_path.to_owned(),
            library_id: self.library.id
        };
        let inserted = conn.transaction(|| -> Result<usize, diesel::result::Error> {
            let books = diesel::insert(&new_book).into(audiobooks::table).get_results::<Audiobook>(&*conn)?;
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
            diesel::insert(&new_chapters).into(chapters::table).execute(&*conn)
        });
        match inserted {
            Ok(_) => {
                println!("Sucessfully saved book: {}", new_book.title);
                Ok(())
            },
            Err(e) => Err(ScannError::Db(e))
        }
    }

    fn relative_path_str<'a>(&'a self, path: &'a AsRef<Path>) -> Result<&'a str, ScannError>{
        match path.as_ref().strip_prefix(&self.library.location).map(|p| p.to_str()) {
            Err(_) => Err(ScannError::Other("Path is not inside library.")),
            Ok(None) => Err(ScannError::Other("Path is not a valid utf-8 String.")),
            Ok(Some(p)) => Ok(p)
        }
    }
    pub(super) fn create_multifile_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<(), ScannError> {
        // for now each file will be a chapter, maybe in the future we want to use chapter
        // metadata if it is present.
        // TODO: build new file
        let hash = checksum_dir(path);
        let walker = WalkDir::new(&path.as_ref())
            .follow_links(true)
            .sort_by(
                |s, o| humane_order(s.to_string_lossy(), o.to_string_lossy())
                );
        let mut all_chapters = Vec::new();
        let mut start_time = 0.0;
        let title = match path.as_ref().file_name().map(|el| el.to_string_lossy()) {
            Some(s) => s.into_owned(),
            None => return Err(ScannError::InvalidUnicode(()))
        };
        conn.transaction(|| {
            let new_book = NewAudiobook {
                length: 0.0,
                library_id: self.library.id,
                location: self.relative_path_str(path)?.to_owned(),
                title: title
            };
            let books = diesel::insert(&new_book).into(audiobooks::table).get_results::<Audiobook>(&*conn)?;
            let book = books.first().unwrap();
            for (i, entry) in walker.into_iter().enumerate() {
                match entry {
                    Ok(file_path) => {
                        match MediaFile::read_file(&file_path.path()) {
                            Ok(f) => {
                                if i == 0 {
                                    if let Some(new_title) = f.get_mediainfo().metadata.get("album") {
                                        diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::id.eq(book.id)))
                                            .set(audiobooks::dsl::title.eq(new_title));
                                    }
                                };
                                let info = f.get_mediainfo();
                                let new_chapter = NewChapter {
                                    title: info.title,
                                    start_time: start_time,
                                    audiobook_id: book.id,
                                    number: i as i64
                                };
                                diesel::insert(&new_chapter).into(chapters::table).execute(&*conn)?;
                                start_time += info.length;
                                all_chapters.push(new_chapter)
                            }
                            Err(e) => return Err(ScannError::MediaError(e))
                        }
                    },
                    Err(e) => return Err(ScannError::WalkDir(e))
                };
            };
            diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::id.eq(book.id)))
                .set(audiobooks::dsl::length.eq(start_time));
            Ok(())
        })
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

pub fn checksum_dir(path: &AsRef<Path>) -> Result<Vec<u8>, io::Error> {
    let walker = WalkDir::new(path.as_ref())
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
