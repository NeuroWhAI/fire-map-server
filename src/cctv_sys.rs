use std::{
    env,
    thread::{self},
    sync::RwLock,
    time::Duration,
    clone::Clone,
};
use rocket::{
    response::{
        content::Json,
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
    *CCTV_DATA.write().unwrap() = get_cctv_data()
        .expect("Fail to get CCTV data");
    thread::spawn(cctv_job)
}

#[get("/cctv")]
pub fn get_cctv() -> Json<String> {
    Json(CCTV_DATA.read().unwrap().clone())
}

fn cctv_job() {
    thread::sleep(Duration::new(60 * 3, 0));

    loop {
        match get_cctv_data() {
            Ok(data) => {
                {
                    *CCTV_DATA.write().unwrap() = data;
                }
                thread::sleep(Duration::new(60 * 3, 0));
            }
            _ => thread::sleep(Duration::new(60 * 1, 0))
        }
    }
}

fn get_cctv_data() -> Result<String, String> {
    let args = format!("key={}&ReqType=2&MinX=120&MaxX=150&MinY=30&MaxY=40&type=ex", *API_KEY);
    let result = reqwest::get(&format!("http://openapi.its.go.kr:8081/api/NCCTVInfo?{}", args))
        .and_then(|mut res| res.text());

    match result {
        Ok(data) => parse_cctv_data(data),
        Err(err) => Err(err.to_string()),
    }
}

fn parse_cctv_data(xml_str: String) -> Result<String, String> {
    let mut reader = xml::Reader::from_str(&xml_str);
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
                        cctvs.push(data.clone());
                        data.clear();
                    },
                    _ => (),
                }
            }
            Ok(Event::Text(e)) => {
                match name.as_slice() {
                    b"cctvurl" => data.url = e.unescape_and_decode(&reader).unwrap_or_default(),
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

    let part_cctvs = cctvs.iter()
        .filter(|tv| tv.is_valid())
        .map(|tv| {
            json!({
                "url": tv.url,
                "latitude": tv.latitude,
                "longitude": tv.longitude,
                "name": tv.name,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "cctvs": part_cctvs,
        "size": part_cctvs.len(),
    }).to_string())
}