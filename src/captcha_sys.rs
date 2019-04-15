use std::{
    sync::{Mutex},
    collections::HashMap,
    time::{Instant, Duration},
};
use rocket::{
    http::{Cookie, Cookies, ContentType},
    response::Content,
};
use captcha::{self, Difficulty};
use crate::util::{self};


lazy_static! {
    static ref ANSWER_MAP: Mutex<HashMap<String, CaptchaAnswer>> = {
        Mutex::new(HashMap::new())
    };
}

const COOKIE_NAME: &'static str = "captcha_id";
const MAX_MAP_SIZE: usize = 512;
const VALID_CAPTCHA_DURATION: u64 = 60 * 5;


struct CaptchaAnswer {
    answer: String,
    created_time: Instant,
}

impl CaptchaAnswer {
    fn new(answer: String) -> Self {
        CaptchaAnswer {
            answer,
            created_time: Instant::now(),
        }
    }

    fn is_valid(&self) -> bool {
        Instant::now() - self.created_time > Duration::new(VALID_CAPTCHA_DURATION, 0)
    }
}


pub fn verify_and_remove_captcha(mut cookies: Cookies, user_answer: &str) -> bool {
    if let Some(cookie) = cookies.get_private(COOKIE_NAME) {
        let mut map = ANSWER_MAP.lock().unwrap();
        let opt_answer = map.remove(cookie.value());
        
        cookies.remove_private(cookie);

        match opt_answer {
            Some(answer) => answer.answer == user_answer,
            None => false
        }
    }
    else {
        false
    }
}


#[get("/captcha")]
pub fn get_captcha(mut cookies: Cookies) -> Content<Vec<u8>> {
    // 캡차 생성.
    let (answer, img_bytes) = captcha::gen(Difficulty::Medium)
        .as_tuple()
        .unwrap();

    let captcha_id = loop {
        let id = util::generate_rand_id(32);
        let mut map = ANSWER_MAP.lock().unwrap();

        // 캡차 아이디가 중복되지 않으면
        if !map.contains_key(&id) {
            map.insert(id.clone(), CaptchaAnswer::new(answer));

            // 해시맵 크기가 일정 크기보다 커지면
            // 만료된 데이터를 삭제.
            if map.len() > MAX_MAP_SIZE {
                map.retain(|_, v| v.is_valid());
            }

            break id;
        }
    };

    // 쿠키에 캡차 아이디 저장.
    cookies.add_private(Cookie::new(COOKIE_NAME, captcha_id));

    // 캡차 이미지 반환.
    Content(ContentType::PNG, img_bytes)
}

#[get("/test-captcha?<answer>")]
pub fn test_captcha(answer: String, cookies: Cookies) -> &'static str {
    if verify_and_remove_captcha(cookies, &answer) {
        "Success!"
    }
    else {
        "Fail!"
    }
}