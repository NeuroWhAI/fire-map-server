pub mod models;
pub mod schema;


use std::env;
use std::time::{UNIX_EPOCH, Duration};

use diesel::prelude::*;
use diesel::pg::PgConnection;
use diesel::result::QueryResult;

use chrono::Utc;

use models::*;
use schema::reports::dsl as r_dsl;
use schema::bad_reports::dsl as bad_dsl;
use schema::shelters::dsl as shelter_dsl;
use schema::user_shelters::dsl as us_dsl;


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

pub fn get_bad_report_list() -> QueryResult<Vec<BadReport>> {
    DB_CONN.with(|conn| {
        bad_dsl::bad_reports
            .load::<BadReport>(conn)
    })
}

pub fn insert_bad_report(report: &NewBadReport) -> QueryResult<BadReport> {
    DB_CONN.with(|conn| {
        diesel::insert_into(schema::bad_reports::table)
            .values(report)
            .get_result::<BadReport>(conn)
    })
}

pub fn delete_bad_report(id: i32) -> QueryResult<usize> {
    DB_CONN.with(|conn| {
        diesel::delete(bad_dsl::bad_reports.find(id))
            .execute(conn)
    })
}

pub fn get_shelters() -> QueryResult<Vec<Shelter>> {
    DB_CONN.with(|conn| {
        shelter_dsl::shelters
            .load::<Shelter>(conn)
    })
}

pub fn insert_shelter(shelter: &NewShelter) -> QueryResult<Shelter> {
    DB_CONN.with(|conn| {
        diesel::insert_into(schema::shelters::table)
            .values(shelter)
            .get_result::<Shelter>(conn)
    })
}

pub fn update_shelter_score(id: i32, good: i32, bad: i32) -> QueryResult<Shelter> {
    DB_CONN.with(|conn| {
        diesel::update(shelter_dsl::shelters.find(id))
            .set((
                schema::shelters::recent_good.eq(good),
                schema::shelters::recent_bad.eq(bad)
            ))
            .get_result(conn)
    })
}

pub fn delete_shelter(id: i32) -> QueryResult<usize> {
    DB_CONN.with(|conn| {
        diesel::delete(shelter_dsl::shelters.find(id))
            .execute(conn)
    })
}

pub fn get_user_shelters() -> QueryResult<Vec<UserShelter>> {
    DB_CONN.with(|conn| {
        us_dsl::user_shelters
            .load::<UserShelter>(conn)
    })
}

pub fn insert_user_shelter(shelter: &NewUserShelter) -> QueryResult<UserShelter> {
    DB_CONN.with(|conn| {
        diesel::insert_into(schema::user_shelters::table)
            .values(shelter)
            .get_result::<UserShelter>(conn)
    })
}

pub fn delete_user_shelter(id: i32) -> QueryResult<usize> {
    DB_CONN.with(|conn| {
        diesel::delete(us_dsl::user_shelters.find(id))
            .execute(conn)
    })
}
