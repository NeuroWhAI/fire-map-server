use std::{
    sync::RwLock,
    time::Duration,
};
use rocket::{
    response::content::Json,
};
use serde_json::{Value as JsonValue, json};

use crate::task_scheduler::{Task, TaskSchedulerBuilder};


lazy_static! {
    static ref FIRE_EVENT_MAP: RwLock<String> = {
        RwLock::new(String::new())
    };
}


enum FireStatus {
    Fire,
    Extinguished,
    Clear,
}


pub fn init_fire_sys(scheduler: &mut TaskSchedulerBuilder) {
    let delay = match get_fire_event_json() {
        Ok(data) => {
            update_fire_event_map(data);
            Duration::new(60 * 3, 0)
        },
        Err(err) => {
            warn!("Fail to init fire events: {}", err);

            update_fire_event_map(json!({
                "events": [],
                "size": 0,
            }).to_string());

            Duration::new(60 * 1, 0)
        },
    };

    scheduler.add_task(Task::new(fire_event_job, delay));
}

#[get("/fire-event-map")]
pub fn get_fire_event_map() -> Json<String> {
    Json(FIRE_EVENT_MAP.read().unwrap().clone())
}

fn fire_event_job() -> Duration {
    info!("Start job for fire event");

    match get_fire_event_json() {
        Ok(data) => {
            update_fire_event_map(data);
            Duration::new(60 * 3, 0)
        },
        Err(err) => {
            warn!("Fail to get fire event: {}", err);
            Duration::new(60 * 1, 0)
        },
    }
}

fn update_fire_event_map(data: String) {
    *FIRE_EVENT_MAP.write().unwrap() = data;
}

fn get_fire_event_json() -> Result<String, String> {
    let json_result = reqwest::get("http://116.67.84.152/ffas/gis/selectFireShowList.do")
        .and_then(|mut res| res.text());

    match json_result {
        Ok(json_str) => {
            serde_json::from_str::<JsonValue>(&json_str)
                .map_err(|err| err.to_string())
                .and_then(|v| {
                    v.as_array()
                        .and_then(|arr| arr.get(0))
                        .and_then(|arr| arr.as_array())
                        .ok_or("Invalid fire event data".into())
                        .map(|events| {
                            // Parse each fire events.
                            let results = events.into_iter().map(|evt| {
                                let status_opt = evt["frfrPrgrsStcd"].as_str()
                                    .map(|s| convert_str_to_fire_status(s));
                                let latitude_opt = evt["frfrSttmnLctnYcrd"].as_str()
                                    .filter(|y| y.find('.').is_some())
                                    .and_then(|y| y.parse::<f64>().ok());
                                let longitude_opt = evt["frfrSttmnLctnXcrd"].as_str()
                                    .filter(|x| x.find('.').is_some())
                                    .and_then(|x| x.parse::<f64>().ok());
                                let address_opt = evt["frfrSttmnAddr"].as_str()
                                    .map(|adr| adr.to_owned());
                                let date_opt = evt["frfrSttmnDt"].as_str()
                                    .map(|date| date.to_owned());
                                let time_opt = evt["frfrSttmnHms"].as_str()
                                    .map(|time| time.to_owned());

                                if let
                                    (Some(status),
                                    Some(latitude),
                                    Some(longitude),
                                    Some(address),
                                    Some(date),
                                    Some(time)) =
                                        (status_opt,
                                        latitude_opt,
                                        longitude_opt,
                                        address_opt,
                                        date_opt,
                                        time_opt)
                                {
                                    Ok(json!({
                                        "status": status as i32,
                                        "latitude": latitude,
                                        "longitude": longitude,
                                        "address": address,
                                        "date": date,
                                        "time": time,
                                    }))
                                }
                                else {
                                    Err("Fail to parse fire events".to_owned())
                                }
                            });

                            let fire_events = results
                                .filter_map(|res| res.ok())
                                .collect::<Vec<_>>();

                            json!({
                                "events": fire_events,
                                "size": fire_events.len(),
                            }).to_string()
                        })
                })
        },
        Err(err) => Err(err.to_string()),
    }
}

fn convert_str_to_fire_status(status: &str) -> FireStatus {
    match status {
        "01" | "02" => FireStatus::Fire,
        "05" => FireStatus::Clear,
        _ => FireStatus::Extinguished,
    }
}
