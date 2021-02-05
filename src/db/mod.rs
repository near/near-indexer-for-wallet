use std::env;

use diesel::PgConnection;
use dotenv::dotenv;

pub(crate) mod access_keys;
pub(crate) mod enums;

pub(crate) use access_keys::AccessKey;

pub(crate) fn establish_connection() -> actix_diesel::Database<PgConnection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set in .env file"));
    actix_diesel::Database::builder().open(&database_url)
}
