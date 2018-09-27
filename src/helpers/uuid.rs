use std::error::Error;
use std::io::Write;

use diesel::backend::Backend;
use diesel::sqlite::Sqlite;
use diesel::row::Row;
use diesel::sql_types::{Text, VarChar};
use diesel::types::{FromSqlRow, FromSql, ToSql};
use diesel::serialize::{self, IsNull};
use diesel::deserialize;
use rocket::request::FromParam;
use rocket::http::RawStr;

use uuid;
use std::str::FromStr;

#[derive(Debug, Hash, Eq, PartialEq, Serialize, Deserialize, Clone, Copy, AsExpression, FromSqlRow)]
#[sql_type = "Text"]
pub struct Uuid(uuid::Uuid);

impl Uuid {
    pub fn parse_str(input: &str) -> Result<Self, uuid::parser::ParseError> {
        uuid::Uuid::parse_str(input).map(Uuid)
    }

    pub fn new_v4() -> Self {
        Uuid(uuid::Uuid::new_v4())
    }

    pub fn hyphenated(&self) -> uuid::adapter::Hyphenated {
        self.0.to_hyphenated()
    }
}

impl ToSql<Text, Sqlite> for Uuid {
    fn to_sql<W: Write>(
        &self,
        out: &mut serialize::Output<W, Sqlite>,
    ) -> serialize::Result {
        let hyphenated = self.0.to_hyphenated().to_string();
        ToSql::<VarChar, Sqlite>::to_sql(&hyphenated, out)
    }
}

impl FromSql<VarChar, Sqlite> for Uuid where {
    fn from_sql(value: Option<&<Sqlite as Backend>::RawValue>)
        -> deserialize::Result<Self> {
        let text: String = FromSql::<Text, Sqlite>::from_sql(value)?;
        match uuid::Uuid::from_str(&text) {
            Err(_) => Err("Can not parse UUID datatype.".into()),
            Ok(value) => Ok(Uuid(value))
        }
    }
}

impl<'a> FromParam<'a> for Uuid {
    type Error = uuid::parser::ParseError;

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        param.parse().map(Uuid)
    }
}
