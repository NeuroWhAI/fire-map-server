use std::{
    f64,
    fs::File,
    io::{self, BufReader, BufRead, Write},
    sync::RwLock,
    time::Duration,
    collections::HashMap,
    time::Instant,
    rc::Rc,
    cell::RefCell,
    clone::Clone,
};
use rocket::{
    http::ContentType,
    response::{
        Content,
        content::Json,
    },
};
use serde_json::json;
use cgmath::{MetricSpace, Point2, Vector2};
use png::HasParameters;

use crate::util;
use crate::task_scheduler::{Task, TaskSchedulerBuilder};


lazy_static! {
    static ref STATION_INFO: HashMap<String, Station> = {
        let lines = BufReader::new(File::open("data/stninfo.csv")
            .expect("Fail to open station data file"))
            .lines()
            .skip(1)
            .map(|ln| ln.unwrap());

        let mut map = HashMap::new();

        for line in lines {
            let data = line.split(',').collect::<Vec<_>>();

            if data[0].is_empty() || data[5].is_empty() || data[6].is_empty()
                || !data[2].is_empty() {
                // Is data invalid?
                continue;
            }

            if let (Ok(lat), Ok(lon)) = (data[5].parse(), data[6].parse()) {
                map.insert(data[0].to_owned(), Station {
                    latitude: lat,
                    longitude: lon,
                });
            }
        }

        map
    };
    static ref WIND_METADATA: RwLock<String> = {
        RwLock::new(String::new())
    };
    static ref WIND_IMG: RwLock<HashMap<u64, Vec<u8>>> = {
        RwLock::new(HashMap::new())
    };
    static ref CLOCK: Instant = {
        Instant::now()
    };
}

const GRID_X_OFFSET: f64 = 13955566.87619434;
const GRID_Y_OFFSET: f64 = 3885936.2337022102;
const GRID_X_END: f64 = 14493683.55532198;
const GRID_Y_END: f64 = 4734203.787602952;
const GRID_RESOLUTION: f64 = 1024.0;
const GRID_HEIGHT: usize = ((GRID_Y_END - GRID_Y_OFFSET) / GRID_RESOLUTION) as usize;
const GRID_WIDTH: usize = ((GRID_X_END - GRID_X_OFFSET) / GRID_RESOLUTION) as usize;
const STATION_RANGE: i32 = 32;


struct ByteVec(Rc<RefCell<Vec<u8>>>);

impl Write for ByteVec {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl Clone for ByteVec {
    fn clone(&self) -> Self {
        ByteVec(self.0.clone())
    }
}

impl ByteVec {
    fn new() -> Self {
        ByteVec(Rc::new(RefCell::new(Vec::new())))
    }

    fn bytes(self) -> Result<Vec<u8>, ByteVec> {
        match Rc::try_unwrap(self.0) {
            Ok(cell) => Ok(cell.into_inner()),
            Err(rc) => Err(ByteVec(rc)),
        }
    }
}


struct Station {
    latitude: f64,
    longitude: f64,
}


struct StationData {
    latitude: f64,
    longitude: f64,
    wind: Vector2<f64>,
}


pub fn init_wind_sys(scheduler: &mut TaskSchedulerBuilder) {
    let delay = match get_wind_img() {
        Ok((id, metadata, img)) => {
            update_wind_map(id, metadata, img);
            Duration::new(60 * 5, 0)
        },
        Err(err) => {
            warn!("Fail to init wind: {}", err);

            let (id, metadata, img) = make_error_response();
            update_wind_map(id, metadata, img);

            Duration::new(60 * 1, 0)
        }
    };

    scheduler.add_task(Task::new(wind_job, delay));
}

#[get("/wind-map-metadata")]
pub fn get_wind_map_metadata() -> Json<String> {
    Json(WIND_METADATA.read().unwrap().clone())
}

#[get("/wind-map?<id>")]
pub fn get_wind_map(id: u64) -> Option<Content<Vec<u8>>> {
    let map = WIND_IMG.read().unwrap();
    if let Some(img) = map.get(&id) {
        Some(Content(ContentType::PNG, img.clone()))
    }
    else {
        None
    }
}


fn wind_job() -> Duration {
    info!("Start job");

    match get_wind_img() {
        Ok((id, metadata, img)) => {
            update_wind_map(id, metadata, img);
            Duration::new(60 * 5, 0)
        },
        Err(err) => {
            warn!("Fail to get wind image: {}", err);
            Duration::new(60 * 1, 0)
        },
    }
}

fn update_wind_map(id: u64, metadata: String, wind_img: Vec<u8>) {
    {
        let mut map = WIND_IMG.write().unwrap();

        // Remove old image data.
        let current_secs = CLOCK.elapsed().as_secs();
        map.retain(|&time, _| current_secs < time + 60 * 60);

        map.insert(id, wind_img);
    }
    {
        *WIND_METADATA.write().unwrap() = metadata;
    }
}

fn make_error_response() -> (u64, String, Vec<u8>) {
    let img_id = CLOCK.elapsed().as_secs();

    let metadata = json!({
        "error": true,
        "id": img_id,
        "width": GRID_WIDTH,
        "height": GRID_HEIGHT,
        "resolution": GRID_RESOLUTION,
        "offset_x": GRID_X_OFFSET,
        "offset_y": GRID_Y_OFFSET,
    }).to_string();

    (img_id, metadata, Vec::new())
}

fn get_wind_img() -> Result<(u64, String, Vec<u8>), String> {
    get_stations()
        .and_then(|stations| {
            if stations.is_empty() {
                warn!("No wind stations");
                return Ok(make_error_response());
            }


            let mut pixels = Vec::with_capacity(GRID_HEIGHT * GRID_WIDTH * 4);
            pixels.resize(pixels.capacity(), 0);


            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;

            let mut station_data = Vec::new();
            
            for stn in stations {
                // Calculate range of wind velocity.
                if stn.wind.x < min_x {
                    min_x = stn.wind.x;
                }
                if stn.wind.y < min_y {
                    min_y = stn.wind.y;
                }
                if stn.wind.x > max_x {
                    max_x = stn.wind.x;
                }
                if stn.wind.y > max_y {
                    max_y = stn.wind.y;
                }

                // Add stations to delaunay.
                let (x, y) = util::transform_lonlat(stn.longitude, stn.latitude);
                let (x, y) = ((x - GRID_X_OFFSET) / GRID_RESOLUTION, (y - GRID_Y_OFFSET) / GRID_RESOLUTION);
                station_data.push((x, y, stn.wind));

                // Show pixels in station range.
                for py in (y as i32 - STATION_RANGE)..(y as i32 + STATION_RANGE) {
                    if py < 0 || py as usize >= GRID_HEIGHT {
                        continue;
                    }

                    let y_index = (GRID_HEIGHT - 1 - py as usize) * GRID_WIDTH * 4;

                    for px in (x as i32 - STATION_RANGE)..(x as i32 + STATION_RANGE) {
                        if px < 0 || px as usize >= GRID_WIDTH {
                            continue;
                        }

                        let index = y_index + px as usize * 4;

                        // Set alpha.
                        pixels[index + 3] = 255;
                    }
                }
            }

            let x_term = max_x - min_x;
            let y_term = max_y - min_y;


            for y in 0..GRID_HEIGHT {
                let mut index = (GRID_HEIGHT - 1 - y) * GRID_WIDTH * 4;

                for x in 0..GRID_WIDTH {
                    let point = Point2::new(x as f64, y as f64);

                    let (total_weight, total_wind_x, total_wind_y) = station_data
                        .iter()
                        .map(|(stn_x, stn_y, wind)| {
                            let distance = point.distance2(Point2::new(*stn_x, *stn_y));
                            let weight = if distance < 1.0 {
                                1.0
                            } else {
                                1.0 / distance.powf(1.5)
                            };
                            (weight, weight * wind)
                        })
                        .fold((0.0, 0.0, 0.0), |acc, (weight, wind)| (acc.0 + weight, acc.1 + wind.x, acc.2 + wind.y));

                    let wind_x = total_wind_x / total_weight;
                    let wind_x = min_x.max(max_x.min(wind_x));
                    
                    let wind_y = total_wind_y / total_weight;
                    let wind_y = min_y.max(max_y.min(wind_y));

                    let norm_wind_x = 255.0 * (wind_x - min_x) / x_term;
                    let norm_wind_y = 255.0 * (wind_y - min_y) / y_term;

                    // RGBA
                    pixels[index + 0] = 0_f64.max(norm_wind_x.floor().min(255.0)) as u8;
                    pixels[index + 1] = 0_f64.max(norm_wind_y.floor().min(255.0)) as u8;

                    index += 4;
                }
            }

            let img_data = ByteVec::new();

            let mut encoder = png::Encoder::new(img_data.clone(), GRID_WIDTH as u32, GRID_HEIGHT as u32);
            encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
            let img_result = encoder.write_header()
                .and_then(|mut writer| writer.write_image_data(&pixels));

            if let Err(err) = img_result {
                return Err(err.to_string());
            }
            

            let img_id = CLOCK.elapsed().as_secs();

            let metadata = json!({
                "error": false,
                "id": img_id,
                "width": GRID_WIDTH,
                "height": GRID_HEIGHT,
                "resolution": GRID_RESOLUTION,
                "offset_x": GRID_X_OFFSET,
                "offset_y": GRID_Y_OFFSET,
                "min_x": min_x,
                "min_y": min_y,
                "max_x": max_x,
                "max_y": max_y,
            }).to_string();


            match img_data.bytes() {
                Ok(bytes) => Ok((img_id, metadata, bytes)),
                Err(_) => Err("Fail to get image bytes".into()),
            }
        })
}

fn get_stations() -> Result<Vec<StationData>, String> {
    reqwest::get("http://www.weather.go.kr/cgi-bin/aws/nph-aws_txt_min")
        .and_then(|mut res| res.text())
        .map_err(|err| err.to_string())
        .and_then(|html| {
            let begin_res = html.find("<table")
                .and_then(|idx| html[idx..].find("javascript").map(|offset| idx + offset))
                .and_then(|idx| html[..idx].rfind("<tr"));
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
                    return Err("Fail to parse station data".into());
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

                if row.len() > 16 {
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
                if let Some(station) = STATION_INFO.get(&row[0]) {
                    let wind_dir = row[14].parse::<f64>();
                    let wind_vel = row[16].parse::<f64>();

                    if let (Ok(dir), Ok(vel)) = (wind_dir, wind_vel) {
                        let angle = dir.to_radians();
                        let dir_x = angle.sin() * vel;
                        let dir_y = angle.cos() * vel;

                        data.push(StationData {
                            latitude: station.latitude,
                            longitude: station.longitude,
                            wind: Vector2::new(dir_x, dir_y),
                        });
                    }
                }
            }

            data
        })
}
