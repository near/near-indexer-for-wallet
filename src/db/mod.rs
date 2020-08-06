use std::env;

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use dotenv::dotenv;

pub(crate) mod access_keys;
pub(crate) mod enums;

pub(crate) use access_keys::AccessKey;

pub(crate) fn establish_connection() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set in .env file"));
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder()
        .build(manager)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
