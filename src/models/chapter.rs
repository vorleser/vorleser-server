use uuid::Uuid;
use diesel::prelude::*;
use schema::chapters;

#[table_name="chapters"]
#[derive(Insertable)]
pub struct NewChapter {
    pub title: String,
    pub start_time: f64,
    pub audiobook_id: Uuid,
    pub number: i64
}

#[derive(Debug, Queryable)]
#[belongs_to(Audiobook)]
pub struct Chapter {
    id: Uuid,
    book_id: Uuid,
    title: String,
    number: i64,
    start_time: f64
}
