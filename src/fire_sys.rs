use std::{
    thread,
    sync::RwLock,
    time::Duration,
};
use rocket::{
    http::ContentType,
    response::{
        Content,
        content::Json,
    },
};
use serde_json::{Value as JsonValue, json};


lazy_static! {
    static ref WARNING_IMG_URI: RwLock<String> = {
        RwLock::new(String::new())
    };
    static ref WARNING_IMG: RwLock<Vec<u8>> = {
        RwLock::new(Vec::new())
    };
    static ref FIRE_EVENT_MAP: RwLock<String> = {
        RwLock::new(String::new())
    };
}


enum FireStatus {
    Fire,
    Extinguished,
    Clear,
}


pub fn init_fire_sys() -> thread::JoinHandle<()> {
    let img_uri = get_fire_warning_image_uri()
        .expect("Fail to get uri of fire warning image");
    update_fire_image_uri(img_uri.clone());
    update_fire_image(get_fire_warning_image(&img_uri)
        .expect("Fail to get fire warning image"));

    update_fire_event_map(get_fire_event_json()
        .expect("Fail to get fire events"));

    thread::spawn(fire_job)
}

#[get("/fire-warning")]
pub fn get_fire_warning() -> Content<Vec<u8>> {
    Content(ContentType::PNG, WARNING_IMG.read().unwrap().clone())
}

#[get("/fire-event-map")]
pub fn get_fire_event_map() -> Json<String> {
    Json(FIRE_EVENT_MAP.read().unwrap().clone())
}

fn fire_job() {
    thread::sleep(Duration::new(60 * 5, 0));

    loop {
        let mut failed = false;


        match get_fire_event_json() {
            Ok(data) => update_fire_event_map(data),
            Err(_) => failed = true,
        }


        let uri_result = get_fire_warning_image_uri();

        if let Ok(uri) = uri_result {
            let missed = {
                &*WARNING_IMG_URI.read().unwrap() != &uri 
            };

            if missed {
                match get_fire_warning_image(&uri) {
                    Ok(bytes) => {
                        update_fire_image(bytes);
                        update_fire_image_uri(uri);
                    },
                    _ => failed = true,
                }
            }
        }
        else {
            failed = true;
        }


        if failed {
            thread::sleep(Duration::new(60 * 1, 0));
        }
        else {
            thread::sleep(Duration::new(60 * 3, 0));
        }
    }
}

fn update_fire_image(img_bytes: Vec<u8>) {
    let mut cache = WARNING_IMG.write().unwrap();
    *cache = img_bytes;
}

fn update_fire_image_uri(uri: String) {
    let mut cache = WARNING_IMG_URI.write().unwrap();
    *cache = uri;
}

fn get_fire_warning_image(uri: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let result = reqwest::get(&format!("http://www.forest.go.kr{}", uri))
        .map_err(|err| err.to_string())
        .and_then(|mut res| res.copy_to(&mut bytes).map_err(|err| err.to_string()));

    match result {
        Ok(_) => Ok(bytes),
        Err(err) => Err(err),
    }
}

fn get_fire_warning_image_uri() -> Result<String, String> {
    let html_result = reqwest::get("http://www.forest.go.kr/kfsweb/kfs/idx/Index.do")
        .and_then(|mut res| res.text());

    match html_result {
        Ok(html) => {
            let uri_opt = html.find("산불경보")
                .and_then(|index| {
                    html[index..].find("intro_img04.png")
                        .or(html[index..].find("intro_img05.png"))
                        .or(html[index..].find("intro_img06.png"))
                        .or(html[index..].find("intro_img07.png"))
                        .and_then(|offset| Some(index + offset))
                })
                .and_then(|index| html[..index].rfind('"'))
                .and_then(|index| {
                    html[(index + 1)..].find('"')
                        .and_then(|offset| Some(index + 1 + offset))
                        .and_then(|end_index| Some(&html[(index + 1)..end_index]))
                });

            match uri_opt {
                Some(uri) => Ok(uri.to_owned()),
                None => Err("Fail to parse fire warning image".into()),
            }
        },
        Err(err) => Err(err.to_string()),
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
                                .filter(|res| res.is_ok())
                                .map(|res| res.unwrap())
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
