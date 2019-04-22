use std::{
    thread::{self},
    sync::RwLock,
    time::Duration,
};
use rocket::{
    http::ContentType,
    response::Content,
};


lazy_static! {
    static ref WARNING_IMG_URI: RwLock<String> = {
        RwLock::new(String::new())
    };
    static ref WARNING_IMG: RwLock<Vec<u8>> = {
        RwLock::new(Vec::new())
    };
}


pub fn init_fire_sys() -> thread::JoinHandle<()> {
    let img_uri = get_fire_warning_image_uri()
        .expect("Fail to get uri of fire warning image");
    update_fire_image_uri(img_uri.clone());
    update_fire_image(get_fire_warning_image(&img_uri)
        .expect("Fail to get fire warning image"));

    thread::spawn(fire_job)
}

#[get("/fire-warning")]
pub fn get_fire_warning() -> Content<Vec<u8>> {
    Content(ContentType::PNG, WARNING_IMG.read().unwrap().clone())
}

fn fire_job() {
    thread::sleep(Duration::new(60 * 5, 0));

    loop {
        let uri_result = get_fire_warning_image_uri();

        if let Ok(ref uri) = uri_result {
            if &*WARNING_IMG_URI.read().unwrap() == uri {
                thread::sleep(Duration::new(60 * 5, 0));
                continue;
            }
        }

        let bytes_result = uri_result
            .and_then(|uri| get_fire_warning_image(&uri));

        match bytes_result {
            Ok(bytes) => {
                update_fire_image(bytes);
                thread::sleep(Duration::new(60 * 5, 0));
            }
            _ => thread::sleep(Duration::new(60 * 1, 0))
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
                    (&html[index..]).find("intro_img04.png")
                        .or((&html[index..]).find("intro_img05.png"))
                        .or((&html[index..]).find("intro_img06.png"))
                        .or((&html[index..]).find("intro_img07.png"))
                        .and_then(|offset| Some(index + offset))
                })
                .and_then(|index| (&html[..index]).rfind('"'))
                .and_then(|index| {
                    (&html[(index + 1)..]).find('"')
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