pub fn get_utc_timestamp_sec() -> i64 {
    let js_date = js_sys::Date::new_0();
    let now_timestamp = js_date.get_time() / 1000.; // convert milliseconds to seconds
    now_timestamp as i64
}

fn get_utc_date() -> String {
    use chrono::prelude::*;
    let now_timestamp = get_utc_timestamp_sec();
    let utc_datetime = chrono::DateTime::from_timestamp(now_timestamp, 0).unwrap();
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