extern crate diesel;
use std::io;
use std::io::Read;
use std::fs::File;
use std::ffi::{OsString, OsStr};
use std::path::Path;
use std::collections::HashMap;

use walkdir::{WalkDir, WalkDirIterator};
use walkdir;
use regex::Regex;
use diesel::prelude::*;
use worker::mediafile::MediaFile;
use worker::error::*;
use ring::digest;
use humanesort::HumaneOrder;

use ::helpers::db::Pool;
use ::models::library::*;
use ::models::audiobook::{Audiobook, NewAudiobook, Update};
use ::models::chapter::{NewChapter, Chapter};
use ::schema::audiobooks;
use ::schema::chapters;
use ::schema::libraries;
use worker::muxer;
use chrono::prelude::*;
use std::os::unix::prelude::*;
use diesel::BelongingToDsl;

pub struct Scanner {
    pub regex: Regex,
    pub library: Library,
    pub pool: Pool
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
    pub fn scan_library(&mut self) -> Result<()> {
        info!("Scanning library: {}", self.library.location);
        let last_scan = self.library.last_scan;
        self.library.last_scan = Some(UTC::now().naive_utc());
        let conn = &*self.pool.get().unwrap();
        let mut walker = WalkDir::new(&self.library.location).follow_links(true).into_iter();

        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => return Err(e.into()),
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
                        info!("No change data for files available, will hash everything.");
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
            };
            // TODO: we should just mark books deleted here, after all accidents where the
            // filesystem is gone for a bit should not lead to you loosing all playback data
            // we should also be able to recover from having the book set to deleted
            let mut deleted = 0;
            for book in Audiobook::belonging_to(&self.library).get_results::<Audiobook>(&*conn)? {
                if !Path::new(&(self.library.location.clone() + &book.location)).exists() {
                    let del = diesel::delete(
                            Audiobook::belonging_to(&self.library)
                            .filter(audiobooks::dsl::id.eq(book.id))
                        ).execute(&*conn)?;
                    match del {
                        0 => warn!("Could not delete audiobook, is something wrong with the DB?"),
                        1 => deleted += 1,
                        x => {
                            warn!("Deleted multiple audiobooks with same UUID, database integrity is compromised.");
                            deleted += x;
                        }
                    }
                }
            };
            info!("Deleted {} audiobooks because there files are no longer present.", deleted);
            ()
        };

        match diesel::update(libraries::dsl::libraries.filter(libraries::dsl::id.eq(self.library.id)))
            .set(&self.library)
            .execute(conn) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.into())
            }
    }

    fn process_audiobook(&self, path: &AsRef<Path>, conn: &diesel::pg::PgConnection) {
        if path.as_ref().is_dir() {
            match self.create_multifile_audiobook(conn, path) {
                Ok(_) => (),
                Err(e) => error_log!("Error: {}", e.description())
            };
        } else {
            match self.create_audiobook(conn, path) {
                Ok(_) => (),
                Err(e) => error_log!("Error: {}", e.description())
            };
        }
    }

    pub(super) fn create_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<()> {
        let relative_path = self.relative_path_str(path)?;
        let hash = checksum_file(path)?;

        let done = match Audiobook::update_path(&hash, &relative_path, conn)? {
            Update::Nothing => true,
            Update::Path => true,
            Update::NotFound => false
        };
        if done {
            return Ok(());
        };

        let file = match MediaFile::read_file(&path.as_ref()) {
            Ok(f) => f,
            Err(e) => return Err(e.into())
        };

        let metadata = file.get_mediainfo();
        let default_book = NewAudiobook {
            title: metadata.title,
            length: metadata.length,
            location: relative_path.to_owned(),
            library_id: self.library.id,
            hash: hash
        };

        let inserted = conn.transaction(|| -> Result<(Audiobook, usize)> {
            let book = Audiobook::ensure_exsits_in(&relative_path, &self.library, &default_book, &conn)?;
            book.delete_all_chapters(&conn);
            let chapters = file.get_chapters();
            let new_chapters: Vec<NewChapter> = chapters.iter().enumerate().map(|(i, chapter)| {
                NewChapter {
                    audiobook_id: book.id,
                    start_time: chapter.start,
                    title: chapter.title.clone().unwrap(),
                    number: i as i64
                }
            }).collect();
            Ok((book, diesel::insert(&new_chapters).into(chapters::table).execute(&*conn)?))
        });
        match inserted {
            Ok((b, num_chapters)) => {
                info!("Successfully saved book: {} with {} chapters.", b.title, num_chapters);
                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    fn relative_path_str<'a>(&'a self, path: &'a AsRef<Path>) -> Result<&'a str>{
        match path.as_ref().strip_prefix(&self.library.location).map(|p| p.to_str()) {
            Err(_) => Err(ErrorKind::Other("Path is not inside library.").into()),
            Ok(None) => Err(ErrorKind::Other("Path is not a valid utf-8 String.").into()),
            Ok(Some(p)) => Ok(p)
        }
    }

    pub(super) fn create_multifile_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<()> {
        // This might lead to inconsistent data as we hash before iterating over the files,
        // not better way to go about this seems possible to me
        // TODO: think about this
        let hash = checksum_dir(path)?;
        let relative_path = self.relative_path_str(path)?.to_owned();
        info!("Scanning multi-file audiobook at {:?}", path.as_ref());

        // if a book with the same hash exists in the database all we want to do is adjust the
        // path to retain all other information related to the book
        //
        // What happens if we have two exact same audiobooks in the library path?:
        // It should just keep switching the paths around whenever a file creation time is
        // updated which is not to bad.
        let done = match Audiobook::update_path(&hash, &relative_path, conn)? {
            Update::Nothing => true,
            Update::Path => true,
            Update::NotFound => false
        };
        if done {
            return Ok(());
        };

        let extension = match probable_audio_extension(&path) {
            Some(e) => e,
            None => return Err(ErrorKind::Other("No valid file extensions found.").into())
        };
        let walker = WalkDir::new(&path.as_ref())
            .follow_links(true)
            .sort_by(
                |s, o| s.to_string_lossy().humane_cmp(&o.to_string_lossy())
                );
        let mut all_chapters = Vec::new();
        let mut mediafiles = Vec::new();
        let mut start_time = 0.0;
        let title = match path.as_ref().file_name().map(|el| el.to_string_lossy()) {
            Some(s) => s.into_owned(),
            None => return Err(ErrorKind::InvalidUtf8.into())
        };

        let default_book = NewAudiobook {
            length: 0.0,
            library_id: self.library.id,
            location: relative_path.clone(),
            title: title,
            hash: hash
        };

        let inserted = conn.transaction(||  -> Result<()> {
            let book = Audiobook::ensure_exsits_in(&relative_path, &self.library, &default_book, &conn)?;
            book.delete_all_chapters(&conn);
            for (i, entry) in walker.into_iter().enumerate() {
                match entry {
                    Ok(file) => {
                        if file.path().is_dir() { continue };
                        match file.path().extension() {
                            Some(ext) => if ext != extension { continue },
                            None => { continue }
                        };
                        let media = match MediaFile::read_file(&file.path()) {
                            Ok(f) => {
                                if i == 0 {
                                    if let Some(new_title) = f.get_mediainfo().metadata.get("album") {
                                        diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::id.eq(book.id)))
                                            .set(audiobooks::dsl::title.eq(new_title)).execute(conn)?;
                                    }
                                };
                                let info = f.get_mediainfo();
                                let new_chapter = NewChapter {
                                    title: info.title,
                                    start_time: start_time,
                                    audiobook_id: book.id,
                                    number: i as i64
                                };
                                diesel::insert(&new_chapter).into(chapters::table).execute(conn)?;
                                start_time += info.length;
                                all_chapters.push(new_chapter);
                                f
                            }
                            Err(e) => return Err(e.into())
                        };
                        mediafiles.push(media)
                    },
                    Err(e) => return Err(e.into())
                };
            };
            diesel::update(Audiobook::belonging_to(&self.library).filter(audiobooks::dsl::id.eq(book.id)))
                .set(audiobooks::dsl::length.eq(start_time)).execute(conn)?;
            muxer::merge_files(
                &("data/".to_string() + &book.id.hyphenated().to_string() + &extension.to_string_lossy().into_owned()),
                &mediafiles
                )?;
            Ok(())
        });
        match inserted {
            Ok(_) => {
                info!("Successfully saved book");
                Ok(())
            },
            Err(e) => Err(e)
        }
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}

pub fn checksum_file(path: &AsRef<Path>) -> Result<Vec<u8>> {
    let mut ctx = digest::Context::new(&digest::SHA256);
    update_hash_from_file(&mut ctx, path)?;
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}

fn update_hash_from_file(ctx: &mut digest::Context, path: &AsRef<Path>) -> Result<()> {
    let mut file = File::open(path.as_ref())?;
    let mut buf: [u8; 1024] = [0; 1024];
    loop {
        let count = file.read(&mut buf[..])?;
        ctx.update(&buf[0..count]);
        if count == 0 { break }
    }
    Ok(())
}

///
/// Returns the largest changed time stamp on any file in a given directory
///
fn most_recent_change(path: &AsRef<Path>) -> Result<Option<NaiveDateTime>> {
    // this is a suboptimal solution it doesn't really matter here but creating a vector is not
    // great.
    let times: Result<Vec<NaiveDateTime>> = WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .map(|el| -> Result<NaiveDateTime> {
            match el {
                Ok(f) => {
                    match f.metadata().map(|el| NaiveDateTime::from_timestamp(el.mtime(), el.mtime_nsec() as u32)) {
                        Ok(modified) => return Ok(modified),
                        Err(e) => return Err(e.into())
                    };
                },
                Err(e) => return Err(e.into())
            }
        })
    .collect();
    Ok(times?.iter().max().cloned())
}


/// Find the most common extension in a directory that might be an audio file.
pub(super) fn probable_audio_extension(path: &AsRef<Path>) -> Option<OsString> {
    // TODO: discard files that don't look like media files
    let mut counts: HashMap<OsString, usize> = HashMap::new();
    for el in WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .filter_map(|opt| {
            match opt.map(|wd| wd.path().extension().map(|el| el.to_owned())) {
                Ok(Some(o)) => Some(o),
                _ => None,
            }
        }) {
        let mut count = counts.entry(el).or_insert(0);
        *count += 1;
    };
    let mut extensions: Vec<(OsString, usize)> = counts.drain().collect();
    extensions.sort_by(|&(_, v1), &(_, v2)| v2.cmp(&v1));
    extensions.pop().map(|el| el.0)
}

pub fn checksum_dir(path: &AsRef<Path>) -> Result<Vec<u8>> {
    let walker = WalkDir::new(path.as_ref())
        .follow_links(true)
        .sort_by(
            |s, o| s.to_string_lossy().humane_cmp(&o.to_string_lossy())
            );
    let mut ctx = digest::Context::new(&digest::SHA256);
    for entry in walker {
        if let Ok(e) = entry {
                let p = e.path();
                if e.file_type().is_file() {
                    update_hash_from_file(&mut ctx, &p)?;
                }
                ctx.update(p.to_string_lossy().as_bytes());
        }
    }
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}
