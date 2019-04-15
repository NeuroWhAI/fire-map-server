#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate lazy_static;
extern crate rand;
#[macro_use] extern crate rocket;
#[macro_use] extern crate diesel;


mod db;
mod util;
mod captcha_sys;
mod report_route;


use std::{env, env::VarError};
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;
use rocket::response::NamedFile;


const STATIC_DIR: &'static str = "static/";
const TEST_DIR: &'static str = "test/";


#[get("/")]
fn index() -> &'static str {
    "Fire Map Server"
}

#[get("/<file..>")]
fn get_static_file(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(STATIC_DIR).join(file)).ok()
}

#[get("/<file..>")]
fn get_test_file(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(TEST_DIR).join(file)).ok()
}


fn main() {
    let rocket_env = env::var("ROCKET_ENV")
        .or_else(|_| -> Result<String, VarError> {
            if cfg!(debug_assertions) {
                Ok("development".into())
            }
            else {
                Ok("production".into())
            }
        }).unwrap();

    create_dir_all(Path::new(STATIC_DIR).join(report_route::IMAGE_PUBLIC_DIR))
        .and(create_dir_all(Path::new(report_route::IMAGE_UPLOAD_DIR)))
        .expect("Initial directory creation failed.");

    let dbg_envs = ["dev", "development", "staging", "stage"];
    if dbg_envs.iter().any(|&v| v == rocket_env) {
        // Debug
        rocket::ignite()
            .mount(&format!("/{}", TEST_DIR), routes![get_test_file])
            .mount("/", routes![captcha_sys::test_captcha])
    }
    else {
        // Release
        rocket::ignite()
    }
    .mount("/", routes![index])
    .mount(&format!("/{}", STATIC_DIR), routes![get_static_file])
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
    .launch();
}
