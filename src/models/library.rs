use uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel::prelude::*;
use schema::{libraries, audiobooks};
use models::audiobook::Audiobook;

#[table_name="libraries"]
#[derive(Insertable)]
pub struct NewLibrary {
    pub location: String,
    pub is_audiobook_regex: String
}

#[table_name="libraries"]
#[derive(Debug, Queryable, AsChangeset, Serialize, Associations, Identifiable)]
#[has_many(audiobooks)]
pub struct Library {
    pub id: Uuid,
    pub content_change_date: NaiveDateTime,
    pub location: String,
    pub is_audiobook_regex: String,
    pub last_scan: Option<NaiveDateTime>
}
