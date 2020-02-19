use diesel;
use diesel::prelude::*;
use std::path::Path;
use diesel::result::Error;
use chrono::NaiveDateTime;
use diesel::sqlite::SqliteConnection;
use crate::models::user::User;
use crate::helpers::uuid::Uuid;

use crate::models::library::Library;
use crate::models::chapter::Chapter;
use crate::schema::{audiobooks, playstates, library_permissions};

#[table_name="audiobooks"]
#[derive(PartialEq, Debug, Queryable, AsChangeset, Associations, Identifiable, Serialize, Clone,
         Insertable)]
#[belongs_to(Library)]
pub struct Audiobook {
    pub id: Uuid,
    pub location: String,
    pub title: String,
    pub artist: Option<String>,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>,
    pub file_extension: String,
    pub deleted: bool
}

pub enum Update {
    Nothing,
    Path,
    NotFound
}

impl Audiobook {
    fn find_by_hash(hash: &[u8], conn: &diesel::sqlite::SqliteConnection) -> Result<Audiobook, diesel::result::Error> {
        audiobooks::dsl::audiobooks.filter(audiobooks::dsl::hash.eq(hash)).get_result(conn)
    }

    /// Updates the path of any book with the given hash to the new_path provided.
    /// Returns true if a path is now correct, returns false if no book with this hash exists.
    pub fn update_path(book_hash: &[u8], new_path: &AsRef<str>, conn: &diesel::sqlite::SqliteConnection)
        -> Result<Update, diesel::result::Error> {
        if let Ok(book) = Self::find_by_hash(book_hash, conn) {
            if book.location != new_path.as_ref() {
                diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::hash.eq(book_hash)))
                    .set(audiobooks::dsl::location.eq(new_path.as_ref())).execute(conn)?;
                return Ok(Update::Path)
            };
            return Ok(Update::Nothing)
        } else {
            return Ok(Update::NotFound);
        }
    }

    pub fn delete_all_chapters(&self, conn: &diesel::sqlite::SqliteConnection) -> diesel::result::QueryResult<usize> {
        diesel::delete(Chapter::belonging_to(self)).execute(&*conn)
    }

    pub fn ensure_exists_in(relative_path: &AsRef<str>, library: &Library,
                            new_book: &Audiobook, conn: &SqliteConnection)
        -> Result<Audiobook, diesel::result::Error> {
        match Self::belonging_to(library)
            .filter(audiobooks::dsl::location.eq(relative_path.as_ref()))
            .first::<Audiobook>(&*conn)
            .optional()? {
                Some(b) => {
                    let mut updated = new_book.clone();
                    updated.id = b.id;
                    diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::id.eq(&b.id))).set(&updated).execute(conn)?;
                    Ok(updated)
                },
                None => {
                    diesel::insert_into(audiobooks::table).values(new_book).execute(conn);
                    Ok(new_book.clone())
                }
            }
    }
}
