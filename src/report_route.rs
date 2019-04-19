use std::{
    time::{UNIX_EPOCH, Duration, Instant},
    sync::RwLock,
    fs::{self},
    path::Path,
    io::{self, Read, Write},
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
use serde_json::json;
use chrono::Utc;
use crate::db::{self};
use crate::util::{self};
use crate::captcha_sys::verify_and_remove_captcha;


type JsonResult = Result<Json<String>, BadRequest<String>>;
type StringResult = Result<String, BadRequest<String>>;


lazy_static! {
    static ref REPORT_MAP_CACHE: RwLock<ReportMapCache> = {
        RwLock::new(ReportMapCache::new())
    };
}

const REPORT_DURATION: u64 = 48 * 60 * 60; // seconds
const CACHE_VALID_DURATION: u64 = 10; // seconds
const PASSWORD_HASH_SORT: &'static str = "^^ NeuroWhAI 42 5749";
const FILE_UPLOAD_LIMIT: usize = (8 * 1024 * 1024 / 3) * 4; // chars
pub const IMAGE_UPLOAD_DIR: &'static str = "upload/images/";
pub const IMAGE_PUBLIC_DIR: &'static str = "images/";


fn make_json_result(json: String) -> JsonResult {
    Ok(Json(json))
}

fn make_json_error(err: String) -> JsonResult {
    Err(BadRequest(Some(err)))
}

fn make_string_result(txt: String) -> StringResult {
    Ok(txt)
}

fn make_string_error(err: String) -> StringResult {
    Err(BadRequest(Some(err)))
}


struct ReportMapCache {
    data: Option<String>,
    created_time: Instant,
}

impl ReportMapCache {
    fn new() -> Self {
        ReportMapCache {
            data: None,
            created_time: Instant::now(),
        }
    }

    fn is_valid(&self) -> bool {
        self.data.is_some()
            && self.created_time.elapsed() <= Duration::new(CACHE_VALID_DURATION, 0)
    }

    fn update(&mut self, data: String) {
        self.data = Some(data);
        self.created_time = Instant::now();
    }

    fn get_data(&self) -> String {
        (*self.data.as_ref().unwrap()).clone()
    }
}


#[derive(FromForm)]
pub struct ReportForm {
    captcha: String,
    user_id: String,
    user_pwd: String,
    latitude: f64,
    longitude: f64,
    lvl: i32,
    description: String,
    img_key: String,
}

impl ReportForm {
    fn verify_error(&self) -> Option<&'static str> {
        if self.user_id.find(char::is_whitespace).is_some() {
            Some("The ID can not contain spaces")
        }
        else if self.user_id.len() < 2 {
            Some("ID must be at least 2 characters")
        }
        else if self.user_id.len() > 24 {
            Some("ID can not be longer than 24 characters")
        }
        else if self.user_pwd.len() < 4 {
            Some("Password must be at least 4 characters")
        }
        else if self.lvl < 0 || self.lvl >= 5 {
            Some("Invalid level")
        }
        else if self.description.len() >= 65536 {
            Some("The maximum length of the description is 65536")
        }
        else if self.img_key.find("..").is_some()
            || self.img_key.len() > 256 {
            Some("Invalid image key")
        }
        else {
            None
        }
    }
}


#[get("/report?<id>")]
pub fn get_report(id: i32) -> JsonResult {
    let result = db::get_report(id);

    if let Ok(r) = result {
        make_json_result(json!({
            "id": r.id,
            "user_id": r.user_id,
            "latitude": r.latitude,
            "longitude": r.longitude,
            "created_time": r.created_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "lvl": r.lvl,
            "description": r.description,
            "img_path": r.img_path,
        }).to_string())
    }
    else {
        make_json_error(result.err().unwrap().to_string())
    }
}

#[get("/report-map")]
pub fn get_report_map() -> JsonResult {
    // 유효한 캐시 데이터가 있다면 반환.
    {
        let cache = REPORT_MAP_CACHE.read().unwrap();
        if cache.is_valid() {
            return make_json_result(cache.get_data())
        }
    }


    let result = db::get_reports_within(Duration::new(REPORT_DURATION, 0));

    if let Ok(reports) = result {
        let part_jsons = reports.iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "user_id": r.user_id,
                    "latitude": r.latitude,
                    "longitude": r.longitude,
                    "created_time": r.created_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    "lvl": r.lvl,
                })
            })
            .collect::<Vec<_>>();

        let reports_json = json!({
            "reports": part_jsons,
            "size": part_jsons.len(),
        }).to_string();

        // 캐시 데이터 갱신.
        {
            let mut cache = REPORT_MAP_CACHE.write().unwrap();
            cache.update(reports_json.clone());
        }

        make_json_result(reports_json)
    }
    else {
        make_json_error(result.err().unwrap().to_string())
    }
}

#[post("/upload-image", format="plain", data="<data>")]
pub fn post_upload_image(data: Data) -> StringResult {
    // Read base64 encoded string.
    let mut file_data = data.open().take(FILE_UPLOAD_LIMIT as u64 + 1);
    let mut data_uri = String::new();
    let read_result = file_data.read_to_string(&mut data_uri);

    match read_result {
        Ok(bytes) if bytes <= FILE_UPLOAD_LIMIT => (),
        Ok(_) => return make_string_error("The file is too large".into()),
        Err(err) => return make_string_error(err.to_string()),
    }

    // Get file extension.
    let ext_result = data_uri.split(',').nth(0)
        .and_then(|x| x.split('/').nth(1))
        .and_then(|x| x.split(';').nth(0));
    if ext_result.is_none() {
        return make_string_error("Invalid uri".into());
    }
    let ext = ext_result.unwrap();

    // Check file extension.
    let allowed_exts = &["jpeg", "jpg", "png", "bmp"];
    if !allowed_exts.iter().any(|&x| x == ext) {
        return make_string_error("Invalid extension".into());
    }

    // Decode base64 string to bytes.
    let decode_result = data_uri.split(',').nth(1)
        .ok_or("Invalid uri".to_owned())
        .and_then(|b64| base64::decode(b64).map_err(|err| err.to_string()));
    if let Err(err) = decode_result {
        return make_string_error(err);
    }
    let bytes = decode_result.unwrap();

    // Create unique id and file for the image.
    let (id, mut file) = loop {
        let id = util::generate_rand_id(32) + "." + ext;
        let path = Path::new(IMAGE_UPLOAD_DIR).join(&id);
        let file_result = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path);

        match file_result {
            Ok(file) => break (id, file),
            Err(ref err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return make_string_error(err.to_string()),
        }
    };

    // Save bytes to file.
    match file.write_all(&bytes) {
        Ok(_) => make_string_result(id),
        Err(err) => make_string_error(err.to_string()),
    }
}

#[post("/report", format="application/x-www-form-urlencoded", data="<form>")]
pub fn post_report(form: Option<Form<ReportForm>>, cookies: Cookies)
    -> StringResult {

    if form.is_none() {
        return make_string_error("Invalid form".into());
    }

    let form = form.unwrap();


    if let Some(err) = form.verify_error() {
        return make_string_error(err.to_string());
    }
    
    if !verify_and_remove_captcha(cookies, 1, &form.captcha) {
        return make_string_error("Wrong captcha".into());
    }


    let img_path: String = if form.img_key.len() > 0 {
        // Move a uploaded image to public directory if exists.
        let uploaded_file = Path::new(IMAGE_UPLOAD_DIR).join(&form.img_key);
        if uploaded_file.exists() {
            let public_file = Path::new(IMAGE_PUBLIC_DIR).join(&form.img_key);
            let move_result = fs::copy(&uploaded_file, Path::new(crate::STATIC_DIR).join(&public_file))
                .and(fs::remove_file(&uploaded_file));

            match move_result {
                Err(err) => return make_string_error(err.to_string()),
                _ => ()
            }

            match public_file.to_str() {
                Some(path) => path.into(),
                None => return make_string_error("Invalid public path".into())
            }
        }
        else {
            return make_string_error("No images uploaded".into());
        }
    }
    else {
        "".into()
    };


    let sorted_pwd = form.user_pwd.clone() + PASSWORD_HASH_SORT;
    let utc = Utc::now().timestamp() as u64;

    let new_report = db::models::NewReport {
        user_id: form.user_id.clone(),
        user_pwd: util::calculate_hash(&sorted_pwd).to_string(),
        latitude: form.latitude,
        longitude: form.longitude,
        created_time: UNIX_EPOCH + Duration::new(utc, 0),
        lvl: form.lvl,
        description: form.description.clone(),
        img_path: img_path,
    };

    match db::insert_report(&new_report) {
        Ok(report) => make_string_result(report.id.to_string()),
        Err(err) => make_string_error(err.to_string())
    }
}

#[delete("/report?<id>&<user_id>&<user_pwd>")]
pub fn delete_report(id: i32, user_id: String, user_pwd: String)
    -> StringResult {

    let sorted_pwd = user_pwd + PASSWORD_HASH_SORT;
    let hashed_pwd = util::calculate_hash(&sorted_pwd).to_string();

    let result = db::get_report(id);

    match result {
        Ok(report) => {
            if report.user_id == user_id && report.user_pwd == hashed_pwd {
                // 이미지 파일이 있다면 삭제.
                if report.img_path.len() > 0 {
                    let img_path = Path::new(crate::STATIC_DIR).join(&report.img_path);
                    if img_path.exists() && img_path.is_file() {
                        let _ = fs::remove_file(img_path);
                    }
                }
                
                // 삭제하고 결과 반환.
                let del_result = db::delete_report(id);
                match del_result {
                    Ok(cnt) if cnt > 0 => make_string_result(cnt.to_string()),
                    Ok(_) => make_string_error("Not found".into()),
                    Err(err) => make_string_error(err.to_string()),
                }
            }
            else {
                make_string_error("Authentication result is incorrect".into())
            }
        }
        _ => make_string_error("Not found".into())
    }
}

#[post("/bad-report?<id>&<captcha>")]
pub fn post_bad_report(id: i32, captcha: String, cookies: Cookies) -> StringResult {
    if !verify_and_remove_captcha(cookies, 2, &captcha) {
        return make_string_error("Wrong captcha".into());
    }

    if db::get_report(id).is_ok() {
        if db::get_bad_report(id).is_ok() {
            make_string_result(id.to_string())
        }
        else {
            let result = db::insert_bad_report(&db::models::NewBadReport {
                id
            });

            match result {
                Ok(r) => make_string_result(r.id.to_string()),
                Err(err) => make_string_error(err.to_string()),
            }
        }
    }
    else {
        make_string_error("Not exists".into())
    }
}