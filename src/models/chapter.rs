use uuid::Uuid;
use diesel::prelude::*;
use models::audiobook::Audiobook;
use schema::chapters;

#[table_name="chapters"]
#[derive(Insertable)]
pub struct NewChapter {
    pub title: Option<String>,
    pub audiobook_id: Uuid,
    pub start_time: f64,
    pub number: i64
}

#[table_name="chapters"]
#[derive(Debug, Queryable, Associations, Identifiable, Serialize)]
#[belongs_to(Audiobook)]
pub struct Chapter {
    id: Uuid,
    title: Option<String>,
    audiobook_id: Uuid,
    start_time: f64,
    number: i64
}
