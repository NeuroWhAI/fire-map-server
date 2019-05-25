use std::{
    env,
    fs,
    sync::RwLock,
    time::Duration,
    collections::HashMap,
};
use rocket::{
    response::{
        status::BadRequest,
        content::Json,
    },
    request::Form,
    http::Cookies,
    data::Data,
};
use serde_json::{json, Value as JsonValue};

use crate::util;
use crate::db;
use crate::task_scheduler::{Task, TaskSchedulerBuilder};


type StringResult = Result<String, BadRequest<String>>;


lazy_static! {
    static ref ADMIN_ID: String = {
        env::var("ADMIN_ID").expect("ADMIN_ID must be set")
    };
    static ref ADMIN_PWD: u64 = {
        let sorted_pwd = env::var("ADMIN_PWD").expect("ADMIN_PWD must be set")
            + PASSWORD_HASH_SORT;
        util::calculate_hash(&sorted_pwd)
    };
    static ref SHELTER_DATA: RwLock<String> = {
        RwLock::new(String::new())
    };
    static ref SHELTER_MAP: RwLock<HashMap<i32, Shelter>> = {
        RwLock::new(HashMap::new())
    };
}

const PASSWORD_HASH_SORT: &'static str = "^^ NeuroWhAI 42 5749";


fn hash_pwd(pwd: &str) -> u64 {
    let sorted_pwd = pwd.to_owned() + PASSWORD_HASH_SORT;
    util::calculate_hash(&sorted_pwd)
}


struct Shelter {
    name: String,
    latitude: f64,
    longitude: f64,
    info: String,
    recent_good: i32,
    recent_bad: i32,

    changed: bool,
}

impl Shelter {
    fn new(name: String, latitude: f64, longitude: f64, info: String) -> Self {
        Shelter {
            name,
            latitude,
            longitude,
            info,
            recent_good: 0,
            recent_bad: 0,

            changed: false,
        }
    }
}


#[derive(FromForm)]
pub struct ShelterForm {
    admin_id: String,
    admin_pwd: String,
    name: String,
    latitude: f64,
    longitude: f64,
    info: String,
}


pub fn init_shelter_sys(scheduler: &mut TaskSchedulerBuilder) {
    init_db_and_shelters();
    update_shelter_data(build_shelter_data());

    scheduler.add_task(Task::new(shelter_job, Duration::new(60 * 60, 0)));
}

#[get("/shelter-map")]
pub fn get_shelter_map() -> Json<String> {
    Json(SHELTER_DATA.read().unwrap().clone())
}

#[post("/admin/shelter", format="application/x-www-form-urlencoded", data="<form>")]
pub fn post_shelter(form: Form<ShelterForm>) -> StringResult {
    let hashed_pwd = hash_pwd(&form.admin_pwd);

    if *ADMIN_ID == form.admin_id && *ADMIN_PWD == hashed_pwd {
        let db_result = db::insert_shelter(&db::models::NewShelter {
            name: form.name.clone(),
            latitude: form.latitude,
            longitude: form.longitude,
            info: form.info.clone(),
            recent_good: 0,
            recent_bad: 0,
        });

        match db_result {
            Ok(s) => {
                // Add to cache map.
                let mut cache_map = SHELTER_MAP.write().unwrap();
                cache_map.insert(s.id, Shelter::new(s.name, s.latitude, s.longitude, s.info));

                Ok(s.id.to_string())
            },
            Err(err) => Err(BadRequest(Some(err.to_string()))),
        }
    }
    else {
        Err(BadRequest(Some("Authentication failed!".into())))
    }
}

#[delete("/admin/shelter?<id>&<admin_id>&<admin_pwd>")]
pub fn delete_shelter(id: i32, admin_id: String, admin_pwd: String) -> StringResult {
    let hashed_pwd = hash_pwd(&admin_pwd);

    if *ADMIN_ID == admin_id && *ADMIN_PWD == hashed_pwd {
        match db::delete_shelter(id) {
            Ok(cnt) => {
                // Remove from cache map.
                let mut cache_map = SHELTER_MAP.write().unwrap();
                cache_map.remove(&id);

                Ok(cnt.to_string())
            },
            Err(err) => Err(BadRequest(Some(err.to_string()))),
        }
    }
    else {
        Err(BadRequest(Some("Authentication failed!".into())))
    }
}


fn shelter_job() -> Duration {
    info!("Start job");

    update_shelter_data(build_shelter_data());

    {
        let mut cache_map = SHELTER_MAP.write().unwrap();

        for mut shelter in cache_map.values_mut() {
            if shelter.recent_good > 0 || shelter.recent_bad > 0 {
                shelter.recent_good /= 2;
                shelter.recent_bad /= 2;
                shelter.changed = true;
            }
            else {
                shelter.changed = false;
            }
        }
    }

    {
        let cache_map = SHELTER_MAP.read().unwrap();

        for (&id, shelter) in cache_map.iter() {
            if shelter.changed {
                // Update DB.
                // Retry when failed.
                for _ in 0..3 {
                    match db::update_shelter_score(id, shelter.recent_good, shelter.recent_bad) {
                        Ok(_) => break,
                        Err(err) => warn!("Fail to update a shelter({}) in DB: {}", id, err),
                    }
                }
            }
        }
    }

    Duration::new(60 * 60, 0)
}

fn init_db_and_shelters() {
    match db::get_shelters() {
        Ok(ref shelters) if shelters.len() == 0 => {
            let data: JsonValue = serde_json::from_str(&fs::read_to_string("data/shelter.json")
                .expect("Can't find shelter.json"))
                .expect("Can't parse shelter.json");
            let data = data.get("shelters").expect("Can't find shelters property")
                .as_array().unwrap();

            for val in data {
                // Parse shelter data.
                let shelter = Shelter::new(
                    val.get("name").and_then(|v| v.as_str()).unwrap().to_owned(),
                    val.get("latitude").and_then(|v| v.as_f64()).unwrap(),
                    val.get("longitude").and_then(|v| v.as_f64()).unwrap(),
                    format!("수용: {}명, 면적: {}㎡",
                        val.get("capacity").and_then(|v| v.as_i64()).unwrap(),
                        val.get("area").and_then(|v| v.as_f64()).unwrap())
                );

                // Init DB.
                let db_result = db::insert_shelter(&db::models::NewShelter {
                    name: shelter.name.clone(),
                    latitude: shelter.latitude,
                    longitude: shelter.longitude,
                    info: shelter.info.clone(),
                    recent_good: 0,
                    recent_bad: 0,
                });

                match db_result {
                    Ok(db_shelter) => {
                        // Init shelters.
                        let mut cache_map = SHELTER_MAP.write().unwrap();
                        cache_map.insert(db_shelter.id, shelter);
                    },
                    Err(err) => panic!(err.to_string()),
                }
            }
        },
        Ok(shelters) => {
            // Init shelters.
            let mut cache_map = SHELTER_MAP.write().unwrap();

            for s in shelters {
                cache_map.insert(s.id, Shelter::new(s.name, s.latitude, s.longitude, s.info));
            }
        },
        Err(err) => panic!(err.to_string()),
    }
}

fn update_shelter_data(data: String) {
    *SHELTER_DATA.write().unwrap() = data;
}

fn build_shelter_data() -> String {
    let shelters = {
        let cache_map = SHELTER_MAP.read().unwrap();

        cache_map.iter().map(|(id, s)| {
            json!({
                "id": id,
                "name": s.name,
                "latitude": s.latitude,
                "longitude": s.longitude,
                "info": s.info,
                "good": s.recent_good,
                "bad": s.recent_bad,
            })
        })
        .collect::<Vec<_>>()
    };

    json!({
        "shelters": shelters,
        "size": shelters.len(),
    }).to_string()
}