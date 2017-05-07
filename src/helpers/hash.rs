use std::io::prelude::*;
use std::error::Error;

use pg::Pg;
use types::{self, ToSql, IsNull, FromSql};

struct Hash {
    data: [u8; 256]
}

primitive_impls!(Hash -> (Hash, pg: (1001, )))


impl ToSql<types::, Pg> for Hash {
    fn to_sql<W: Write>(&self, out: &mut W) -> Result<IsNull, Box<Error+Send+Sync>> {
        out.write_all(self.data.as_bytes())
            .map(|_| IsNull::No)
            .map_err(|e| Box::new(e) as Box<Error+Send+Sync>)
    }
}
