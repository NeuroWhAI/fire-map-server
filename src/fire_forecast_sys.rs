use std::{
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
    static ref FORECAST_DATA: RwLock<String> = {
        RwLock::new(String::new())
    };
}


struct Forecast {
    code: String,
    level: f32,
}


pub fn init_fire_forecast_sys(scheduler: &mut TaskSchedulerBuilder) {
    update_forecast_cache(get_forecast_data()
        .expect("Fail to get fire forecast data"));

    scheduler.add_task(Task::new(forecast_job, Duration::new(60 * 15, 0)));
}

#[get("/fire-forecast-map")]
pub fn get_fire_forecast_map() -> Json<String> {
    Json(FORECAST_DATA.read().unwrap().clone())
}


fn forecast_job() -> Duration {
    info!("Start job");

    match get_forecast_data() {
        Ok(data) => {
            update_forecast_cache(data);
            Duration::new(60 * 15, 0)
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

fn get_forecast_data() -> Result<String, String> {
    reqwest::get("http://forestfire.nifos.go.kr/mobile/jsp/fireGrade.jsp")
        .and_then(|mut res| res.text())
        .map_err(|err| err.to_string())
        .and_then(|html| {
            let begin_res = html.find(">전국<")
                .and_then(|idx| html[idx..].find("<tr").map(|offset| idx + offset));
            let end_res = begin_res.as_ref()
                .and_then(|&begin| html[begin..].find("</table").map(|offset| begin + offset));

            if let (Some(begin), Some(end)) = (begin_res, end_res) {
                Ok((html, begin, end))
            }
            else {
                Err("Fail to parse table".into())
            }
        })
        .and_then(|(html, mut begin, end)| {
            let mut table: Vec<Vec<_>> = Vec::new();

            while begin < end {
                let mut row = Vec::new();

                let end_tr = html[begin..].find("</tr").map(|offset| begin + offset);
                if end_tr.is_none() {
                    return Err("Fail to parse forecast data".into());
                }
                let end_tr = end_tr.unwrap();

                loop {
                    let begin_res = html[begin..].find("<td").map(|offset| begin + offset);
                    if begin_res.is_none() {
                        break;
                    }
                    begin = begin_res.unwrap();

                    if begin > end_tr {
                        break;
                    }

                    let begin_res = html[begin..].find('>').map(|offset| begin + offset);
                    if begin_res.is_none() {
                        break;
                    }
                    begin = begin_res.unwrap();

                    let end_td_res = html[begin..].find("</td").map(|offset| begin + offset);
                    if end_td_res.is_none() {
                        break;
                    }
                    let end_td = end_td_res.unwrap();

                    row.push(util::extract_text_from_html(&html[(begin + 1)..end_td]));
                }

                if row.len() >= 3 {
                    table.push(row);
                }

                let begin_res = html[end_tr..].find("<tr").map(|offset| end_tr + offset);
                if begin_res.is_none() {
                    break;
                }
                begin = begin_res.unwrap();
            }

            Ok(table)
        })
        .map(|table| {
            let mut data = Vec::new();

            for row in table {
                let level = row[2].parse::<f32>();

                if let Ok(lvl) = level {
                    data.push(Forecast {
                        code: row[0].clone(),
                        level: lvl,
                    });
                }
            }

            data
        })
        .map(|total_forecasts| {
            let part_forecasts = total_forecasts.into_iter()
                .map(|forecast| {
                    json!({
                        "code": forecast.code,
                        "lvl": forecast.level,
                    })
                })
                .collect::<Vec<_>>();

            json!({
                "forecasts": part_forecasts,
                "size": part_forecasts.len(),
            }).to_string()
        })
}
