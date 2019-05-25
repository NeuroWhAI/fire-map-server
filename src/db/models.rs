use std::time::SystemTime;
use super::schema::*;


#[derive(Queryable)]
pub struct Report {
    pub id: i32,
    pub user_id: String,
    pub user_pwd: String,
    pub latitude: f64,
    pub longitude: f64,
    pub created_time: SystemTime,
    pub lvl: i32,
    pub description: String,
    pub img_path: String,
}

#[derive(Insertable)]
#[table_name="reports"]
pub struct NewReport {
    pub user_id: String,
    pub user_pwd: String,
    pub latitude: f64,
    pub longitude: f64,
    pub created_time: SystemTime,
    pub lvl: i32,
    pub description: String,
    pub img_path: String,
}

#[derive(Queryable)]
pub struct BadReport {
    pub id: i32,
    pub report_id: i32,
    pub reason: String,
}

#[derive(Insertable)]
#[table_name="bad_reports"]
pub struct NewBadReport {
    pub report_id: i32,
    pub reason: String,
}

#[derive(Queryable)]
pub struct Shelter {
    pub id: i32,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub info: String,
    pub recent_good: i32,
    pub recent_bad: i32,
}

#[derive(Insertable)]
#[table_name="shelters"]
pub struct NewShelter {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub info: String,
    pub recent_good: i32,
    pub recent_bad: i32,
}

#[derive(Queryable)]
pub struct UserShelter {
    pub id: i32,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub info: String,
    pub evidence: String,
}

#[derive(Insertable)]
#[table_name="user_shelters"]
pub struct NewUserShelter {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub info: String,
    pub evidence: String,
}