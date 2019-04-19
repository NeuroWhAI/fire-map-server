pub mod models;
pub mod schema;


use std::env;
use std::time::{UNIX_EPOCH, Duration};

use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::result::QueryResult;

use chrono::Utc;

use models::*;
use schema::reports::dsl::{self as r_dsl};


thread_local! {
    static DB_CONN: PgConnection = establish_connection();
}


fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn get_reports_within(time: Duration) -> QueryResult<Vec<Report>> {
    let utc = Utc::now().timestamp() as u64;
    let now = UNIX_EPOCH + Duration::new(utc, 0);
    let filter_time = now - time;

    DB_CONN.with(|conn| {
        r_dsl::reports
            .filter(r_dsl::created_time.gt(filter_time))
            .load::<Report>(conn)
    })
}

pub fn get_report(id: i32) -> QueryResult<Report> {
    DB_CONN.with(|conn| {
        r_dsl::reports
            .find(id)
            .first(conn)
    })
}

pub fn insert_report(report: &NewReport) -> QueryResult<Report> {
    DB_CONN.with(|conn| {
        diesel::insert_into(schema::reports::table)
            .values(report)
            .get_result::<Report>(conn)
    })
}

pub fn delete_report(id: i32) -> QueryResult<usize> {
    DB_CONN.with(|conn| {
        diesel::delete(r_dsl::reports.find(id))
            .execute(conn)
    })
}

pub fn insert_bad_report(report: &NewBadReport) -> QueryResult<BadReport> {
    DB_CONN.with(|conn| {
        diesel::insert_into(schema::bad_reports::table)
            .values(report)
            .get_result::<BadReport>(conn)
    })
}
