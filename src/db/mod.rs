pub mod models;
pub mod schema;


use std::env;
use std::time::{SystemTime, Duration};

use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::result::QueryResult;

use models::*;
use schema::reports::dsl::{self};


thread_local! {
    static DB_CONN: PgConnection = establish_connection();
}


fn establish_connection() -> PgConnection {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    println!("{}", &database_url);
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn get_reports_within(time: Duration) -> QueryResult<Vec<Report>> {
    DB_CONN.with(|conn| {
        dsl::reports
            .filter(dsl::created_time.gt(SystemTime::now() - time))
            .load::<Report>(conn)
    })
}

pub fn get_report(id: i32) -> QueryResult<Report> {
    DB_CONN.with(|conn| {
        dsl::reports
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
        diesel::delete(dsl::reports.find(id))
            .execute(conn)
    })
}
