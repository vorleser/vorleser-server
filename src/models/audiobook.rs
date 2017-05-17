use uuid::Uuid;
use diesel;
use diesel::prelude::*;
use schema::audiobooks;
use schema::playstates;
use std::path::Path;
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

#[derive(Debug, Queryable, AsChangeset)]
#[hasmany(chapters)]
#[belongs_to(Library)]
#[table_name="audiobooks"]
pub struct Audiobook {
    pub id: Uuid,
    pub title: String,
    pub location: String,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>
}

impl Audiobook {
    fn find_by_hash(hash: &[u8], conn: &diesel::pg::PgConnection) -> Result<Audiobook, diesel::result::Error> {
        audiobooks::dsl::audiobooks.filter(audiobooks::dsl::hash.eq(hash)).get_result(conn)
    }

    /// Updates the path of any book with the given hash to the new_path provided.
    /// Returns true if a path is now correct, returns false if no book with this hash exists.
    pub fn update_path_for_hash(hash: &[u8], new_path: &AsRef<str>, conn: &diesel::pg::PgConnection) -> Result<bool, diesel::result::Error> {
        println!("Updating Path");
        if let Ok(book) = Self::find_by_hash(hash, conn) {
            if book.location != new_path.as_ref() {
                diesel::update(audiobooks::dsl::audiobooks.filter(audiobooks::dsl::hash.eq(hash)))
                    .set(audiobooks::dsl::location.eq(new_path.as_ref())).execute(conn)?;
            };
            return Ok(true)
        } else {
            return Ok(false);
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
