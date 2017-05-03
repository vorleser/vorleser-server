extern crate diesel;
use walkdir::{WalkDir, WalkDirIterator};
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

struct Scanner {
    regex: Regex,
    library: Library,
    pool: Pool
}

impl Scanner {
    pub fn new(conn_pool: Pool, library: Library) -> Self {
        Self {
            regex: Regex::new(library.is_audiobook_regex.as_str()).expect("Invalid Regex!"),
            library: library,
            pool: conn_pool
        }
    }

    pub fn scan_library(&self) -> Result<(), ()> {
        //todo: it might be nice to check for file changed data and only check new files
        println!("Scanning library.");
        let mut walker = WalkDir::new(&self.library.location.as_str()).follow_links(true).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => return Err(()),
                Some(Ok(i)) => i,
            };
            let path = entry.path().strip_prefix(&self.library.location).unwrap();
            if path.components().count() == 0 { continue };
            if is_audiobook(path, &self.regex) {
                self.process_audiobook(path);
                println!("{:?}", path);
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            }
        };
        Ok(())
    }

    fn process_audiobook(&self, path: &Path) {
        unimplemented!();
        if path.is_dir() {
            self.create_multifile_audiobook(self.pool.get().unwrap(), path);
        } else {
            self.create_audiobook(self.pool.get().unwrap(), path);
        }
    }

    pub(super) fn create_audiobook(&self, conn: PooledConnection, path: &Path) -> Result<(), MediaError> {
        let file = try!(MediaFile::read_file(path));
        conn.transaction(|| -> Result<(), diesel::result::Error> {
            let md = file.get_mediainfo();
            let new_book = NewAudiobook {
                title: md.title,
                length: md.length,
                location: path.to_str().unwrap().to_owned(),
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
