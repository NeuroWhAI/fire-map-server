use std::{fs::File, io::{BufReader, BufRead}, sync::RwLock};

use rocket::response::content::Json;
use serde_json::json;

use crate::TaskSchedulerBuilder;


lazy_static! {
    static ref PLACE_MAP_CACHE: RwLock<String> = {
        RwLock::new(String::new())
    };
}


pub fn init_danger_place_sys(_scheduler: &mut TaskSchedulerBuilder) {
    update_danger_place_map();
}

#[get("/danger-place-map")]
pub fn get_danger_place_map() -> Json<String> {
    Json(PLACE_MAP_CACHE.read().unwrap().clone())
}


fn update_danger_place_map() {
    let lines = BufReader::new(File::open("data/danger_places.csv")
        .expect("Fail to open danger place data file"))
        .lines()
        .skip(1)
        .filter_map(|ln| ln.ok());

    let places = lines.map(|line| {
        let data = line.split(',').collect::<Vec<_>>();
        json!({
            "addr": data[0],
            "lat": data[1].parse::<f64>().unwrap_or_default(),
            "lon": data[2].parse::<f64>().unwrap_or_default(),
            "t": data[3].parse::<i32>().unwrap_or(-1),
            "name": data[4],
        })
    }).collect::<Vec<_>>();

    let map_data = json!({
        "places": places,
        "size": places.len(),
    }).to_string();
    
    *PLACE_MAP_CACHE.write().unwrap() = map_data;
}
