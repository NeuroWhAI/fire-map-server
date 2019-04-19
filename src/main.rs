#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate lazy_static;
extern crate rand;
#[macro_use] extern crate rocket;
#[macro_use] extern crate diesel;


mod db;
mod util;
mod captcha_sys;
mod report_route;
mod shelter_route;
mod cctv_sys;


use std::{env, env::VarError};
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;
use rocket::response::NamedFile;
use rocket::fairing::AdHoc;


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

const STATIC_DIR: &'static str = "static/";
const TEST_DIR: &'static str = "test/";


#[get("/")]
fn index() -> &'static str {
    "Fire Map Server"
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
    let cctv_task = cctv_sys::init_cctv_sys();


    create_dir_all(Path::new(STATIC_DIR).join(report_route::IMAGE_PUBLIC_DIR))
        .and(create_dir_all(Path::new(report_route::IMAGE_UPLOAD_DIR)))
        .expect("Initial directory creation failed.");


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
        report_route::get_report,
        report_route::get_report_map,
        report_route::post_report,
        report_route::delete_report,
        report_route::post_upload_image,
    ])
    .mount("/", routes![
        shelter_route::get_shelter_map,
    ])
    .mount("/", routes![
        cctv_sys::get_cctv,
        cctv_sys::get_cctv_map,
    ])
    .launch();


    cctv_task.join().unwrap();
}
