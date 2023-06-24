use base64::engine::{general_purpose, Engine as _};
use rand::RngCore;

pub fn get_utc_timestamp_sec() -> i64 {
    let js_date = js_sys::Date::new_0();
    let now_timestamp = js_date.get_time() / 1000.; // convert milliseconds to seconds
    now_timestamp as i64
}

fn get_utc_date() -> String {
    use chrono::prelude::*;
    let now_timestamp = get_utc_timestamp_sec();
    let naive_datetime = chrono::NaiveDateTime::from_timestamp_opt(now_timestamp, 0).unwrap();
    let utc_datetime = chrono::Utc.from_utc_datetime(&naive_datetime);
    format!(
        "{}-{:02}-{:02}",
        utc_datetime.year(),
        utc_datetime.month(),
        utc_datetime.day(),
    )
}

pub fn make_id() -> String {
    let unique = uuid::Uuid::new_v4();
    format!("{}-{}", get_utc_date(), unique)
}

pub fn make_secret() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    general_purpose::STANDARD_NO_PAD.encode(&bytes)
}
