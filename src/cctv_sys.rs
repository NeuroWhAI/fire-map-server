use std::{
    env,
    thread::{self},
    sync::RwLock,
    time::Duration,
    clone::Clone,
    collections::HashMap,
    path::Path,
};
use rocket::{
    response::{
        content::Json,
        status::NotFound,
    },
};
use quick_xml::{
    self as xml,
    events::Event,
    Error::UnexpectedToken,
};
use serde_json::json;


lazy_static! {
    static ref API_KEY: String = {
        env::var("CCTV_KEY")
            .expect("CCTV_KEY must be set")
    };
    static ref CCTV_DATA: RwLock<String> = {
        RwLock::new(String::new())
    };
    static ref CCTV_LIST: RwLock<HashMap<String, CctvData>> = {
        RwLock::new(HashMap::new())
    };
}


struct CctvData {
    url: String,
    latitude: f32,
    longitude: f32,
    name: String,
}

impl CctvData {
    fn new() -> Self {
        CctvData {
            url: "".into(),
            latitude: 0.0,
            longitude: 0.0,
            name: "".into(),
        }
    }

    fn clear(&mut self) {
        self.url.clear();
        self.latitude = 0.0;
        self.longitude = 0.0;
        self.name.clear();
    }

    fn is_valid(&self) -> bool {
        self.url.len() > 0
            && self.latitude > 20.0 && self.latitude < 50.0
            && self.longitude > 110.0 && self.longitude < 160.0
            && self.name.len() > 0
    }
}

impl Clone for CctvData {
    fn clone(&self) -> Self {
        CctvData {
            url: self.url.clone(),
            latitude: self.latitude,
            longitude: self.longitude,
            name: self.name.clone(),
        }
    }
}


pub fn init_cctv_sys() -> thread::JoinHandle<()> {
    update_cctv_cache(get_cctv_data(false)
        .expect("Fail to get CCTV data"));

    thread::spawn(cctv_job)
}

#[get("/cctv-map")]
pub fn get_cctv_map() -> Json<String> {
    Json(CCTV_DATA.read().unwrap().clone())
}

#[get("/cctv?<name>")]
pub fn get_cctv(name: String) -> Result<Json<String>, NotFound<String>> {
    let list = CCTV_LIST.read().unwrap();

    list.get(&name)
        .ok_or(NotFound("There is no CCTV with that name".into()))
        .map(|tv| {
            Json(json!({
                "url": tv.url,
                "latitude": tv.latitude,
                "longitude": tv.longitude,
                "name": tv.name,
            }).to_string())
        })
}

fn cctv_job() {
    thread::sleep(Duration::new(60 * 3, 0));

    loop {
        match get_cctv_data(true) {
            Ok(data) => {
                update_cctv_cache(data);
                thread::sleep(Duration::new(60 * 3, 0));
            }
            _ => thread::sleep(Duration::new(60 * 1, 0))
        }
    }
}

fn update_cctv_cache(cctvs: Vec<CctvData>) {
    {
        *CCTV_DATA.write().unwrap() = stringify_cctvs(&cctvs);
    }

    for tv in cctvs {
        let mut list = CCTV_LIST.write().unwrap();

        if let Some(cache) = list.get_mut(&tv.name) {
            *cache = tv;
        }
        else {
            list.insert(tv.name.clone(), tv);
        }
    }
}

fn get_cctv_data(allow_error: bool) -> Result<Vec<CctvData>, String> {
    let args = format!("key={}&ReqType=2&MinX=120&MaxX=150&MinY=30&MaxY=40", *API_KEY);
    let url = format!("http://openapi.its.go.kr:8081/api/NCCTVInfo?{}", args);
    let ex_result = reqwest::get(&format!("{}&type=ex", url))
        .and_then(|mut res| res.text());
    let its_result = reqwest::get(&format!("{}&type=its", url))
        .and_then(|mut res| res.text());

    match (ex_result, its_result) {
        (Ok(ex), Ok(its)) => parse_cctv_data(&ex).and_then(|mut v_ex| {
            parse_cctv_data(&its).map(|mut v_its| {
                v_its.append(&mut v_ex);
                v_its
            })
        }),
        (Ok(ref ex), Err(_)) if allow_error => parse_cctv_data(ex),
        (Err(_), Ok(ref its)) if allow_error => parse_cctv_data(its),
        (_, Err(err)) => Err(err.to_string()),
        (Err(err), _) => Err(err.to_string()),
    }
}

fn stringify_cctvs(cctvs: &Vec<CctvData>) -> String {
    let part_cctvs = cctvs.iter()
        .map(|tv| {
            json!({
                "latitude": tv.latitude,
                "longitude": tv.longitude,
                "name": tv.name,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "cctvs": part_cctvs,
        "size": part_cctvs.len(),
    }).to_string()
}

fn parse_cctv_data(xml_str: &String) -> Result<Vec<CctvData>, String> {
    let mut reader = xml::Reader::from_str(xml_str);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut cctvs = Vec::new();
    let mut name = Vec::new();
    let mut data = CctvData::new();

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                name.clear();
                name.extend_from_slice(e.name());
            },
            Ok(Event::End(ref e)) => {
                match e.name() {
                    b"data" => {
                        if data.is_valid() {
                            cctvs.push(data.clone());
                        }
                        data.clear();
                    },
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                match name.as_slice() {
                    b"cctvurl" => data.url = convert_cctv_url(&e.unescape_and_decode(&reader).unwrap_or_default()),
                    b"coordy" => data.latitude = e.unescape_and_decode(&reader)
                        .and_then(|s| s.parse::<f32>().map_err(|e| UnexpectedToken(e.to_string())))
                        .unwrap_or_default(),
                    b"coordx" => data.longitude = e.unescape_and_decode(&reader)
                        .and_then(|s| s.parse::<f32>().map_err(|e| UnexpectedToken(e.to_string())))
                        .unwrap_or_default(),
                    b"cctvname" => data.name = e.unescape_and_decode(&reader).unwrap_or_default(),
                    _ => (),
                }
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => (),
        }

        buf.clear();
    }

    Ok(cctvs.into_iter().collect())
}

fn convert_cctv_url(url: &String) -> String {
    let route = Path::new(url).strip_prefix("http://cctvsec.ktict.co.kr/");

    match route {
        Ok(route) => {
            let converted = Path::new("/cctv-proxy/").join(route);

            match converted.to_str() {
                Some(converted_url) => converted_url.to_owned(),
                None => url.clone(),
            }
        },
        Err(_) => url.clone(),
    }
}