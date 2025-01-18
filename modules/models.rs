use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct DefaultConfig {
    pub def_uv1_start: String,
    pub def_uv1_end: String,
    pub def_uv2_start: String,
    pub def_uv2_end: String,
    pub def_heat_start: String,
    pub def_heat_end: String,
    pub def_led_R: i32,
    pub def_led_G: i32,
    pub def_led_B: i32,
    pub def_led_CW: i32,
    pub def_led_WW: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub week_number: i32,
    pub uv1_start: String,
    pub uv1_end: String,
    pub uv2_start: String,
    pub uv2_end: String,
    pub heat_start: String,
    pub heat_end: String,
    pub led_r: i32,
    pub led_g: i32,
    pub led_b: i32,
    pub led_cw: i32,
    pub led_ww: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Override {
    pub id: i32,
    pub red: Option<i32>,
    pub green: Option<i32>,
    pub blue: Option<i32>,
    pub cool_white: Option<i32>,
    pub warm_white: Option<i32>,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct History {
    pub id: i32,
    pub timestamp: String,
    pub temp_basking1: Option<f32>,
    pub temp_basking2: Option<f32>,
    pub temp_cool: Option<f32>,
    pub humidity: Option<f32>,
    pub time_uv1: Option<String>,
    pub time_uv2: Option<String>,
    pub time_heat: Option<String>,
    pub overheat: Option<String>,
}