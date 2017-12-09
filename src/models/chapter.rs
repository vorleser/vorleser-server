use helpers::uuid::Uuid;
use diesel::prelude::*;
use models::audiobook::Audiobook;
use schema::chapters;

#[table_name="chapters"]
#[derive(Debug, Queryable, Associations, Identifiable, Serialize, Insertable)]
#[belongs_to(Audiobook)]
pub struct Chapter {
    pub id: Uuid,
    pub title: Option<String>,
    pub audiobook_id: Uuid,
    pub start_time: f64,
    pub number: i64
}
