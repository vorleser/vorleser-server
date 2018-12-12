use crate::helpers::uuid::Uuid;
use diesel::prelude::*;
use crate::models::audiobook::Audiobook;
use crate::schema::chapters;

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
