use uuid::Uuid;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use schema::libraries;

#[table_name="libraries"]
#[derive(Insertable)]
pub struct NewLibrary {
    pub location: String,
    pub is_audiobook_regex: String
}

#[table_name="libraries"]
#[derive(Debug, Queryable)]
pub struct Library {
    pub id: Uuid,
    pub content_change_date: NaiveDateTime,
    pub location: String,
    pub is_audiobook_regex: String
}
