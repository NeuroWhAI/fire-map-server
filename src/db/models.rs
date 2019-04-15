use std::time::SystemTime;
use super::schema::reports;


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