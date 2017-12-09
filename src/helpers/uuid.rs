use std::error::Error;
use std::io::Write;

use diesel::backend::Backend;
use diesel::sqlite::Sqlite;
use diesel::row::Row;
use diesel::types::*;

use uuid;
use std::str::FromStr;

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Uuid(uuid::Uuid);

impl Uuid {
    fn parse_str(input: &str) -> Result<Uuid, uuid::ParseError> {
        uuid::Uuid::parse_str(input)
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
            Ok(value) => Ok(value)
        }
    }
}

impl FromSqlRow<Text, Sqlite> for Uuid {
    fn build_from_row<T: Row<Sqlite>>(row: &mut T) -> Result<Self, Box<Error + Send + Sync>> {
        Self::from_sql(row.take())
    }
}
