use std::error::Error;
use std::io::Write;

use diesel::backend::Backend;
use diesel::sqlite::Sqlite;
use diesel::row::Row;
use diesel::types::*;
use rocket::request::FromParam;
use rocket::http::RawStr;

use uuid;
use std::str::FromStr;

#[derive(Debug, Hash, Eq, PartialEq, Serialize, Deserialize, Clone)]
pub struct Uuid(uuid::Uuid);

impl Uuid {
    pub fn parse_str(input: &str) -> Result<Self, uuid::ParseError> {
        uuid::Uuid::parse_str(input).map(|v| Uuid(v))
    }

    pub fn new_v4() -> Self {
        Uuid(uuid::Uuid::new_v4())
    }

    pub fn hyphenated(&self) -> uuid::Hyphenated {
        self.0.hyphenated()
    }
}

expression_impls!(Text -> Uuid);

impl ToSql<VarChar, Sqlite> for Uuid {
    fn to_sql<W: Write>(
        &self,
        out: &mut ToSqlOutput<W, Sqlite>,
    ) -> Result<IsNull, Box<Error + Send + Sync>> {
        let hyphenated = self.0.hyphenated().to_string();
        ToSql::<VarChar, Sqlite>::to_sql(&hyphenated, out)
    }
}

impl FromSql<VarChar, Sqlite> for Uuid where {
    fn from_sql(value: Option<&<Sqlite as Backend>::RawValue>)
        -> Result<Self, Box<Error + Send + Sync>> {
        let text: String = FromSql::<Text, Sqlite>::from_sql(value)?;
        match uuid::Uuid::from_str(&text) {
            Err(_) => Err("Can not parse UUID datatype.".into()),
            Ok(value) => Ok(Uuid(value))
        }
    }
}

impl FromSqlRow<Text, Sqlite> for Uuid {
    fn build_from_row<T: Row<Sqlite>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
        Self::from_sql(row.take())
    }
}

impl<'a> FromParam<'a> for Uuid {
    type Error = uuid::ParseError;

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        param.parse().map(|v| Uuid(v))
    }
}
