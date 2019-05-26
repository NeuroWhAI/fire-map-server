#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate lazy_static;
extern crate rand;
#[macro_use] extern crate rocket;
#[macro_use] extern crate diesel;
#[macro_use] extern crate log;


mod db;
mod util;
mod logger;
mod task_scheduler;
mod captcha_sys;
mod report_sys;
mod shelter_sys;
mod cctv_sys;
mod fire_sys;
mod wind_sys;
mod active_fire_sys;
mod fire_forecast_sys;


use std::{env, env::VarError};
use std::path::{Path, PathBuf};
use std::time::Duration;
use rocket::response::NamedFile;
use rocket::fairing::AdHoc;
use log::LevelFilter;

use crate::logger::Logger;
use crate::task_scheduler::TaskSchedulerBuilder;


lazy_static! {
    static ref ROCKET_ENV: String = {
        env::var("ROCKET_ENV")
            .or_else(|_| -> Result<String, VarError> {
                if cfg!(debug_assertions) {
                    Ok("development".into())
                }
                else {
                    Ok("production".into())
                }
            }).unwrap()
    };
    static ref DEBUG: bool = {
        let dbg_envs = ["dev", "development", "staging", "stage"];
        dbg_envs.iter().any(|&v| v == *ROCKET_ENV)
    };
}

static LOGGER: Logger = Logger;

const STATIC_DIR: &'static str = "static/";
const TEST_DIR: &'static str = "test/";


#[get("/")]
fn index() -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_DIR).join("index.html")).ok()
}

#[get("/<file..>")]
fn get_static_file(file: PathBuf) -> Option<NamedFile> {
    if !*DEBUG && file.starts_with(TEST_DIR) {
        None
    }
    else {
        NamedFile::open(Path::new(STATIC_DIR).join(file)).ok()
    }
}


fn main() {
    let log_level = if *DEBUG {
        LevelFilter::Info
    }
    else {
        LevelFilter::Warn
    };

    log::set_logger(&LOGGER)
        .map(|_| log::set_max_level(log_level))
        .expect("Fail to set logger");


    let mut scheduler = TaskSchedulerBuilder::new()
        .n_workers(4)
        .period_resolution(Duration::new(0, 100/*ms*/ * 1_000_000));

    report_sys::init_report_sys(&mut scheduler);
    shelter_sys::init_shelter_sys(&mut scheduler);
    cctv_sys::init_cctv_sys(&mut scheduler);
    fire_sys::init_fire_sys(&mut scheduler);
    wind_sys::init_wind_sys(&mut scheduler);
    active_fire_sys::init_active_fire_sys(&mut scheduler);
    fire_forecast_sys::init_fire_forecast_sys(&mut scheduler);

    let scheduler = scheduler.build();


    if *DEBUG {
        rocket::ignite()
            .attach(AdHoc::on_response("CORS", |_, rsp| {
                rsp.set_raw_header("Access-Control-Allow-Origin", "*");
                rsp.set_raw_header("Access-Control-Allow-Methods", "GET");
                rsp.set_raw_header("Access-Control-Max-Age", "3600");
                rsp.set_raw_header("Access-Control-Allow-Headers", "Origin,Accept,X-Requested-With,Content-Type,Access-Control-Request-Method,Access-Control-Request-Headers,Authorization");
            }))
            .mount("/", routes![captcha_sys::test_captcha])
    }
    else {
        rocket::ignite()
    }
    .mount("/", routes![index, get_static_file])
    .mount("/", routes![
        captcha_sys::get_captcha,
    ])
    .mount("/", routes![
        report_sys::get_report,
        report_sys::get_report_map,
        report_sys::post_report,
        report_sys::delete_report,
        report_sys::post_upload_image,
        report_sys::post_bad_report,
        report_sys::get_bad_report_list,
        report_sys::delete_bad_report,
    ])
    .mount("/", routes![
        shelter_sys::get_shelter,
        shelter_sys::get_shelter_map,
        shelter_sys::post_shelter,
        shelter_sys::delete_shelter,
        shelter_sys::get_user_shelter_list,
        shelter_sys::post_user_shelter,
        shelter_sys::post_eval_shelter,
        shelter_sys::delete_user_shelter,
    ])
    .mount("/", routes![
        cctv_sys::get_cctv,
        cctv_sys::get_cctv_map,
    ])
    .mount("/", routes![
        fire_sys::get_fire_warning,
        fire_sys::get_fire_event_map,
    ])
    .mount("/", routes![
        wind_sys::get_wind_map_metadata,
        wind_sys::get_wind_map,
    ])
    .mount("/", routes![
        active_fire_sys::get_active_fire_map,
    ])
    .mount("/", routes![
        fire_forecast_sys::get_fire_forecast_map,
    ])
    .launch();


    scheduler.join();
}
