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
};
use serde_json::{json, Value as JsonValue};

use crate::db;
use crate::util;
use crate::captcha_sys::verify_and_remove_captcha;
use crate::task_scheduler::{Task, TaskSchedulerBuilder};


type JsonResult = Result<Json<String>, BadRequest<String>>;
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


fn check_admin(id: &str, pwd: &str) -> bool {
    let sorted_pwd = pwd.to_owned() + PASSWORD_HASH_SORT;
    let hashed_pwd = util::calculate_hash(&sorted_pwd);

    *ADMIN_ID == id && *ADMIN_PWD == hashed_pwd
}


struct Shelter {
    id: i32,
    name: String,
    latitude: f64,
    longitude: f64,
    info: String,
    recent_good: i32,
    recent_bad: i32,

    cached: bool,
    json_cache: String,
    synced: bool,
}

impl Shelter {
    fn new(id: i32, name: String, latitude: f64, longitude: f64, info: String) -> Self {
        let mut s = Shelter {
            id,
            name,
            latitude,
            longitude,
            info,
            recent_good: 0,
            recent_bad: 0,

            cached: false,
            json_cache: String::new(),
            synced: true,
        };

        s.update_cache();

        s
    }

    fn update_cache(&mut self) {
        self.json_cache = json!({
            "id": self.id,
            "name": self.name,
            "latitude": self.latitude,
            "longitude": self.longitude,
            "info": self.info,
            "good": self.recent_good,
            "bad": self.recent_bad,
        }).to_string();

        self.cached = true;
    }

    fn reserve_update(&mut self) {
        self.cached = false;
        self.synced = false;
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


#[derive(FromForm)]
pub struct UserShelterForm {
    captcha: String,
    name: String,
    latitude: f64,
    longitude: f64,
    info: String,
    evidence: String,
}

impl UserShelterForm {
    fn verify_error(&self) -> Option<&'static str> {
        let len_name = self.name.chars().count();
        let len_info = self.info.chars().count();

        if len_name < 2 {
            Some("Name must be at least 2 characters")
        }
        else if len_name > 10 {
            Some("Name can not be longer than 10 characters")
        }
        else if len_info > 20 {
            Some("The maximum length of the information is 20")
        }
        else {
            None
        }
    }
}


pub fn init_shelter_sys(scheduler: &mut TaskSchedulerBuilder) {
    init_db_and_shelters();
    update_shelter_data(build_shelter_data());

    scheduler.add_task(Task::new(shelter_data_job, Duration::new(60 * 5, 0)));
    scheduler.add_task(Task::new(shelter_update_job, Duration::new(60 * 60, 0)));
}

#[get("/shelter?<id>")]
pub fn get_shelter(id: i32) -> JsonResult {
    let cache_map = SHELTER_MAP.read().unwrap();

    match cache_map.get(&id) {
        Some(shelter) => Ok(Json(shelter.json_cache.clone())),
        None => Err(BadRequest(Some("Not found".into()))),
    }
}

#[get("/shelter-map")]
pub fn get_shelter_map() -> Json<String> {
    Json(SHELTER_DATA.read().unwrap().clone())
}

#[post("/admin/shelter", format="application/x-www-form-urlencoded", data="<form>")]
pub fn post_shelter(form: Form<ShelterForm>) -> StringResult {
    if check_admin(&form.admin_id, &form.admin_pwd) {
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
                cache_map.insert(s.id, Shelter::new(s.id, s.name, s.latitude, s.longitude, s.info));

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
    if check_admin(&admin_id, &admin_pwd) {
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

#[get("/admin/user-shelter-list?<admin_id>&<admin_pwd>")]
pub fn get_user_shelter_list(admin_id: String, admin_pwd: String) -> JsonResult {
    if !check_admin(&admin_id, &admin_pwd) {
        return Err(BadRequest(Some("Authentication failed!".into())));
    }

    match db::get_user_shelters() {
        Ok(shelters) => {
            let parts = shelters.iter().map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "latitude": s.latitude,
                    "longitude": s.longitude,
                    "info": s.info,
                    "evidence": s.evidence,
                })
            })
            .collect::<Vec<_>>();

            Ok(Json(json!({
                "shelters": parts,
                "size": parts.len(),
            }).to_string()))
        },
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

#[post("/user-shelter", format="application/x-www-form-urlencoded", data="<form>")]
pub fn post_user_shelter(form: Option<Form<UserShelterForm>>, cookies: Cookies) -> StringResult {
    if form.is_none() {
        return Err(BadRequest(Some("Invalid form".into())));
    }

    let form = form.unwrap();


    if let Some(err) = form.verify_error() {
        return Err(BadRequest(Some(err.to_string())));
    }

    if !verify_and_remove_captcha(cookies, 3, &form.captcha) {
        return Err(BadRequest(Some("Wrong captcha".into())));
    }


    let db_result = db::insert_user_shelter(&db::models::NewUserShelter {
        name: form.name.clone(),
        latitude: form.latitude,
        longitude: form.longitude,
        info: form.info.clone(),
        evidence: form.evidence.clone(),
    });

    match db_result {
        Ok(user_shelter) => Ok(user_shelter.id.to_string()),
        Err(err) => Err(BadRequest(Some(err.to_string()))),
    }
}

#[post("/eval-shelter?<captcha>&<id>&<score>")]
pub fn post_eval_shelter(captcha: String, id: i32, score: i32, cookies: Cookies) -> JsonResult {
    if !verify_and_remove_captcha(cookies, 4, &captcha) {
        return Err(BadRequest(Some("Wrong captcha".into())));
    }


    let mut cache_map = SHELTER_MAP.write().unwrap();

    if let Some(mut shelter) = cache_map.get_mut(&id) {
        if score > 0 {
            shelter.recent_good += 1;
            shelter.reserve_update();
        }
        else if score < 0 {
            shelter.recent_bad += 1;
            shelter.reserve_update();
        }

        Ok(Json(json!({
            "id": id,
            "good": shelter.recent_good,
            "bad": shelter.recent_bad,
        }).to_string()))
    }
    else {
        Err(BadRequest(Some("Can't find a shelter".into())))
    }
}

#[delete("/admin/user-shelter?<id>&<admin_id>&<admin_pwd>")]
pub fn delete_user_shelter(id: i32, admin_id: String, admin_pwd: String) -> StringResult {
    if check_admin(&admin_id, &admin_pwd) {
        match db::delete_user_shelter(id) {
            Ok(cnt) => Ok(cnt.to_string()),
            Err(err) => Err(BadRequest(Some(err.to_string()))),
        }
    }
    else {
        Err(BadRequest(Some("Authentication failed!".into())))
    }
}


fn shelter_data_job() -> Duration {
    info!("Start data job");

    {
        let mut cache_map = SHELTER_MAP.write().unwrap();

        for shelter in cache_map.values_mut() {
            if !shelter.cached {
                shelter.update_cache();
            }
        }
    }

    update_shelter_data(build_shelter_data());

    Duration::new(60 * 5, 0)
}

fn shelter_update_job() -> Duration {
    info!("Start update job");

    {
        let cache_map = SHELTER_MAP.read().unwrap();

        for (&id, shelter) in cache_map.iter() {
            if !shelter.synced {
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

    {
        let mut cache_map = SHELTER_MAP.write().unwrap();

        for mut shelter in cache_map.values_mut() {
            shelter.synced = true;

            if shelter.recent_good > 0 {
                shelter.recent_good -= 1;
                shelter.reserve_update();
            }

            if shelter.recent_bad > 0 {
                shelter.recent_bad -= 1;
                shelter.reserve_update();
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
                let sh_name = val.get("name").and_then(|v| v.as_str()).unwrap().to_owned();
                let sh_latitude = val.get("latitude").and_then(|v| v.as_f64()).unwrap();
                let sh_longitude = val.get("latitude").and_then(|v| v.as_f64()).unwrap();
                let sh_info = format!("수용: {}명, 면적: {}㎡",
                    val.get("capacity").and_then(|v| v.as_i64()).unwrap(),
                    val.get("area").and_then(|v| v.as_f64()).unwrap());

                // Init DB.
                let db_result = db::insert_shelter(&db::models::NewShelter {
                    name: sh_name.clone(),
                    latitude: sh_latitude,
                    longitude: sh_longitude,
                    info: sh_info.clone(),
                    recent_good: 0,
                    recent_bad: 0,
                });

                match db_result {
                    Ok(db_shelter) => {
                        // Init shelters.
                        let mut cache_map = SHELTER_MAP.write().unwrap();
                        cache_map.insert(db_shelter.id, Shelter::new(db_shelter.id,
                            sh_name, sh_latitude, sh_longitude, sh_info));
                    },
                    Err(err) => panic!(err.to_string()),
                }
            }
        },
        Ok(shelters) => {
            // Init shelters.
            let mut cache_map = SHELTER_MAP.write().unwrap();

            for s in shelters {
                cache_map.insert(s.id, Shelter::new(s.id, s.name, s.latitude, s.longitude, s.info));
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

        cache_map.values().map(|s| {
            json!({
                "id": s.id,
                "latitude": s.latitude,
                "longitude": s.longitude,
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