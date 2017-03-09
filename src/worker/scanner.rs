extern crate diesel;
use walkdir::{WalkDir, WalkDirIterator};
use regex::Regex;

use std::path::{Path, PathBuf};
use diesel::prelude::*;
use diesel::*;
use worker::mediafile::MediaFile;
use worker::error::*;
use ::helpers::db::{Pool, PooledConnection};
use ::models::audiobook::{Audiobook, NewAudiobook};
use ::models::chapter::{Chapter, NewChapter};
use ::schema::audiobooks;
use ::schema::chapters;

struct Scanner {
    regex: Regex,
    path: PathBuf,
    pool: Pool
}

impl Scanner {
    pub fn new(conn_pool: Pool, root: PathBuf) -> Self {
        Self {
            regex: Regex::new("^[^/]+$").expect("Invalid Regex!"),
            path: root,
            pool: conn_pool
        }
    }

    pub fn scan_library(&self) {
        //todo: it might be nice to check for file changed data and only check new files
        println!("Scanning library.");
        let mut walker = WalkDir::new(&self.path).follow_links(true).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => panic!("Error: {}", e),
                Some(Ok(i)) => i,
            };
            let path = entry.path().strip_prefix(&self.path).unwrap();
            if path.components().count() == 0 { continue };
            if is_audiobook(path, &self.regex) {
                process_audiobook(path, self.pool.get().unwrap());
                println!("{:?}", path);
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            }
        }
    }

    pub fn launch_scan_thread() {
        unimplemented!();
    }
}

fn process_audiobook(path: &Path, conn: PooledConnection) {
    if path.is_dir() {
        // handle multfile audiobook
    } else {
        // handle single file audiobook
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}

fn create_multifile_audiobook(path: &Path) -> Result<(), MediaError> {
    println!("Creating audiobook from dir");
    Ok(())
}

pub(super) fn create_audiobook(conn: PooledConnection, path: &Path) -> Result<(), MediaError> {
    let file = try!(MediaFile::read_file(path));
    let md = file.get_mediainfo();
    let new_book = NewAudiobook {
        title: md.title,
        length: md.length
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
    Ok(())
}
