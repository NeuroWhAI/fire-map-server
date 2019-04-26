use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::f64;

use rand::{
    thread_rng, Rng,
    distributions,
};


pub fn generate_rand_id(length: usize) -> String {
    thread_rng()
        .sample_iter(&distributions::Alphanumeric)
        .take(length)
        .collect()
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn extract_text_from_html(html: &str) -> String {
    let mut buffer = String::new();

    let mut chars = html.chars();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            while let Some(ch2) = chars.next() {
                if ch2 == '>' {
                    break;
                }
            }
        }
        else {
            buffer.push(ch);
        }
    }

    buffer
}

const LL_RADIUS: f64 = 6378136.98;
const LL_RANGE: f64 = LL_RADIUS * f64::consts::PI * 2.0;
const LL_LON2X: f64 = LL_RANGE / 360.0;
const LL_RAD_OVER_DEG: f64 = f64::consts::PI / 180.0;
pub fn transform_lonlat(longitude: f64, latitude: f64) -> (f64, f64) {
    let x = longitude * LL_LON2X;

    if latitude > 86.0 {
        (x, LL_RANGE)
    }
    else if latitude < -86.0 {
        (x, -LL_RANGE)
    }
    else {
        let y = latitude * LL_RAD_OVER_DEG;
        let y = (1.0 / y.cos() + y.tan()).log(f64::consts::E);
        (x, y * LL_RADIUS)
    }
}