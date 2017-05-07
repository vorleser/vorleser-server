use uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel::prelude::*;
use schema::libraries;

#[table_name="libraries"]
#[derive(Insertable)]
pub struct NewLibrary {
    pub location: String,
    pub is_audiobook_regex: String,
    pub last_scan: Option<SystemTime>
}

#[table_name="libraries"]
#[derive(Debug, Queryable, AsChangeset)]
pub struct Library {
    pub id: Uuid,
    pub content_change_date: NaiveDateTime,
    pub location: String,
    pub is_audiobook_regex: String,
    pub last_scan: Option<SystemTime>
}
