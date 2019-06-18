use std::{
    fs,
    sync::RwLock,
    time::Duration,
};
use rocket::{
    response::content::Json,
};
use serde_json::json;

use crate::util;
use crate::task_scheduler::{Task, TaskSchedulerBuilder};


lazy_static! {
    static ref DISTRICT_CODES: Vec<String> = {
        fs::read_to_string("data/district_code.txt")
            .map(|text| text.split(',').map(|s| s.to_owned()).collect())
            .expect("Can't initialize district codes")
    };
    static ref FORECAST_DATA: RwLock<String> = {
        RwLock::new(String::new())
    };
}


struct Forecast {
    code: String,
    level: f32,
}


pub fn init_fire_forecast_sys(scheduler: &mut TaskSchedulerBuilder) {
    let delay = match get_forecast_data(16) {
        Ok(data) => {
            update_forecast_cache(data);
            Duration::new(60 * 30, 0)
        },
        Err(err) => {
            warn!("Fail to init fire forecast cache: {}", err);

            update_forecast_cache(json!({
                "error": true,
                "fires": [],
                "size": 0,
            }).to_string());

            Duration::new(60 * 1, 0)
        }
    };

    scheduler.add_task(Task::new(forecast_job, delay));
}

#[get("/fire-forecast-map")]
pub fn get_fire_forecast_map() -> Json<String> {
    Json(FORECAST_DATA.read().unwrap().clone())
}


fn forecast_job() -> Duration {
    info!("Start job");

    match get_forecast_data(8) {
        Ok(data) => {
            update_forecast_cache(data);
            Duration::new(60 * 30, 0)
        },
        Err(err) => {
            warn!("Fail to get fire forecast data: {}", err);
            Duration::new(60 * 1, 0)
        },
    }
}

fn update_forecast_cache(json: String) {
    *FORECAST_DATA.write().unwrap() = json;
}

fn get_forecast_by_code(code: &str) -> Result<Forecast, String> {
    let uri = format!("http://forestfire.nifos.go.kr/mobile/jsp/fireGrade.jsp?cd={}&subCd={}",
        &code[..2], code);

    reqwest::get(&uri)
        .and_then(|mut res| res.text())
        .map_err(|err| err.to_string())
        .and_then(|html| {
            html.find(">전국<")
                .and_then(|idx| html[idx..].find("</table").map(|offset| idx + offset))
                .and_then(|idx| html[..idx].rfind("<td"))
                .and_then(|idx| html[idx..].find('>').map(|offset| idx + offset))
                .and_then(|idx| html[idx..].find("</td").map(|offset| (idx + 1, idx + offset)))
                .map(|(begin, end)| util::extract_text_from_html(&html[begin..end]))
                .and_then(|level| level.parse().ok())
                .map(|level| Forecast {
                    code: code.to_owned(),
                    level,
                })
                .ok_or("Fail to parse fire forecast".into())
        })
}

fn get_forecast_data(retry_cnt: usize) -> Result<String, String> {
    let mut left_retries = retry_cnt;
    let mut total_forecasts = Vec::new();

    for code in &*DISTRICT_CODES {
        loop {
            match get_forecast_by_code(code) {
                Ok(forecast) => {
                    total_forecasts.push(forecast);
                    break;
                },
                Err(err) => {
                    if left_retries > 0 {
                        warn!("Retry({}/{}) to get {} forecast data", left_retries, retry_cnt, code);
                        left_retries -= 1;
                    }
                    else {
                        return Err(err);
                    }
                },
            }
        }
    }

    let part_forecasts = total_forecasts.into_iter()
        .map(|forecast| {
            json!({
                "code": forecast.code,
                "lvl": forecast.level,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "error": false,
        "forecasts": part_forecasts,
        "size": part_forecasts.len(),
    }).to_string())
}
