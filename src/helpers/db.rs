use std::ops::Deref;
use rocket::http::Status;
use rocket::{Request, State, Outcome};
use rocket::request::{self, FromRequest};
use diesel::pg::PgConnection;
use r2d2;
use r2d2_diesel::ConnectionManager;
use std::env;
use dotenv::dotenv;

pub type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;
pub type Connection = PgConnection;

pub fn init_db_pool() -> Pool {
    let config = r2d2::Config::default();
    dotenv().unwrap();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::new(config, manager).expect("Failed to create pool.")
}

#[cfg(test)]
pub fn init_test_db_pool() -> Pool {
    use diesel::Connection;
    let config = r2d2::Config::builder().pool_size(1).build();
    dotenv().unwrap();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::new(config, manager).expect("Failed to create pool.");
    (*pool.get().unwrap()).begin_test_transaction();
    pool
}

pub struct DB(PooledConnection);

impl Deref for DB {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for DB {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<DB, ()> {
        let pool = match <State<Pool> as FromRequest>::from_request(request) {
            Outcome::Success(pool) => pool,
            Outcome::Failure(e) => return Outcome::Failure(e),
            Outcome::Forward(_) => return Outcome::Forward(()),
        };

        match pool.get() {
            Ok(conn) => Outcome::Success(DB(conn)),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ()))
        }
    }
}
