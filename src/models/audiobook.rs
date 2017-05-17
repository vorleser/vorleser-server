use uuid::Uuid;
use diesel;
use diesel::prelude::*;
use schema::audiobooks;
use schema::playstates;
use std::path::Path;
use ::models::library::Library;
use ::models::chapter::Chapter;
use chrono::NaiveDateTime;

#[table_name="audiobooks"]
#[derive(Insertable)]
pub struct NewAudiobook {
    pub title: String,
    pub location: String,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>
}

#[table_name="audiobooks"]
#[derive(Debug, Queryable, AsChangeset, Associations, Identifiable)]
#[hasmany(chapters)]
#[belongs_to(Library)]
pub struct Audiobook {
    pub id: Uuid,
    pub title: String,
    pub location: String,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>
}

pub enum Update {
    Nothing,
    Path,
    NotFound
}

impl Audiobook {
    fn find_by_hash(hash: &[u8], conn: &diesel::pg::PgConnection) -> Result<Audiobook, diesel::result::Error> {
        audiobooks::dsl::audiobooks.filter(audiobooks::dsl::hash.eq(hash)).get_result(conn)
    }

    /// Updates the path of any book with the given hash to the new_path provided.
    /// Returns true if a path is now correct, returns false if no book with this hash exists.
    pub fn update_path(book_hash: &[u8], new_path: &AsRef<str>, conn: &diesel::pg::PgConnection) -> Result<Update, diesel::result::Error> {
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
}

#[derive(Insertable, Queryable, AsChangeset, Deserialize)]
#[table_name="playstates"]
pub struct Playstate {
    pub position: f64,
    pub completed: bool,
    pub user_id: Uuid,
    pub audiobook_id: Uuid,
    pub timestamp: NaiveDateTime
}
