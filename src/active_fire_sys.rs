use std::{
    sync::RwLock,
    time::Duration,
};
use rocket::{
    response::content::Json,
};
use serde_json::json;
use chrono::NaiveDateTime;

use crate::task_scheduler::{Task, TaskSchedulerBuilder};


lazy_static! {
    static ref FIRE_DATA: RwLock<String> = {
        RwLock::new(String::new())
    };
}


struct FireRecord {
    latitude: f64,
    longitude: f64,
    brightness: f32,
    radiative_power: f32,
    time: i64,
}


pub fn init_active_fire_sys(scheduler: &mut TaskSchedulerBuilder) {
    let delay = match get_fire_data() {
        Ok(data) => {
            update_fire_data(data);
            Duration::new(60 * 15, 0)
        },
        Err(err) => {
            warn!("Fail to init active fire cache: {}", err);

            update_fire_data(json!({
                "fires": [],
                "size": 0,
            }).to_string());

            Duration::new(60 * 1, 0)
        }
    };

    scheduler.add_task(Task::new(active_fire_job, delay));
}

#[get("/active-fire-map")]
pub fn get_active_fire_map() -> Json<String> {
    Json(FIRE_DATA.read().unwrap().clone())
}

fn active_fire_job() -> Duration {
    info!("Start job");

    match get_fire_data() {
        Ok(json) => {
            update_fire_data(json);
            Duration::new(60 * 15, 0)
        },
        Err(err) => {
            warn!("Fail to get fire data: {}", err);
            Duration::new(60 * 1, 0)
        },
    }
}

fn update_fire_data(json: String) {
    *FIRE_DATA.write().unwrap() = json;
}

fn get_fire_data() -> Result<String, String> {
    let modis = parse_fire_data("https://firms.modaps.eosdis.nasa.gov/active_fire/c6/text/MODIS_C6_Russia_and_Asia_24h.csv");
    let viirs = parse_fire_data("https://firms.modaps.eosdis.nasa.gov/active_fire/viirs/text/VNP14IMGTDL_NRT_Russia_and_Asia_24h.csv");

    let records = match (modis, viirs) {
        (Ok(mut m), Ok(mut v)) => {
            m.append(&mut v);
            m
        },
        (Err(err), Ok(v)) => {
            warn!("Fail to parse MODIS: {}", err);
            v
        },
        (Ok(m), Err(err)) => {
            warn!("Fail to parse VIIRS: {}", err);
            m
        },
        (Err(err), Err(_)) => return Err(err),
    };

    let json_records = records.into_iter().map(|r| {
        json!({
            "latitude": r.latitude,
            "longitude": r.longitude,
            "bright": r.brightness,
            "power": r.radiative_power,
            "time": r.time,
        })
    }).collect::<Vec<_>>();

    Ok(json!({
        "fires": json_records,
        "size": json_records.len(),
    }).to_string())
}

fn parse_fire_data(uri: &str) -> Result<Vec<FireRecord>, String> {
    reqwest::get(uri)
        .and_then(|mut res| res.text())
        .map_err(|err| err.to_string())
        .map(|csv| {
            csv.lines().skip(1)
                .map(|row| row.split(',').collect())
                .filter(|records: &Vec<&str>| records.len() >= 12)
                .filter(|records| {
                    // Only high confidence data.
                    match records[8] {
                        "high" => true,
                        _ => match records[8].parse::<i32>() {
                            Ok(confidence) => (confidence >= 70),
                            _ => false
                        }
                    }
                })
                .map(|records| {
                    let lat_res = records[0].parse();
                    let lon_res = records[1].parse();
                    let bright_res = records[2].parse();
                    let power_res = records[11].parse();

                    let date_str = records[5];
                    let time_str = format!("{:0>4}", records[6]);
                    let date_time_str = format!("{} {}", date_str, time_str);
                    let time_res = NaiveDateTime::parse_from_str(&date_time_str,
                        "%Y-%m-%d %H%M");

                    match (lat_res, lon_res, bright_res, power_res, time_res) {
                        (Ok(lat), Ok(lon), Ok(bright), Ok(power), Ok(time)) => Some(FireRecord {
                            latitude: lat,
                            longitude: lon,
                            brightness: bright,
                            radiative_power: power,
                            time: time.timestamp(),
                        }),
                        _ => None
                    }
                })
                .filter_map(|opt| opt)
                .filter(|r| r.latitude > 32.477024 && r.longitude > 123.825178
                    && r.latitude < 39.322145 && r.longitude < 132.799568)
                .collect()
        })
}