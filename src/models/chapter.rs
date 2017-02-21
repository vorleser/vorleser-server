use diesel::prelude::*;
use schema::chapters;

#[derive(Insertable)]
#[table_name="chapters"]
struct NewChapter {
    pub length: f64,
    pub tilte: String,
    pub book: Uuid
}

#[derive(Debug, Queryable)]
#[belongs_to(Audiobook)]
struct Chatper {
    id: Uuid,
    book_id: Uuid,
    title: String,
    length: f64
}
