use std::fs::{self};
use rocket::{
    response::{
        status::BadRequest,
        content::Json,
    },
};


type JsonResult = Result<Json<String>, BadRequest<String>>;


lazy_static! {
    static ref SHELTER_DATA: String = {
        fs::read_to_string("data/shelter.json")
            .expect("Can't find shelter.json")
    };
}


#[get("/shelter-map")]
pub fn get_shelter_map() -> JsonResult {
    Ok(Json(SHELTER_DATA.clone()))
}