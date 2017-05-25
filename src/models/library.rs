use uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel::prelude::*;
use schema::{libraries, audiobooks, library_permissions};
use models::audiobook::Audiobook;

#[table_name="libraries"]
#[derive(Insertable)]
pub struct NewLibrary {
    pub location: String,
    pub is_audiobook_regex: String
}

#[table_name="libraries"]
#[derive(Debug, Clone, Queryable, AsChangeset, Serialize, Associations, Identifiable)]
#[has_many(audiobooks, library_permissions)]
pub struct Library {
    pub id: Uuid,
    pub content_change_date: NaiveDateTime,
    pub location: String,
    pub is_audiobook_regex: String,
    pub last_scan: Option<NaiveDateTime>
}

#[table_name="library_permissions"]
#[primary_key(library_id, user_id)]
#[derive(Debug, Clone, Queryable, AsChangeset, Serialize, Associations, Identifiable)]
pub struct LibraryAccess {
    pub library_id: Uuid,
    pub user_id: Uuid,
}
