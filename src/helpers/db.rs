use std::ops::Deref;
use rocket::http::Status;
use rocket::{Request, State, Outcome};
use rocket::request::{self, FromRequest};
use diesel::sqlite::SqliteConnection;
use r2d2_diesel::ConnectionManager;
use r2d2::{self, CustomizeConnection};
use std::env;
use dotenv::dotenv;
use diesel::dsl::sql;
use diesel::{self, ExecuteDsl};

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;
pub type Connection = SqliteConnection;

/// Our connection_customizer will set timeout behavior on the SQLite connection
#[derive(Copy, Clone, Debug)]
pub struct BusyWaitConnectionCustomizer;

impl<C: diesel::Connection, E> CustomizeConnection<C, E> for BusyWaitConnectionCustomizer {
    fn on_acquire(&self, conn: &mut C) -> Result<(), E> {
        conn.batch_execute("PRAGMA busy_timeout = 5000;").unwrap();
        conn.batch_execute("PRAGMA journal_mode = WAL;").unwrap();
        Ok(())
    }
}

/// Initialize database DB pool from specified URL.
/// Will fall back to "DATABASE_URL" environment variable if `url` is None.
pub fn init_db_pool(url: Option<String>) -> Pool {
    dotenv().unwrap();
    let database_url = url.unwrap_or(
        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    );
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    r2d2::Pool::builder()
        .connection_customizer(Box::new(BusyWaitConnectionCustomizer{}))
        .build(manager)
        .expect("Failed to create pool.")
}

#[cfg(test)]
pub fn init_test_db_pool() -> Pool {
    use diesel::Connection;
    dotenv().unwrap();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
    let pool = r2d2::Pool::builder()
        .connection_customizer(Box::new(BusyWaitConnectionCustomizer{}))
        .max_size(1)
        .build(manager)
        .expect("Failed to create pool.");
    ::embedded_migrations::run(&*pool.get().unwrap());
    (&*pool.get().unwrap()).begin_test_transaction();
    pool
}

pub struct DB(PooledConnection);

impl Deref for DB {
    type Target = SqliteConnection;

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
